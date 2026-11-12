# Embedded LLM chat

Open or create an antodo workspace, then choose **LLM** in navigation. Choose a provider, fill its connection fields and select **Use this connection**. This validates settings; the first **Send** makes the connection. Configuration, keys and history are session state and are never saved in the task database or preferences. Provider workers use private temporary request, credential, prompt and response files, removed when the worker ends.

**Start chat** appears on the workspace header and in task and Note editors. Editor chat carries an explicitly typed task or Note context, so the assistant can look up the correct record and its applied relationships. New, unapplied records do not yet exist in the database; apply them before starting record-specific chat. Apply draft edits before asking chat to change tasks or Notes. Chat can still read applied data while drafts are open.

## Providers

### Ollama (local)

1. Install [Ollama](https://docs.ollama.com/quickstart) and ensure its service is running (`ollama serve` when needed).
2. Install a model supporting tool calling, for example `ollama pull qwen3:8b`.
3. Choose Ollama, use `http://127.0.0.1:11434/v1/chat/completions`, and enter the exact installed model name. No API key is needed for the normal local service.
4. Select Use this connection, open chat and ask “List my current tasks.”

Antodo uses Ollama's [OpenAI-compatible API](https://docs.ollama.com/api/openai-compatibility). Model quality, tool support, context capacity and hardware affect results. HTTP providers require `curl` on PATH. HTTPS is required for remote endpoints; plain HTTP is restricted to loopback. Compatible local servers can use the same provider settings with their own URL/model.

### OpenAI API

Create an API key in the [OpenAI developer platform](https://platform.openai.com/api-keys), choose OpenAI API and enter the key, a model supporting Chat Completions and function tools, and `https://api.openai.com/v1/chat/completions`. The default model is editable. API usage has separate billing from a ChatGPT subscription. See [function calling](https://developers.openai.com/api/docs/guides/function-calling).

### Gemini API

Create an API key in [Google AI Studio](https://aistudio.google.com/apikey). Choose Gemini API, enter the key and a tool-capable model available to your account, and use `https://generativelanguage.googleapis.com/v1beta/openai/chat/completions`. See Google's [OpenAI compatibility documentation](https://ai.google.dev/gemini-api/docs/openai). Antodo uses this compatibility API rather than the native Gemini request format.

### Local Codex CLI

Install the official `codex` command and run `codex login` as the same operating-system user running antodo. Choose Codex CLI; URL/key are unused and an empty model uses the CLI default. Antodo starts a fresh `codex exec` process per provider round, supplying the bounded session conversation and task-and-Note tool catalog. It does not attach to a running Codex Desktop conversation.

The CLI must support `--ephemeral`, `--ignore-user-config`, `--ignore-rules`, `--output-schema`, `--json`, and read-only sandbox mode. Antodo runs it in an isolated temporary directory, disables shell/unified-exec/skill-search features and web search, ignores custom MCP/configuration, and asks for structured proposed actions. Antodo executes those actions itself against the open document. An incompatible CLI or missing login produces a chat error; update/login and retry. See [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode).

### ChatGPT Desktop

Antodo does not assume that ChatGPT Desktop exposes a local HTTP chat API. Use OpenAI API or Codex CLI for embedded chat. To use an external ChatGPT/Codex client with antodo's task tools, follow the separate [MCP connection instructions](MCP.md). Client support and any required remote/tunnel connection are described there. Existing external chats are not imported.

## Data access and actions

The assistant can call `list_tasks` (search/status, bounded pagination), `get_task`, `list_tags`, `list_notes` (search plus tag/task filters), and `get_note`. Task results include linked `note_ids`; Note results include `tags` and `tasks`. Enable chat editing before applying the connection to allow `create_task`, `update_task`, `delete_task`, `create_note`, `update_note`, and `delete_note`. Read-only is the default, enforced even if the provider invents a write call.

Chat can create and revise Markdown Notes, assign or remove existing tags, and link the same Note to multiple tasks. Each task can have multiple Notes. Note `tags`/`tasks` arrays and task `note_ids` arrays replace the complete respective relationship set; omitted fields preserve it and `[]` clears it. The assistant is instructed to read the record first and preserve existing links unless removal was requested. Both directions use the same core transactions, reference validation and cascade behavior as the GUI. Task graph, completion, date/timezone and recurrence validation remain in force.

Try “Find notes tagged Research”, “Summarize the notes linked to this task”, or “Link this note to the Release and Documentation tasks, keeping its existing links.” Tag creation/editing and occurrence override tools are not currently exposed. There is no arbitrary SQL, generated code execution, file-open/save tool, or direct database access by the model.

Successful actions update the working SQLite document immediately, refresh the UI, and remain **unsaved until Save**. Earlier successful actions remain if a later action fails or you cancel a request. Deletion is available when edits are enabled, so ask for it explicitly. Tool results/rejections are passed back to the assistant; the chat records each attempted tool by name and outcome. Invalid actions and actions during unapplied local drafts are rejected.

Your selected provider receives your questions, selected task or Note IDs, recent session context and workspace data returned by its tools. Task descriptions and Markdown Note content are treated as data, not authority to perform actions. Cloud providers may therefore receive private workspace content needed to answer your request; choose local Ollama when that data should remain local.

## Session behavior and troubleshooting

Chat remembers up to the last six completed exchanges, including tool results, capped at 512 KB of serialized history. Workspace, task and Note chat share this history; each new question explicitly identifies its current task or Note context (or none). Closing chat keeps completed history; switching/closing workspaces, Disconnect, Clear conversation or applying connection settings clears it. Chat history is not persisted across app restarts. Cancel request or closing chat stops pending work and drops unprocessed proposed actions.

Requests have a 100-second overall process timeout (HTTP also uses a 90-second transfer timeout), up to eight provider rounds and four tool calls per response. Task and Note lists are paginated. Oversized results return an instruction to narrow the query rather than silently hiding records. Large/local models may need a smaller workload to finish within the timeout.

If connection fails, check the URL, installed model, credentials, model tool support, `curl`/`codex` availability and provider service. For Codex, verify `codex login` and CLI version. HTTP error bodies and subprocess stderr are not shown to avoid inadvertently displaying credentials. After a cancelled or failed turn, successful actions already applied remain visible in the workspace; verify them before retrying a mutation.
