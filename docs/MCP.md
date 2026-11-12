# Connect to antodo MCP

Start antodo 1.0.0, open/create a workspace, and choose **MCP**. The server shares that workspace's working document, including applied unsaved changes. Unsaved text in an editor is not exposed. It does not automatically start with the app.

1. Keep Bind address at `127.0.0.1` (IPv6 `::1` is also supported). Port defaults to `8765`; use `0` for a free port and copy the actual running URL.
2. Keep the generated token or supply at least 16 letters/digits/hyphens/underscores. **Copy token** copies it; it is masked and never written to preferences. Keep it private.
3. Read-only is the default. Enable editing before starting to offer create/update/delete tools.
4. Select **Start**. On errors such as an occupied port, change settings and retry. Select **Stop** before changing settings. Closing or replacing the file stops the server; switching pages does not.

## Codex on this machine

Copy the generated configuration from the MCP page into Codex's `config.toml`:

```toml
[mcp_servers.antodo]
url = "http://127.0.0.1:8765/mcp"
bearer_token_env_var = "ANTODO_MCP_TOKEN"
```

Set `ANTODO_MCP_TOKEN` to the copied token in the environment that launches Codex, then restart/reconnect it. A terminal-launched Codex inherits that shell's environment; an already-running desktop app will not inherit a later shell change. Ask it to list your antodo tasks. Recopy the token if antodo was restarted. The page's config uses the actual selected port while running.

For another local client, choose **Streamable HTTP**, use the running URL, and configure `Authorization: Bearer <copied token>`. This transport does not implement stdio or the older HTTP+SSE transport.

## ChatGPT / cloud clients

Cloud services cannot directly reach `127.0.0.1` on your computer. Configure a supported private MCP tunnel or authenticated gateway separately. It must reach antodo and supply the upstream bearer token; a tunnel URL alone does not remove antodo authentication. Antodo does not implement OAuth or provision tunnel accounts.

For ChatGPT, follow the current [Secure MCP Tunnel setup](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels): create/associate the tunnel with your workspace, run its client on the machine that can reach antodo, configure upstream authentication, verify readiness, and add the tunnel as a custom MCP server. Feature availability and authentication settings depend on the client and workspace. If your connector only accepts OAuth, use a compatible authenticated gateway that supplies antodo's upstream token. Keep antodo bound to loopback.

A gateway must preserve the upstream Host expected by antodo and must not forward an unrelated browser Origin; antodo accepts absent Origin or its own local HTTP origin. GET/SSE is optional and returns 405; JSON POST responses are supported.

## Tools and changes

- `list_tasks`: `search`, serialized `status`, `offset`, `limit` (default 100; maximum 200). Returns `items`, `total`, `next_offset`; each task includes `note_ids` for its linked Notes.
- `get_task`: internal `id` returned by list/create tools; includes `note_ids` for discovering related Notes.
- `list_tags`: `offset`/`limit` pagination with IDs usable in task and Note fields.
- With editing enabled: `create_task(name, fields?)`, `update_task(id, fields)`, `delete_task(id)`. The optional `fields.note_ids` array replaces that task's linked Notes in the same transaction as task field changes; omitted preserves links and `[]` unlinks all Notes.
- `list_notes`: case-insensitive literal `search` across names and Markdown content, optional `tag_id` and `task_id` filters, plus `offset`/`limit` pagination (the same limits and result shape as `list_tasks`). Search and filters compose.
- `get_note`: internal `id` returned by list/create tools; returns `id`, `name`, the original Markdown `content`, `tags` (tag IDs), and `tasks` (linked task IDs).
- With editing enabled: `create_note(name, content?, tags?, tasks?)`, `update_note(id, fields)`, `delete_note(id)`. Note patches accept `name`, `content`, `tags`, and `tasks`; omitted fields are preserved, an empty content string clears the body, and `[]` clears a relationship set. IDs cannot be changed.

Status values use serialized model names: `Pending`, `Wip`, `Done`, `OnHold`, `WontDo`, `Someday`, `Event`. Optional dates use `null` or objects such as `{"value":"2030-01-10 09:30:00","timezone":"Europe/Budapest"}`. Get a task to inspect the remaining fields. Omitted patch fields are preserved; internal IDs cannot be changed. Relationships/tags use IDs returned by tools, not names.

Core commands validate graph cycles, completion dependencies, recurrence bounds, date pairs/timezones, DST ambiguity and ratings. Notes require a nonblank name, preserve Markdown source verbatim, and accept only existing tag/task IDs. A Note may link to several tasks, and each task may link to several Notes. Deleting a tag/task removes its links while retaining Notes; deleting a Note leaves its tasks and tags intact. Note and task-side relationship patches are transactional. Invalid commands leave the working document intact. Event clears its inapplicable fields. External mutations are refused while a local editor (including a note draft) or modal is open or a tag draft exists; finish/close these first. Read calls remain available.

Successful tool mutations appear in the app and set Unsaved changes. Select **Save** to keep them on disk. MCP has no file-open/save/overwrite tools, so it cannot bypass workspace selection or unsaved-file guards.


For example, first resolve the tag and task IDs with `list_tags` and `list_tasks`, then call:

```json
{"name":"Design reference","content":"# Decisions\n\nKeep the rationale here.","tags":["TAG_ID"],"tasks":["TASK_A_ID","TASK_B_ID"]}
```

Pass this as `create_note` arguments. Later, `list_notes({"task_id":"TASK_A_ID"})` finds that task's Notes. To add another task through `update_note`, read the current Note and include the existing task IDs in the replacement `fields.tasks` array. Use `update_task` with `fields.note_ids` to manage the same links from the task side. Missing references or invalid task fields reject the whole patch.

## Protocol verification

The endpoint supports MCP initialize, ping, notifications/initialized, tools/list and tools/call, using stateless JSON-RPC over Streamable HTTP. Supported protocol versions: 2025-03-26, 2025-06-18 and 2025-11-25. It returns JSON rather than an SSE stream. Include the token, `Content-Type: application/json` and `Accept: application/json, text/event-stream` in requests.

References: [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [Codex configuration](https://learn.chatgpt.com/docs/extend/mcp), [OpenAI private tunnels](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels).
