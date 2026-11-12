use antodo::core::{
    Document, Item, Moment, Status,
    mcp::{self, Config, Server},
};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    thread,
    time::{Duration, Instant},
};

fn rpc(doc: &mut Document, name: &str, args: Value, edits: bool, clear: bool) -> (Value, bool) {
    mcp::dispatch(
        doc,
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":args}}),
        edits,
        clear,
    )
}
fn content(value: &Value) -> Value {
    serde_json::from_str(value["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}
#[test]
fn initialize_catalog_and_json_rpc_errors() {
    let mut doc = Document::new().unwrap();
    for (requested, expected) in [
        ("2025-11-25", "2025-11-25"),
        ("2025-06-18", "2025-06-18"),
        ("2025-03-26", "2025-03-26"),
        ("unknown", "2025-11-25"),
    ] {
        let (reply, changed) = mcp::dispatch(
            &mut doc,
            &json!({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":requested}}),
            false,
            true,
        );
        assert_eq!(reply["result"]["protocolVersion"], expected);
        assert_eq!(reply["result"]["serverInfo"]["version"], "1.0.0");
        assert!(!changed);
    }
    for (edit, count) in [(false, 5), (true, 11)] {
        let (reply, _) = mcp::dispatch(
            &mut doc,
            &json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            edit,
            true,
        );
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), count);
        assert_eq!(tools[0]["annotations"]["readOnlyHint"], true);
    }
    let (reply, _) = mcp::dispatch(
        &mut doc,
        &json!({"jsonrpc":"2.0","id":3,"method":"unknown"}),
        false,
        true,
    );
    assert_eq!(reply["error"]["code"], -32601);
    let (reply, _) = mcp::dispatch(&mut doc, &json!({"id":4}), false, true);
    assert_eq!(reply["error"]["code"], -32600);
}
#[test]
fn note_tools_support_markdown_crud_search_and_explicit_pagination() {
    let mut doc = Document::new().unwrap();
    let markdown = "# Project\n\n**Unchanged Markdown**\n- [ ] Work";
    let (reply, changed) = rpc(
        &mut doc,
        "create_note",
        json!({"name":"Alpha","content":markdown}),
        true,
        true,
    );
    assert!(changed);
    let note = content(&reply);
    let id = note["id"].clone();
    assert_eq!(note["content"], markdown);
    assert_eq!(
        uuid::Uuid::parse_str(id.as_str().unwrap())
            .unwrap()
            .get_version_num(),
        7
    );
    for name in ["Beta", "Gamma"] {
        let (_, changed) = rpc(&mut doc, "create_note", json!({"name":name}), true, true);
        assert!(changed);
    }
    let (reply, changed) = rpc(&mut doc, "get_note", json!({"id":id}), false, false);
    assert!(!changed);
    assert_eq!(content(&reply), note);
    let (reply, _) = rpc(
        &mut doc,
        "list_notes",
        json!({"search":"UNCHANGED"}),
        false,
        false,
    );
    assert_eq!(content(&reply)["total"], 1);
    assert_eq!(content(&reply)["items"][0], note);
    let (reply, _) = rpc(&mut doc, "list_notes", json!({"limit":2}), false, true);
    assert_eq!(content(&reply)["total"], 3);
    assert_eq!(content(&reply)["next_offset"], 2);
    let (reply, _) = rpc(
        &mut doc,
        "list_notes",
        json!({"offset":2,"limit":2}),
        false,
        true,
    );
    assert_eq!(content(&reply)["items"].as_array().unwrap().len(), 1);
    assert!(content(&reply)["next_offset"].is_null());
    let (reply, changed) = rpc(
        &mut doc,
        "update_note",
        json!({"id":id,"fields":{"name":"Renamed"}}),
        true,
        true,
    );
    assert!(changed);
    assert_eq!(content(&reply)["id"], id);
    assert_eq!(content(&reply)["name"], "Renamed");
    assert_eq!(content(&reply)["content"], markdown);
    let (reply, _) = rpc(
        &mut doc,
        "update_note",
        json!({"id":id,"fields":{"content":""}}),
        true,
        true,
    );
    assert_eq!(content(&reply)["name"], "Renamed");
    assert_eq!(content(&reply)["content"], "");
    let (reply, changed) = rpc(&mut doc, "delete_note", json!({"id":id}), true, true);
    assert!(changed);
    assert_eq!(content(&reply)["deleted"], true);
    assert_eq!(doc.notes().unwrap().len(), 2);
}

#[test]
fn note_tools_respect_read_only_and_local_draft_guards() {
    let mut doc = Document::new().unwrap();
    let saved = doc.save_note(antodo::core::Note::new("Keep me")).unwrap();
    for (allow, clear) in [(false, true), (true, false)] {
        for (tool, args) in [
            ("create_note", json!({"name":"Blocked"})),
            (
                "update_note",
                json!({"id":saved.id,"fields":{"name":"Blocked"}}),
            ),
            ("delete_note", json!({"id":saved.id})),
        ] {
            let (reply, changed) = rpc(&mut doc, tool, args, allow, clear);
            assert_eq!(reply["result"]["isError"], true);
            assert!(!changed);
            assert_eq!(doc.notes().unwrap(), std::slice::from_ref(&saved));
        }
    }
    for edits in [false, true] {
        let (reply, _) = mcp::dispatch(
            &mut doc,
            &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
            edits,
            true,
        );
        let tools = reply["result"]["tools"].as_array().unwrap();
        for name in ["list_notes", "get_note"] {
            assert_eq!(
                tools.iter().find(|tool| tool["name"] == name).unwrap()["annotations"]["readOnlyHint"],
                true
            );
        }
        let delete = tools.iter().find(|tool| tool["name"] == "delete_note");
        assert_eq!(delete.is_some(), edits);
        if let Some(delete) = delete {
            assert_eq!(delete["annotations"]["destructiveHint"], true);
        }
    }
}

#[test]
fn invalid_note_calls_preserve_records() {
    let mut doc = Document::new().unwrap();
    let saved = doc.save_note(antodo::core::Note::new("Keep me")).unwrap();
    for (tool, args) in [
        ("create_note", json!({"name":" "})),
        ("create_note", json!({"name":"Wrong type","content":false})),
        (
            "update_note",
            json!({"id":saved.id,"fields":{"name":" ","content":"Must not apply"}}),
        ),
        (
            "update_note",
            json!({"id":saved.id,"fields":{"name":"Changed","id":uuid::Uuid::now_v7()}}),
        ),
        (
            "update_note",
            json!({"id":saved.id,"fields":{"content":null}}),
        ),
        ("update_note", json!({"id":saved.id,"fields":[]})),
        ("update_note", json!({"id":saved.id})),
        ("get_note", json!({"id":"invalid"})),
        ("get_note", json!({})),
        ("get_note", json!({"id":uuid::Uuid::now_v7()})),
        ("delete_note", json!({"id":uuid::Uuid::now_v7()})),
        ("list_notes", json!({"search":false})),
        ("list_notes", json!({"limit":201})),
        ("list_notes", json!({"offset":-1})),
    ] {
        let (reply, changed) = rpc(&mut doc, tool, args, true, true);
        assert_eq!(reply["result"]["isError"], true, "{tool}: {reply}");
        assert!(!changed);
        assert_eq!(doc.notes().unwrap(), std::slice::from_ref(&saved));
    }
}

#[test]
fn task_lookup_errors_preserve_applied_state() {
    let mut doc = Document::new().unwrap();
    let saved = doc.save_item(Item::new("Keep me")).unwrap();
    for name in ["get_task", "update_task"] {
        for (id, expected) in [
            (Value::Null, "id is required"),
            (json!("bad-id"), "Invalid task ID"),
            (json!(uuid::Uuid::now_v7()), "Task not found"),
        ] {
            let (reply, changed) = rpc(
                &mut doc,
                name,
                json!({"id":id,"fields":{"name":"Must not apply"}}),
                true,
                true,
            );
            assert_eq!(reply["result"]["isError"], true);
            assert!(
                reply["result"]["content"][0]["text"]
                    .as_str()
                    .unwrap()
                    .contains(expected)
            );
            assert!(!changed);
            assert_eq!(doc.items().unwrap(), std::slice::from_ref(&saved));
        }
    }
}
#[test]
fn task_tools_validate_mutations_and_protect_local_edits() {
    let mut doc = Document::new().unwrap();
    let args = json!({"name":"Appointment","fields":{"status":"Event","start":{"value":"2030-01-01 09:00","timezone":"Europe/Budapest"},"end":{"value":"2030-01-01 10:00","timezone":"Europe/Budapest"}}});
    for (allow, clear) in [(false, true), (true, false)] {
        let (reply, changed) = rpc(&mut doc, "create_task", args.clone(), allow, clear);
        assert_eq!(reply["result"]["isError"], true);
        assert!(!changed);
        assert!(doc.items().unwrap().is_empty());
    }
    let (reply, changed) = rpc(&mut doc, "create_task", args, true, true);
    assert!(changed);
    let id = content(&reply)["id"].clone();
    assert!(doc.is_dirty());
    assert_eq!(doc.items().unwrap()[0].status, Status::Event);
    let (reply, _) = rpc(&mut doc, "get_task", json!({"id":id}), false, true);
    assert_eq!(content(&reply)["name"], "Appointment");
    let (reply, _) = rpc(
        &mut doc,
        "update_task",
        json!({"id":id,"fields":{"name":"Revised","status":"Wip","description":"Notes"}}),
        true,
        true,
    );
    assert_eq!(content(&reply)["name"], "Revised");
    for fields in [
        json!({"id":id}),
        json!({"priority":6}),
        json!({"start":null}),
        json!({"is_recurring":true,"rrule":"FREQ=DAILY"}),
    ] {
        let before = doc.items().unwrap();
        let (reply, changed) = rpc(
            &mut doc,
            "update_task",
            json!({"id":id,"fields":fields}),
            true,
            true,
        );
        assert_eq!(reply["result"]["isError"], true);
        assert!(!changed);
        assert_eq!(doc.items().unwrap(), before);
    }
    let (reply, _) = rpc(&mut doc, "delete_task", json!({"id":id}), true, true);
    assert_eq!(content(&reply)["deleted"], true);
    assert!(doc.items().unwrap().is_empty());
}
#[test]
fn pagination_and_filtering_do_not_silently_drop_tasks() {
    let mut doc = Document::new().unwrap();
    for i in 0..5 {
        doc.save_item(Item::new(format!("Task {i}"))).unwrap();
    }
    let (reply, _) = rpc(&mut doc, "list_tasks", json!({"limit":2}), false, true);
    assert_eq!(content(&reply)["total"], 5);
    assert_eq!(content(&reply)["next_offset"], 2);
    let (reply, _) = rpc(
        &mut doc,
        "list_tasks",
        json!({"offset":4,"limit":2}),
        false,
        true,
    );
    assert_eq!(content(&reply)["items"].as_array().unwrap().len(), 1);
    assert!(content(&reply)["next_offset"].is_null());
    let (reply, _) = rpc(
        &mut doc,
        "list_tasks",
        json!({"search":"Task 3","status":"Pending"}),
        false,
        true,
    );
    assert_eq!(content(&reply)["total"], 1);
    for bad in [
        json!({"limit":201}),
        json!({"offset":-1}),
        json!({"status":"garbage"}),
    ] {
        let (reply, _) = rpc(&mut doc, "list_tasks", bad, false, true);
        assert_eq!(reply["result"]["isError"], true);
    }
}
#[test]
fn mcp_enforces_graph_completion_and_dst_rules() {
    let mut doc = Document::new().unwrap();
    let parent = doc.save_item(Item::new("Parent")).unwrap();
    let mut child = Item::new("Child");
    child.parents.push(parent.id);
    let child = doc.save_item(child).unwrap();
    for fields in [
        json!({"parents":[child.id]}),
        json!({"status":"Done"}),
        json!({"planned":{"value":"2026-03-29 02:30","timezone":"Europe/Budapest"}}),
    ] {
        let (reply, changed) = rpc(
            &mut doc,
            "update_task",
            json!({"id":parent.id,"fields":fields}),
            true,
            true,
        );
        assert_eq!(reply["result"]["isError"], true);
        assert!(!changed);
    }
    let explicit = Moment::parse("2030-01-01 12:34:56", "UTC").unwrap();
    let (reply, changed) = rpc(
        &mut doc,
        "update_task",
        json!({"id":child.id,"fields":{"status":"Done","completion":explicit}}),
        true,
        true,
    );
    assert!(changed);
    assert_eq!(
        content(&reply)["completion"]["value"],
        "2030-01-01 12:34:56"
    );
}
fn request(
    address: SocketAddr,
    token: &str,
    origin: Option<&str>,
    method: &str,
    value: &str,
) -> String {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(7)))
        .unwrap();
    let origin = origin
        .map(|v| format!("Origin: {v}\r\n"))
        .unwrap_or_default();
    write!(stream,"{method} /mcp HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\n{origin}Content-Type: application/json\r\nAccept: application/json, text/event-stream\r\nContent-Length: {}\r\n\r\n{value}", value.len()).unwrap();
    read_response(&mut stream)
}
fn read_response(stream: &mut TcpStream) -> String {
    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    while !response.ends_with(b"\r\n\r\n") {
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }
    let header = String::from_utf8(response.clone()).unwrap();
    let length: usize = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    response.extend_from_slice(&body);
    String::from_utf8(response).unwrap()
}

fn raw_request(address: SocketAddr, value: &[u8]) -> String {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(7)))
        .unwrap();
    stream.write_all(value).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    read_response(&mut stream)
}

#[test]
fn http_rejects_malformed_headers_bodies_and_untrusted_hosts() {
    let config = Config {
        port: 0,
        ..Config::default()
    };
    let server = Server::start(config.clone()).unwrap();
    let address = server.address;
    assert_eq!(server.url(), format!("http://{address}/mcp"));
    let valid = format!(
        "POST /mcp HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{{}}",
        config.token
    );
    for (request, status) in [
        (
            valid.replace(&format!("Host: {address}"), "Host: evil.example"),
            403,
        ),
        (valid.replace("/mcp", "/wrong"), 404),
        (
            valid.replace("Content-Type: application/json", "Content-Type: text/plain"),
            415,
        ),
        (
            valid.replace(
                "Content-Length: 2",
                "Transfer-Encoding: chunked\r\nContent-Length: 2",
            ),
            400,
        ),
        (
            valid.replace(
                "Content-Length: 2",
                "MCP-Protocol-Version: unknown\r\nContent-Length: 2",
            ),
            400,
        ),
        (valid.replace("Content-Length: 2\r\n", ""), 400),
        (
            valid.replace("Content-Length: 2", "Content-Length: -1"),
            400,
        ),
        (
            valid.replace("Content-Length: 2", "Content-Length: 1048577"),
            413,
        ),
        (valid.replace("Content-Length: 2", "Content-Length: 3"), 400),
        (
            valid.replace(
                "Content-Length: 2",
                "Content-Length: 2\r\ncontent-length: 2",
            ),
            400,
        ),
        (
            valid.replace("Content-Length: 2", "Malformed header\r\nContent-Length: 2"),
            400,
        ),
        (valid.replace("{}", "[]"), 400),
        (valid.replace("{}", "xx"), 400),
        (
            valid.replace(
                "Content-Length: 2",
                &format!("Extra: {}\r\nContent-Length: 2", "x".repeat(17_000)),
            ),
            413,
        ),
    ] {
        let response = raw_request(address, request.as_bytes());
        assert!(
            response.starts_with(&format!("HTTP/1.1 {status}")),
            "{response}"
        );
    }
    let mut invalid_utf8 = valid.into_bytes();
    invalid_utf8[0] = 0xff;
    assert!(raw_request(address, &invalid_utf8).starts_with("HTTP/1.1 400"));
    let unfinished_header = format!("POST /mcp HTTP/1.1\r\nExtra: {}", "x".repeat(17_000));
    assert!(raw_request(address, unfinished_header.as_bytes()).starts_with("HTTP/1.1 413"));
    for (body, status) in [
        (json!({"jsonrpc":"2.0","method":"ping"}), 400),
        (json!({"jsonrpc":"1.0","id":1,"method":"ping"}), 400),
        (json!({"jsonrpc":"2.0","result":{}}), 202),
        (json!({"jsonrpc":"2.0","error":{}}), 202),
    ] {
        assert!(
            request(address, &config.token, None, "POST", &body.to_string())
                .starts_with(&format!("HTTP/1.1 {status}"))
        );
    }
}

#[test]
fn malformed_tool_arguments_and_pagination_preserve_document_state() {
    let mut doc = Document::new().unwrap();
    let saved = doc.save_item(Item::new("Keep me")).unwrap();
    for (name, arguments) in [
        ("list_tasks", json!([])),
        ("list_tasks", json!({"search": 1})),
        ("list_tasks", json!({"limit": 0})),
        ("list_tasks", json!({"limit": "1"})),
        ("unknown", json!({})),
        ("create_task", json!({})),
        ("create_task", json!({"name":"Bad", "fields": []})),
        (
            "create_task",
            json!({"name":"Bad", "fields": {"name":"Override"}}),
        ),
        (
            "create_task",
            json!({"name":"Bad", "fields": {"unsupported":true}}),
        ),
    ] {
        let (reply, changed) = rpc(&mut doc, name, arguments, true, true);
        assert_eq!(reply["result"]["isError"], true, "{reply}");
        assert!(!changed);
        assert_eq!(doc.items().unwrap(), std::slice::from_ref(&saved));
    }
    for name in ["list_tasks", "list_tags"] {
        let (reply, changed) = rpc(&mut doc, name, json!({"offset":100}), false, true);
        assert!(!changed);
        assert_eq!(content(&reply)["items"], json!([]));
        assert!(content(&reply)["next_offset"].is_null());
    }
    for request in [
        json!({"jsonrpc":"2.0","id":{},"method":"ping"}),
        json!({"jsonrpc":"2.0","id":1,"method":1}),
        json!([]),
    ] {
        let (reply, changed) = mcp::dispatch(&mut doc, &request, true, true);
        assert_eq!(reply["error"]["code"], -32600);
        assert!(!changed);
    }
    let (reply, changed) = mcp::dispatch(
        &mut doc,
        &json!({"jsonrpc":"2.0","id":1,"method":"ping"}),
        false,
        true,
    );
    assert_eq!(reply["result"], json!({}));
    assert!(!changed);
}
#[test]
fn http_transport_auth_origin_handshake_tools_stop_and_restart() {
    let mut config = Config {
        port: 0,
        allow_edits: true,
        ..Default::default()
    };
    let mut server = Server::start(config.clone()).unwrap();
    let address = server.address;
    // write! may split headers/body across packets. Early rejection must not
    // reset the connection before the client receives its HTTP response.
    for _ in 0..10 {
        assert!(request(address, "wrong", None, "POST", "{}").starts_with("HTTP/1.1 401"));
    }
    assert!(
        request(
            address,
            &config.token,
            Some("http://evil.example"),
            "POST",
            "{}"
        )
        .starts_with("HTTP/1.1 403")
    );
    assert!(request(address, &config.token, None, "GET", "").starts_with("HTTP/1.1 405"));
    assert!(request(address, &config.token, None, "POST", "bad json").starts_with("HTTP/1.1 400"));
    assert!(
        request(
            address,
            &config.token,
            None,
            "POST",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#
        )
        .starts_with("HTTP/1.1 202")
    );
    let mut doc = Document::new().unwrap();
    for message in [
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25"}}),
        json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"create_task","arguments":{"name":"From HTTP"}}}),
    ] {
        let token = config.token.clone();
        let message = message.to_string();
        let client = thread::spawn(move || request(address, &token, None, "POST", &message));
        let until = Instant::now() + Duration::from_secs(3);
        while !client.is_finished() && Instant::now() < until {
            server.poll(&mut doc, true);
            thread::sleep(Duration::from_millis(5));
        }
        let response = client.join().unwrap();
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        let body: Value = serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
        assert!(body.get("result").is_some());
    }
    assert_eq!(doc.items().unwrap()[0].name, "From HTTP");
    server.stop();
    assert!(TcpStream::connect(address).is_err());
    config.port = address.port();
    let replacement = Server::start(config).unwrap();
    assert_eq!(replacement.address, address);
}
#[test]
fn invalid_configuration_and_occupied_port_fail_without_starting() {
    let bad = Config {
        address: "0.0.0.0".parse().unwrap(),
        ..Default::default()
    };
    assert!(Server::start(bad).is_err());
    let bad = Config {
        token: "short".into(),
        ..Default::default()
    };
    assert!(Server::start(bad).is_err());
    let first = Server::start(Config {
        port: 0,
        ..Default::default()
    })
    .unwrap();
    assert!(
        Server::start(Config {
            port: first.address.port(),
            ..Default::default()
        })
        .is_err()
    );
}

#[test]
fn note_relations_are_discoverable_filterable_and_editable_from_both_sides() {
    use antodo::core::{Note, Tag};
    let mut doc = Document::new().unwrap();
    let alpha = doc.save_item(Item::new("Alpha task")).unwrap();
    let beta = doc.save_item(Item::new("Beta task")).unwrap();
    let tag = doc.save_tag(Tag::new("Reference", "#A395FF")).unwrap();
    let (reply, changed) = rpc(
        &mut doc,
        "create_note",
        json!({
            "name":"Shared reference", "content":"# Shared Markdown", "tags":[tag.id], "tasks":[alpha.id,beta.id]
        }),
        true,
        true,
    );
    assert!(changed, "{reply}");
    let shared = content(&reply);
    assert_eq!(shared["tags"], json!([tag.id]));
    assert_eq!(shared["tasks"], json!([alpha.id, beta.id]));
    let shared_id: uuid::Uuid = serde_json::from_value(shared["id"].clone()).unwrap();
    let other = doc.save_note(Note::new("Other reference")).unwrap();
    for task in [&alpha, &beta] {
        let (reply, _) = rpc(&mut doc, "get_task", json!({"id":task.id}), false, true);
        assert_eq!(content(&reply)["note_ids"], json!([shared_id]));
    }
    let (reply, _) = rpc(
        &mut doc,
        "list_tasks",
        json!({"search":"Alpha"}),
        false,
        true,
    );
    assert_eq!(content(&reply)["items"][0]["note_ids"], json!([shared_id]));
    let (reply, _) = rpc(
        &mut doc,
        "list_notes",
        json!({"search":"MARKDOWN","tag_id":tag.id,"task_id":beta.id}),
        false,
        true,
    );
    assert_eq!(content(&reply)["items"], json!([shared]));
    let (reply, _) = rpc(
        &mut doc,
        "list_notes",
        json!({"task_id":uuid::Uuid::now_v7()}),
        false,
        true,
    );
    assert_eq!(content(&reply)["total"], 0);
    let (reply, changed) = rpc(
        &mut doc,
        "update_task",
        json!({"id":alpha.id,"fields":{"note_ids":[other.id],"name":"Renamed task"}}),
        true,
        true,
    );
    assert!(changed, "{reply}");
    assert_eq!(content(&reply)["note_ids"], json!([other.id]));
    assert_eq!(doc.note(shared_id).unwrap().tasks, [beta.id]);
    assert_eq!(doc.note(other.id).unwrap().tasks, [alpha.id]);
    let (reply, changed) = rpc(
        &mut doc,
        "update_note",
        json!({"id":shared_id,"fields":{"content":"Updated"}}),
        true,
        true,
    );
    assert!(changed, "{reply}");
    assert_eq!(content(&reply)["tags"], json!([tag.id]));
    assert_eq!(content(&reply)["tasks"], json!([beta.id]));
    let (_, changed) = rpc(
        &mut doc,
        "update_note",
        json!({"id":shared_id,"fields":{"tasks":[alpha.id,beta.id],"tags":[]}}),
        true,
        true,
    );
    assert!(changed);
    assert_eq!(doc.notes_for_task(alpha.id).unwrap().len(), 2);
    assert_eq!(doc.notes_for_task(beta.id).unwrap().len(), 1);
    assert!(doc.note(shared_id).unwrap().tags.is_empty());
    let (reply, changed) = rpc(
        &mut doc,
        "create_task",
        json!({"name":"Third task","fields":{"note_ids":[other.id]}}),
        true,
        true,
    );
    assert!(changed, "{reply}");
    assert_eq!(content(&reply)["note_ids"], json!([other.id]));
    assert_eq!(doc.note(other.id).unwrap().tasks.len(), 2);
}

#[test]
fn invalid_relation_patches_and_write_guards_leave_tasks_and_notes_unchanged() {
    use antodo::core::Note;
    let mut doc = Document::new().unwrap();
    let task = doc.save_item(Item::new("Keep task")).unwrap();
    let note = doc.save_note(Note::new("Keep note")).unwrap();
    let initial_items = doc.items().unwrap();
    let initial_notes = doc.notes().unwrap();
    let bad_calls = [
        (
            "update_task",
            json!({"id":task.id,"fields":{"name":"Must rollback","note_ids":[uuid::Uuid::now_v7()]}}),
        ),
        (
            "create_task",
            json!({"name":"Must rollback","fields":{"note_ids":[uuid::Uuid::now_v7()]}}),
        ),
        (
            "update_task",
            json!({"id":task.id,"fields":{"note_ids":null}}),
        ),
        (
            "update_note",
            json!({"id":note.id,"fields":{"content":"Must rollback","tasks":[uuid::Uuid::now_v7()]}}),
        ),
        (
            "update_note",
            json!({"id":note.id,"fields":{"content":"Must rollback","tags":[uuid::Uuid::now_v7()]}}),
        ),
        (
            "update_note",
            json!({"id":note.id,"fields":{"tasks":false}}),
        ),
        (
            "create_note",
            json!({"name":"Bad note","tags":["not-an-id"]}),
        ),
        ("list_notes", json!({"tag_id":false})),
        ("list_notes", json!({"task_id":"not-an-id"})),
    ];
    for (name, args) in bad_calls {
        let (reply, changed) = rpc(&mut doc, name, args, true, true);
        assert_eq!(reply["result"]["isError"], true, "{name}: {reply}");
        assert!(!changed);
        assert_eq!(doc.items().unwrap(), initial_items);
        assert_eq!(doc.notes().unwrap(), initial_notes);
    }
    for (allowed, clear) in [(false, true), (true, false)] {
        for (name, args) in [
            (
                "update_task",
                json!({"id":task.id,"fields":{"note_ids":[note.id]}}),
            ),
            (
                "update_note",
                json!({"id":note.id,"fields":{"tasks":[task.id]}}),
            ),
        ] {
            let (reply, changed) = rpc(&mut doc, name, args, allowed, clear);
            assert_eq!(reply["result"]["isError"], true);
            assert!(!changed);
            assert_eq!(doc.items().unwrap(), initial_items);
            assert_eq!(doc.notes().unwrap(), initial_notes);
        }
    }
}

#[test]
fn mcp_catalog_advertises_note_relationship_fields() {
    let mut doc = Document::new().unwrap();
    let (reply, _) = mcp::dispatch(
        &mut doc,
        &json!({"jsonrpc":"2.0","id":1,"method":"tools/list"}),
        true,
        true,
    );
    let tools = reply["result"]["tools"].as_array().unwrap();
    let schema = |name: &str| {
        &tools.iter().find(|tool| tool["name"] == name).unwrap()["inputSchema"]["properties"]
    };
    for key in ["tags", "tasks"] {
        assert_eq!(schema("create_note")[key]["type"], "array");
        assert_eq!(
            schema("update_note")["fields"]["properties"][key]["type"],
            "array"
        );
    }
    for key in ["tag_id", "task_id"] {
        assert_eq!(schema("list_notes")[key]["type"], "string");
    }
    assert_eq!(
        schema("update_task")["fields"]["properties"]["note_ids"]["type"],
        "array"
    );
}
