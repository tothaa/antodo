//! Local, authenticated, stateless Streamable HTTP MCP transport and domain tools.
//! The network threads never own a document. `poll` runs commands on the same
//! working copy as the GUI, serializing changes with all other domain commands.
use super::{Document, Item, Note, Result, Status, invalid};
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use uuid::Uuid;

const PROTOCOL_VERSIONS: [&str; 3] = ["2025-11-25", "2025-06-18", "2025-03-26"];

#[derive(Clone)]
pub struct Config {
    pub address: IpAddr,
    pub port: u16,
    pub token: String,
    pub allow_edits: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            address: "127.0.0.1".parse().unwrap(),
            port: 8765,
            token: format!("{}{}", Uuid::now_v7().simple(), Uuid::now_v7().simple()),
            allow_edits: false,
        }
    }
}
impl Config {
    pub fn validate(&self) -> Result<()> {
        if !self.address.is_loopback() {
            return Err(invalid("MCP must bind to a loopback address"));
        }
        if self.token.len() < 16
            || !self
                .token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(invalid(
                "Use an access token of at least 16 letters, digits, hyphens or underscores",
            ));
        }
        Ok(())
    }
}
struct Request {
    value: Value,
    reply: mpsc::SyncSender<Value>,
    active: Arc<AtomicBool>,
}
pub struct Server {
    pub address: SocketAddr,
    config: Config,
    receiver: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn start(config: Config) -> Result<Self> {
        config.validate()?;
        let listener = TcpListener::bind(SocketAddr::new(config.address, config.port))?;
        let address = listener.local_addr()?;
        listener.set_nonblocking(true)?;
        let stop = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = mpsc::sync_channel(32);
        let worker_stop = stop.clone();
        let token = config.token.clone();
        let worker = thread::Builder::new()
            .name("antodo-mcp".into())
            .spawn(move || {
                let mut clients: Vec<JoinHandle<()>> = Vec::new();
                while !worker_stop.load(Ordering::Relaxed) {
                    clients.retain(|client| !client.is_finished());
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            if clients.len() >= 8 {
                                let _ = respond(&mut stream, 503, None);
                                continue;
                            }
                            let sender = sender.clone();
                            let stop = worker_stop.clone();
                            let token = token.clone();
                            clients.push(thread::spawn(move || {
                                let _ = serve(&mut stream, address, &token, &sender, &stop);
                            }));
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
                for client in clients {
                    let _ = client.join();
                }
            })?;
        Ok(Self {
            address,
            config,
            receiver,
            stop,
            worker: Some(worker),
        })
    }
    pub fn url(&self) -> String {
        format!("http://{}/mcp", self.address)
    }
    /// Process a bounded batch; mutations are refused while a local draft/modal is active.
    pub fn poll(&self, document: &mut Document, local_edits_clear: bool) -> bool {
        let mut changed = false;
        for request in self.receiver.try_iter().take(16) {
            if !request.active.load(Ordering::Acquire) {
                continue;
            }
            let (value, mutation) = dispatch(
                document,
                &request.value,
                self.config.allow_edits,
                local_edits_clear,
            );
            changed |= mutation;
            let _ = request.reply.send(value);
        }
        changed
    }
    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
    }
}

fn rpc_error(id: Value, code: i32, message: impl Into<String>) -> Value {
    json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message.into()}})
}
/// JSON-RPC implementation, shared by the real transport and protocol tests.
pub fn dispatch(
    document: &mut Document,
    request: &Value,
    allow_edits: bool,
    local_edits_clear: bool,
) -> (Value, bool) {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    if request.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
        || request.get("method").and_then(Value::as_str).is_none()
        || !(id.is_string() || id.is_number() || id.is_null())
    {
        return (rpc_error(id, -32600, "Invalid JSON-RPC request"), false);
    }
    let params = request.get("params").cloned().unwrap_or(json!({}));
    let mut changed = false;
    let result = match request["method"].as_str().unwrap_or("") {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("");
            let version = if PROTOCOL_VERSIONS.contains(&requested) {
                requested
            } else {
                PROTOCOL_VERSIONS[0]
            };
            json!({"protocolVersion":version,"capabilities":{"tools":{"listChanged":false}},"serverInfo":{"name":"antodo","version":env!("CARGO_PKG_VERSION")},"instructions":"Works on the open antodo working document. List/get tools return internal IDs for subsequent calls. Dates use {value: YYYY-MM-DD or YYYY-MM-DD HH:MM:SS, timezone: IANA zone}. Edits are not saved to disk automatically; use Save in antodo. Local drafts block external mutations."})
        }
        "ping" => json!({}),
        "tools/list" => json!({"tools":tools(allow_edits)}),
        "tools/call" => {
            let name = params["name"].as_str().unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            let write = [
                "create_task",
                "update_task",
                "delete_task",
                "create_note",
                "update_note",
                "delete_note",
            ]
            .contains(&name);
            let output = if write && !allow_edits {
                Err(invalid("Server is read-only; enable edits and restart it"))
            } else if write && !local_edits_clear {
                Err(invalid(
                    "Finish or discard local edits and close dialogs before changing tasks or notes through MCP",
                ))
            } else {
                call(document, name, &arguments)
            };
            let is_error = output.is_err();
            changed = write && !is_error;
            let text = output
                .map(|value| value.to_string())
                .unwrap_or_else(|error| error.to_string());
            json!({"content":[{"type":"text","text":text}],"isError":is_error})
        }
        _ => return (rpc_error(id, -32601, "Method not found"), false),
    };
    (json!({"jsonrpc":"2.0","id":id,"result":result}), changed)
}
pub(crate) fn tools(edits: bool) -> Vec<Value> {
    let mut tools = vec![
        tool(
            "list_tasks",
            "List/search applied task records, including linked note_ids. Pagination is explicit; status uses Pending, Wip, Done, OnHold, WontDo, Someday or Event.",
            json!({"search":{"type":"string"},"status":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
            &[],
            true,
        ),
        tool(
            "get_task",
            "Get a complete task by its internal ID, including note_ids for linked Notes.",
            json!({"id":{"type":"string"}}),
            &["id"],
            true,
        ),
        tool(
            "list_tags",
            "List the workspace tags and their IDs, with explicit pagination.",
            json!({"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
            &[],
            true,
        ),
        tool(
            "list_notes",
            "List/search applied Markdown notes by name or content, optionally filtering by tag_id and task_id. Notes include tag IDs and linked task IDs. Pagination is explicit.",
            json!({"search":{"type":"string"},"tag_id":{"type":"string"},"task_id":{"type":"string"},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":200}}),
            &[],
            true,
        ),
        tool(
            "get_note",
            "Get a complete Markdown note by its internal ID, including tags and linked tasks.",
            json!({"id":{"type":"string"}}),
            &["id"],
            true,
        ),
    ];
    if edits {
        tools.extend([
            tool("create_task", "Create a task with optional properties. Core validates schedules, recurrence and relationships. Changes stay unsaved until Save in antodo.", json!({"name":{"type":"string"},"fields":task_fields(false)}), &["name"], false),
            tool("update_task", "Patch task properties; omitted fields are preserved. Set optional dates to null to clear. Internal id cannot be changed.", json!({"id":{"type":"string"},"fields":task_fields(true)}), &["id","fields"], false),
            tool("delete_task", "Delete a task, retaining its children. Changes stay unsaved until Save in antodo.", json!({"id":{"type":"string"}}), &["id"], false),
            tool("create_note", "Create a named Markdown note with optional tags and task links. Changes stay unsaved until Save in antodo.", note_fields(), &["name"], false),
            tool("update_note", "Patch a note's name, Markdown content, tags or task links; omitted fields are preserved. Relation arrays replace their whole set; use [] to clear. Internal id cannot be changed.", json!({"id":{"type":"string"},"fields":{"type":"object","properties":note_fields(),"additionalProperties":false}}), &["id","fields"], false),
            tool("delete_note", "Delete a Markdown note. Changes stay unsaved until Save in antodo.", json!({"id":{"type":"string"}}), &["id"], false),
        ]);
    }
    tools
}
fn note_fields() -> Value {
    json!({
        "name":{"type":"string"},
        "content":{"type":"string","description":"Markdown source"},
        "tags":{"type":"array","items":{"type":"string"},"description":"Complete set of existing tag IDs from list_tags; [] clears tags"},
        "tasks":{"type":"array","items":{"type":"string"},"description":"Complete set of linked task IDs from list_tasks; [] unlinks all tasks"}
    })
}
fn task_fields(include_name: bool) -> Value {
    let date = json!({"anyOf":[
        {"type":"object","properties":{"value":{"type":"string","description":"YYYY-MM-DD or YYYY-MM-DD HH:MM:SS"},"timezone":{"type":"string","description":"IANA timezone, e.g. Europe/Budapest"}},"required":["value","timezone"],"additionalProperties":false},
        {"type":"null"}
    ]});
    let mut properties = json!({
        "description":{"type":"string"},
        "status":{"type":"string","enum":["Pending","Wip","Done","OnHold","WontDo","Someday","Event"]},
        "start":date,"end":date,"deadline":date,"planned":date,"completion":date,
        "tags":{"type":"array","items":{"type":"string"},"description":"Existing tag IDs from list_tags"},
        "parents":{"type":"array","items":{"type":"string"},"description":"Existing task IDs; cycles are rejected"},
        "note_ids":{"type":"array","items":{"type":"string"},"description":"Complete set of linked note IDs from list_notes; [] unlinks all notes. Omit to preserve links."},
        "participants":{"type":"array","items":{"type":"string"}},
        "is_recurring":{"type":"boolean"},
        "rrule":{"type":"string","description":"RFC 5545 RRULE with exactly one finite COUNT or UNTIL"},
        "reminder":{"anyOf":[{"type":"integer","minimum":0,"maximum":525600},{"type":"null"}],"description":"Minutes before the scheduled time"},
        "reminded":{"type":"boolean"},
        "priority":{"type":"integer","minimum":1,"maximum":5},
        "severity":{"type":"integer","minimum":1,"maximum":5},
        "effort":{"type":"integer","minimum":0}
    });
    if include_name {
        properties["name"] = json!({"type":"string"});
    }
    json!({"type":"object","properties":properties,"additionalProperties":false})
}

fn tool(name: &str, description: &str, properties: Value, required: &[&str], read: bool) -> Value {
    json!({"name":name,"description":description,"inputSchema":{"type":"object","properties":properties,"required":required,"additionalProperties":false},"annotations":{"readOnlyHint":read,"destructiveHint":name.starts_with("delete_"),"idempotentHint":read,"openWorldHint":false}})
}
fn task_id(args: &Value) -> Result<Uuid> {
    record_id(args, "task")
}
fn record_id(args: &Value, kind: &str) -> Result<Uuid> {
    Uuid::parse_str(
        args["id"]
            .as_str()
            .ok_or_else(|| invalid("id is required"))?,
    )
    .map_err(|_| invalid(format!("Invalid {kind} ID")))
}
fn find_task(document: &Document, args: &Value) -> Result<Item> {
    let id = task_id(args)?;
    document
        .items()?
        .into_iter()
        .find(|item| item.id == id)
        .ok_or_else(|| invalid("Task not found"))
}
fn optional_id(args: &Value, key: &str) -> Result<Option<Uuid>> {
    args.get(key)
        .map(|value| {
            let value = value
                .as_str()
                .ok_or_else(|| invalid(format!("{key} must be an ID string")))?;
            Uuid::parse_str(value).map_err(|_| invalid(format!("Invalid {key}")))
        })
        .transpose()
}
fn linked_notes(item_id: Uuid, notes: &[Note]) -> Value {
    json!(
        notes
            .iter()
            .filter(|note| note.tasks.contains(&item_id))
            .map(|note| note.id)
            .collect::<Vec<_>>()
    )
}
fn task_value(item: &Item, notes: &[Note]) -> Result<Value> {
    let mut value = serde_json::to_value(item)?;
    value["note_ids"] = linked_notes(item.id, notes);
    Ok(value)
}
fn patch_note(note: Note, fields: &serde_json::Map<String, Value>) -> Result<Note> {
    let mut value = serde_json::to_value(note)?;
    for (key, field) in fields {
        if key == "id" || !value.as_object().unwrap().contains_key(key) {
            return Err(invalid(format!("Cannot set field {key}")));
        }
        value[key] = field.clone();
    }
    Ok(serde_json::from_value(value)?)
}
fn page(args: &Value) -> Result<(usize, usize)> {
    let number = |key: &str, default| -> Result<usize> {
        match args.get(key) {
            None => Ok(default),
            Some(v) => v
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| invalid(format!("{key} must be a non-negative integer"))),
        }
    };
    let offset = number("offset", 0)?;
    let limit = number("limit", 100)?;
    if !(1..=200).contains(&limit) {
        return Err(invalid("limit must be between 1 and 200"));
    }
    Ok((offset, limit))
}
fn paginated<T: serde::Serialize>(values: &[T], args: &Value) -> Result<Value> {
    let (offset, limit) = page(args)?;
    let end = offset.saturating_add(limit).min(values.len());
    let rows = if offset < values.len() {
        &values[offset..end]
    } else {
        &[]
    };
    Ok(
        json!({"items":rows,"total":values.len(),"next_offset":if end < values.len() { Some(end) } else { None }}),
    )
}
fn call(document: &mut Document, name: &str, args: &Value) -> Result<Value> {
    if !args.is_object() {
        return Err(invalid("arguments must be an object"));
    }
    match name {
        "list_tasks" => {
            let mut items = document.items()?;
            if let Some(search) = args.get("search") {
                let search = search
                    .as_str()
                    .ok_or_else(|| invalid("search must be text"))?
                    .to_lowercase();
                items.retain(|i| {
                    i.name.to_lowercase().contains(&search)
                        || i.description.to_lowercase().contains(&search)
                });
            }
            if let Some(status) = args.get("status") {
                let status: Status = serde_json::from_value(status.clone())?;
                items.retain(|i| i.status == status);
            }
            let mut result = paginated(&items, args)?;
            let (offset, _) = page(args)?;
            let notes = document.notes()?;
            for (value, item) in result["items"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .zip(items.iter().skip(offset))
            {
                value["note_ids"] = linked_notes(item.id, &notes);
            }
            Ok(result)
        }
        "list_tags" => paginated(&document.tags()?, args),
        "list_notes" => {
            let search = match args.get("search") {
                None => "",
                Some(value) => value
                    .as_str()
                    .ok_or_else(|| invalid("search must be text"))?,
            };
            paginated(
                &document.search_notes_filtered(
                    search,
                    optional_id(args, "tag_id")?,
                    optional_id(args, "task_id")?,
                )?,
                args,
            )
        }
        "get_note" => Ok(serde_json::to_value(
            document.note(record_id(args, "note")?)?,
        )?),
        "create_note" => {
            let note = Note::new(
                args["name"]
                    .as_str()
                    .ok_or_else(|| invalid("name is required"))?,
            );
            let note = patch_note(note, args.as_object().unwrap())?;
            Ok(serde_json::to_value(document.save_note(note)?)?)
        }
        "update_note" => {
            let note = document.note(record_id(args, "note")?)?;
            let fields = args["fields"]
                .as_object()
                .ok_or_else(|| invalid("fields must be an object"))?;
            Ok(serde_json::to_value(
                document.save_note(patch_note(note, fields)?)?,
            )?)
        }
        "delete_note" => {
            document.delete_note(record_id(args, "note")?)?;
            Ok(json!({"deleted":true}))
        }
        "get_task" => task_value(&find_task(document, args)?, &document.notes()?),
        "delete_task" => {
            document.delete_item(task_id(args)?)?;
            Ok(json!({"deleted":true}))
        }
        "create_task" | "update_task" => {
            let item = if name == "create_task" {
                Item::new(
                    args["name"]
                        .as_str()
                        .ok_or_else(|| invalid("name is required"))?,
                )
            } else {
                find_task(document, args)?
            };
            let mut value = serde_json::to_value(item)?;
            let empty = json!({});
            let fields = args
                .get("fields")
                .unwrap_or(&empty)
                .as_object()
                .ok_or_else(|| invalid("fields must be an object"))?;
            let note_ids = fields
                .get("note_ids")
                .map(|ids| serde_json::from_value::<Vec<Uuid>>(ids.clone()))
                .transpose()?;
            for (key, field) in fields {
                if key == "note_ids" {
                    continue;
                }
                if key == "id"
                    || !value.as_object().unwrap().contains_key(key)
                    || (name == "create_task" && key == "name")
                {
                    return Err(invalid(format!("Cannot set field {key}")));
                }
                value[key] = field.clone();
            }
            let item = serde_json::from_value(value)?;
            let item = match note_ids {
                Some(ids) => document.save_item_with_notes(item, ids)?,
                None => document.save_item(item)?,
            };
            task_value(&item, &document.notes()?)
        }
        _ => Err(invalid("Unknown tool")),
    }
}

fn respond(stream: &mut TcpStream, status: u16, value: Option<&Value>) -> std::io::Result<()> {
    let body = value.map(Value::to_string).unwrap_or_default();
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        _ => "Service Unavailable",
    };
    write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\nAllow: POST\r\n\r\n{body}",
        body.len()
    )?;
    // Rejecting a request before consuming its body must still deliver the
    // complete HTTP error. Closing with unread TCP data can instead send RST.
    stream.shutdown(std::net::Shutdown::Write)?;
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    let until = Instant::now() + Duration::from_millis(100);
    let mut drained = 0;
    let mut buffer = [0; 4096];
    while drained < 65_536 && Instant::now() < until {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(n) => drained += n,
        }
    }
    Ok(())
}
fn serve(
    stream: &mut TcpStream,
    address: SocketAddr,
    token: &str,
    sender: &mpsc::SyncSender<Request>,
    stop: &AtomicBool,
) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(500)))?;
    stream.set_write_timeout(Some(Duration::from_millis(500)))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut bytes = Vec::new();
    let header_end = loop {
        if stop.load(Ordering::Relaxed) || Instant::now() > deadline {
            return Ok(());
        }
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            return Ok(());
        }
        bytes.extend_from_slice(&buffer[..n]);
        if let Some(index) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break index + 4;
        }
        if bytes.len() > 16_384 {
            return respond(stream, 413, None);
        }
    };
    if header_end > 16_384 {
        return respond(stream, 413, None);
    }
    let Ok(header) = std::str::from_utf8(&bytes[..header_end]) else {
        return respond(stream, 400, None);
    };
    let mut lines = header.split("\r\n");
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let method = first.next().unwrap_or("");
    let path = first.next().unwrap_or("");
    let mut headers = std::collections::HashMap::new();
    for line in lines.filter(|l| !l.is_empty()) {
        let Some((key, value)) = line.split_once(':') else {
            return respond(stream, 400, None);
        };
        if headers
            .insert(key.to_ascii_lowercase(), value.trim())
            .is_some()
        {
            return respond(stream, 400, None);
        }
    }
    let hosts = [address.to_string(), format!("localhost:{}", address.port())];
    if !headers
        .get("host")
        .is_some_and(|host| hosts.contains(&host.to_string()))
    {
        return respond(stream, 403, None);
    }
    if headers
        .get("origin")
        .is_some_and(|origin| !hosts.iter().any(|host| *origin == format!("http://{host}")))
    {
        return respond(stream, 403, None);
    }
    if headers.get("authorization").copied() != Some(format!("Bearer {token}").as_str()) {
        return respond(stream, 401, None);
    }
    if path != "/mcp" {
        return respond(stream, 404, None);
    }
    if method != "POST" {
        return respond(stream, 405, None);
    }
    if headers.contains_key("transfer-encoding") {
        return respond(stream, 400, None);
    }
    if !headers
        .get("content-type")
        .is_some_and(|v| v.split(';').next() == Some("application/json"))
    {
        return respond(stream, 415, None);
    }
    if let Some(version) = headers.get("mcp-protocol-version")
        && !PROTOCOL_VERSIONS.contains(version)
    {
        return respond(stream, 400, None);
    }
    let Some(length) = headers
        .get("content-length")
        .and_then(|v| v.parse::<usize>().ok())
    else {
        return respond(stream, 400, None);
    };
    if length > 1_048_576 {
        return respond(stream, 413, None);
    }
    let total = header_end + length;
    while bytes.len() < total {
        if stop.load(Ordering::Relaxed) || Instant::now() > deadline {
            return Ok(());
        }
        let mut buffer = [0; 4096];
        let n = stream.read(&mut buffer)?;
        if n == 0 {
            return respond(stream, 400, None);
        }
        bytes.extend_from_slice(&buffer[..n]);
    }
    let value: Value = match serde_json::from_slice(&bytes[header_end..total]) {
        Ok(v) => v,
        Err(_) => {
            return respond(
                stream,
                400,
                Some(&rpc_error(Value::Null, -32700, "Parse error")),
            );
        }
    };
    if !value.is_object() || value["jsonrpc"] != "2.0" {
        return respond(
            stream,
            400,
            Some(&rpc_error(Value::Null, -32600, "Invalid request")),
        );
    }
    if value.get("id").is_none() {
        if value["method"]
            .as_str()
            .is_some_and(|m| m.starts_with("notifications/"))
            || (value.get("result").is_some() || value.get("error").is_some())
        {
            return respond(stream, 202, None);
        }
        return respond(stream, 400, None);
    }
    let (reply, receive) = mpsc::sync_channel(1);
    let active = Arc::new(AtomicBool::new(true));
    if sender
        .try_send(Request {
            value,
            reply,
            active: active.clone(),
        })
        .is_err()
    {
        return respond(stream, 503, None);
    }
    for _ in 0..250 {
        if stop.load(Ordering::Acquire) {
            break;
        }
        match receive.recv_timeout(Duration::from_millis(20)) {
            Ok(value) => return respond(stream, 200, Some(&value)),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(_) => {}
        }
    }
    active.store(false, Ordering::Release);
    respond(stream, 503, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_requests_cannot_write_and_poll_batches_are_bounded() {
        let (sender, receiver) = mpsc::sync_channel(32);
        let server = Server {
            address: "127.0.0.1:0".parse().unwrap(),
            config: Config {
                allow_edits: true,
                ..Config::default()
            },
            receiver,
            stop: Arc::new(AtomicBool::new(false)),
            worker: None,
        };
        let mut replies = Vec::new();
        for index in 0..20 {
            let (reply, receive) = mpsc::sync_channel(1);
            sender.send(Request {
                value: json!({"jsonrpc":"2.0","id":index,"method":"tools/call","params":{"name":"create_task","arguments":{"name":format!("Task {index}")}}}),
                reply,
                active: Arc::new(AtomicBool::new(index != 0)),
            }).unwrap();
            replies.push(receive);
        }
        let mut doc = Document::new().unwrap();
        assert!(server.poll(&mut doc, true));
        assert_eq!(doc.items().unwrap().len(), 15);
        assert!(matches!(
            replies[0].try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        ));
        assert!(matches!(
            replies[16].try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        // The next batch must recheck local edit safety instead of inheriting
        // permission from the earlier poll.
        assert!(!server.poll(&mut doc, false));
        for reply in &replies[16..] {
            assert_eq!(reply.try_recv().unwrap()["result"]["isError"], true);
        }
        let items = doc.items().unwrap();
        assert_eq!(items.len(), 15);
        assert!(!items.iter().any(|item| item.name == "Task 0"));
    }
}
