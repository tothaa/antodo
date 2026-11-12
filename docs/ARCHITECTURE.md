# Architecture

antodo has two required architectural parts: a GUI-independent core and an Iced user interface. Data access uses generic SQL functions against the SQLite connection. Keep these boundaries when extending the application.

```text
src/main.rs                    starts Iced
       │
       ▼
src/ui/                        widgets, messages, input forms, dialogs
       │ domain commands and query functions
       ▼
src/core/                      all task rules and application logic
       │ generic parameterized SQL functions
       ▼
core::sql → rusqlite::Connection → SQLite working database
                                      │ explicit Save / Save As
                                      ▼
                                <name>.antodo
```

## Module responsibilities

| Module | Responsibility |
| --- | --- |
| `core::model` | Typed items, statuses, dates/timezones, tags, overrides, occurrences and reminders. |
| `core::sql` | Reusable `execute`, `query`, `query_optional` and ordered-collection replacement functions taking a connection, SQL and bound parameters. |
| `core::store` | Schema, file lifecycle, transactional domain commands, graph/date/status validation and reminder acknowledgement. |
| `core::recurrence` | Validating finite RFC 5545 rules; bounded expansion; occurrence overrides. |
| `core::query` | Search and filters, overview, hierarchy, calendar entries, reports, reminder decisions. |
| `core::preferences` | Atomic machine-local persistence of last-file path, theme and content size. |
| `ui::app` | Iced state/messages, native file dialogs, unsaved-change guards and periodic reminder checks. |
| `ui::editor` | Input buffers and conversion to typed domain values. |
| `ui::views` | Rendering, responsive task cards, Markdown presentation and theme-aware layout. |
| `ui::preview` | Isolated sample data and one-screen native screenshot capture for visual review. |

UI state is not the source of business invariants. Call core commands even when a control appears to prevent an invalid action. Core tests run with `cargo test --no-default-features`, which excludes the GUI dependencies. SQL strings and database connections must not enter UI modules. Do not add an ORM or task-specific repository abstraction in place of `core::sql`.

## SQLite schema and files

A `.antodo` file is a normal SQLite database with application ID `0x414E544F` and schema version `2` in `PRAGMA user_version`. Opening checks both, database integrity, foreign keys, and domain content. Unrecognized and future format versions are rejected; no destructive downgrade is attempted.

| Table | Data |
| --- | --- |
| `items` | UUIDv7, name, unbounded Markdown `TEXT`, each date field, status, recurrence, reminder state, priority, severity and effort. |
| `item_parents` | Many-to-many child/parent links, preserving ordering. |
| `tags` | UUIDv7, unique case-insensitive name and `#RRGGBB` color. |
| `item_tags` | Item/tag assignments and ordering. |
| `participants` | Per-item participant strings and ordering. |
| `event_overrides` | UUIDv7, master item ID, original and replacement start/end, cancellation, status and reminder state. |

Each date column stores a serialized `Moment`, preserving the local date/time string, IANA timezone, and an optional UTC-offset disambiguator for generated repeated-hour instants. Missing dates are SQL `NULL`. IDs and bookkeeping fields remain internal; task names and dates identify choices in the GUI. Relationships use foreign keys and cascading deletion. Removing a task deletes its links and overrides, while keeping its child tasks.

`Document` owns an in-memory SQLite working copy. New creates one; Open validates a selected file and copies its contents into memory. Item, relationship, tag and override edits are transactional. A rejected operation does not partially change the document.

Save uses SQLite backup to a sibling temporary file, flushes it, and atomically replaces the selected `.antodo` file. Only successful saves update the document path and clean state. File fingerprints detect edits by another session; active SQLite journal/WAL sidecars prevent replacement. Existing non-antodo files are never accepted as overwrite destinations. This is a single-editor document workflow, not a multiwriter database service. A power loss before Save can lose unsaved in-memory changes, so the interface makes dirty state explicit.

Preferences live in the OS configuration directory selected by `directories::ProjectDirs`. Their JSON contains only `last_file`, `dark_mode` and `font_size`. Missing preferences use defaults; malformed files report an error and the UI can use defaults. A sibling temporary file makes preference replacement atomic.

## Task and graph invariants

- New IDs are UUIDv7; stored IDs are validated.
- Names must contain non-whitespace text. Priorities and severities are 1–5; effort is a nonnegative integer.
- Start/end are a pair and the end cannot precede the start. Other date fields are independent.
- Parent links form a directed acyclic graph. Multiple parents are allowed; new links are checked before the transaction commits.
- Done requires all direct children to be Done or Won't Do. Reopening or moving a child cannot invalidate a Done parent or a completed parent occurrence.
- Completion defaults to now on entering Done, honors a supplied date, and clears on leaving Done.
- Tags must exist before assignment. Participant lists and links are normalized before storage.
- Occurrence overrides must refer to an actual generated original start/end pair. Only one override can exist for a particular original occurrence.
- Schedule changes clear stale overrides. Reminder-relevant changes rearm acknowledgements.

Hierarchy rendering expands each task's children once and emits a shared reference under later parents. Iterative traversal avoids recursion overflow on deep trees and cannot loop even if malformed input is supplied directly to a query helper.

## Dates and finite recurrence

User-entered `Moment` values retain date-only or date-time precision and independent timezones. IANA timezone rules determine UTC instants. A date-only deadline is overdue only after its local day finishes. Calendar all-day end dates are inclusive; timed intervals end exclusively, with zero-duration events retained at their instant.

The recurrence module uses the `rrule` crate for RFC 5545 property expansion, generating in a surrogate wall-clock timeline and resolving timezone transitions explicitly. Invalid generated clock times are skipped without consuming COUNT; generated repeated times choose the first instant. User-entered nonexistent or ambiguous local times are rejected. Timed occurrence durations preserve elapsed time; date-only durations preserve calendar-day count.

Recurrence requires exactly one positive COUNT or UNTIL, not both. The master start must match the recurrence rule; a mismatching DTSTART has undefined RFC behavior and is rejected rather than silently dropping the first date. Timed UNTIL must be UTC; all-day UNTIL must be a date. All-day rules cannot contain hourly/minutely/secondly frequencies or time-of-day properties. Safety limits are enforced with errors:

| Limit | Value |
| --- | --- |
| Entire generated series | 10,000 occurrences |
| Candidate generation | 20,000 candidates |
| UNTIL span | 90,000 frequency periods |
| RRULE text | 2,048 bytes |
| Time-of-day combinations | 1,000 |
| INTERVAL | 1–65,535 |
| Recurrence years | 0001–9999 |
| Reminder lead | 0–525,600 minutes |

The full series is validated before viewport filtering. Oversized, empty or unsupported rules return errors rather than truncated calendars. COUNT/UNTIL are required even when a viewport would otherwise bound the display. This prevents an apparently harmless view request from making validation unbounded.

Overrides apply before calendar-range filtering, so moving an occurrence into a month works even when its original date was elsewhere. Cancelled occurrences disappear. Occurrence status and reminder state replace the master's corresponding values for that occurrence. Planned/deadline markers follow each recurrence's offset and rescheduling. Master schedule changes invalidate the old occurrence identity and clear overrides.

## Queries and reports

Filters compose with AND; free text searches name and Markdown source, case-insensitively. Date filters compare the selected field's local date. Entering a range without selecting a field selects Deadline; clearing the date field clears its bounds. Calendar and overview status, date and notification filters apply to effective occurrences after overrides and marker shifts. Name, description, parent, participant, tag, priority, severity, effort, recurrence rule and reminder configuration describe the master task. List, outline and report filters operate on master records. Internal ID selection is translated into named UI choices.

The overview derives overdue, ongoing, today, upcoming and nearest-deadline groups using the local viewing timezone. It evaluates complete bounded recurrence series so old overdue occurrences and far-future nearest deadlines are not silently omitted. A task appears once per overview group, even when several occurrences match. Overview totals count unique matching tasks; a recurring task counts as Done only when all matching non-cancelled occurrences are Done. These occurrence-aware overview totals can differ from the master-record report.

Reports count master tasks, avoiding double-counting shared parents or repeated occurrences. Completion is `Done / all filtered tasks × 100`; Won't Do is shown separately. Effort uses completed master-task points. Tag reports can overlap because a task can carry more than one tag; empty denominators produce 0%.

Reminders are app-local messages. Every 20 seconds the UI asks the core which unacknowledged triggers are due, records those acknowledgements in the working database, and displays the messages. Scheduled start takes precedence over planned date and deadline. There is a 24-hour catch-up window after the occurrence start. Completed/cancelled occurrences do not remind. Persist acknowledgements with Save. There is no scheduler running after the application exits.

Derived recurring planned/deadline markers are validated before saving items or overrides and when opening a file. A marker that falls in a nonexistent local clock time is rejected with its task name and date, so one invalid marker cannot disable the whole overview. Overview cards use representative occurrence dates and statuses; opening one edits that occurrence. UI caches these derived views between messages and invalidates them after changes.

Version-1 documents migrate transactionally in the working copy by rebuilding the item/occurrence status constraints to accept Event; disk bytes remain unchanged until Save. Event commands clear task-only dates and effort in core.

## MCP

`core::mcp` owns transport configuration, bounded local HTTP workers, JSON-RPC dispatch and task tools. Network workers enqueue requests; the UI's conditional 100 ms subscription asks core to process a bounded batch against the GUI-owned Document. There is no second database connection or competing working copy. Local drafts/modals block external mutations. Read calls return applied state, not incomplete GUI drafts. UI refreshes caches after mutations. Server destruction closes the listener and cancels outstanding work; workspace replacement/close drops the server. No Iced dependency is required in core.

The server binds only to loopback, validates Host and Origin, requires a bearer token, limits request headers/bodies and concurrent connections, supports JSON POST replies and returns 405 for optional GET/SSE and session DELETE. It negotiates MCP protocol versions 2025-03-26, 2025-06-18 and 2025-11-25 without issuing sessions. Tools cannot perform file lifecycle operations. Lists explicitly paginate rather than silently truncating records.


## LLM chat

`core::llm` owns provider configuration validation, bounded session history, cancellable workers, HTTP/Codex codecs and tool orchestration. HTTP requests use curl with private temporary request/header files (keys are not placed in argv); Codex uses an isolated ephemeral structured-output process. Codex reads its prompt from a private temporary file, allowing cancellation and timeout checks even if the child never reads stdin. Temporary request/response files are removed when the worker ends. No shell command interpolation is used. Worker threads return assistant messages only. On the owning UI thread, core dispatches proposed tool calls through `core::mcp::dispatch` and the existing Document commands; it never runs arbitrary SQL or generated code. Read-only and unapplied-draft protection are enforced there. The UI renders configuration/chat, polls only during active requests, supplies selected task context and refreshes cached views/clean editors after successful actions. External MCP writes are suspended while chat is open. Workspace reset cancels chat and clears history. No Iced dependencies enter core.

Session history retains at most six completed exchanges / 512 KB; each response is capped at 128 KB, tool results at 64 KB, HTTP output at 2 MB, and turns at eight provider rounds / four tool calls per response. Errors and cancellation discard unfinished conversation context but preserve successful task mutations. There is no file lifecycle, tag write, occurrence override or unrestricted database tool in the LLM catalog.


## Notes and bulk task commands

Schema version 3 adds `notes(id, name, content)`. IDs are UUIDv7, names must be nonempty, and content retains Markdown source. `core::notes` owns validation, transactional CRUD and Unicode-aware literal search; the GUI only edits and renders domain values. Versions 1 and 2 migrate on the in-memory working copy and remain unchanged on disk until Save. Notes share file-safety and external-edit guards with tasks.

`Document::save_items` prepares every change, validates the complete resulting graph and calendar, and writes all task/relationship changes in one transaction. Graph validation uses an iterative topological pass, including on document open. `core::actions` reuses this for bulk status, ratings, tags, plans and parent links. Hierarchy selection retains `(item_id, parent_id)` so moving one displayed reference preserves the task's other parents. A group is validated and committed together; failed validation or SQL writes roll back the entire operation.

`ui::task_interactions` centralizes pointer selection, context menus and drag gestures for all task views. It delegates every mutation to core. The Notes drawer uses the shared Markdown highlighter and guarded apply/discard/stay flow. Linux window identity is `antodo`, matching the installed desktop entry; `ui::icon` embeds the SVG-derived RGBA icon for window managers that support bitmap window icons.


## Shared Notes relationships and chat context

Schema version 4 adds ordered `note_tags` and `note_tasks` tables with cascading foreign keys. The latter is the only stored task-note relationship; `Note.tasks` and task-side note selections are views of the same links. `save_note` validates referenced records and replaces content/tags/tasks in one transaction. `save_item_with_notes` reuses the task batch validation path and writes the task plus its note selection in that transaction. Ordinary task commands preserve note links. Deleting a task or tag removes its relationships while preserving notes; deleting a note preserves tasks and tags.

Migration from schema versions 1–3 operates only on the working copy until Save. Notes retain default empty relationship arrays when deserializing older JSON. `core::llm::Context` distinguishes selected tasks and notes; MCP and embedded chat share the same tool catalog and core mutation methods. Clean embedded-chat editors refresh from successful tool mutations, while dirty drafts block writes. External MCP still requires local drawers and dialogs to be closed.


## Linked drawer navigation

The GUI keeps a stack of task/Note targets while following linked records. Navigation captures its origin before an apply/discard prompt, so applying the current editor cannot lose its return target. Close pops one level; explicit Apply/Delete also return after success. Restored editors reload current working records and relationships, skipping deleted targets. Selecting the current record adds no level. Page/workspace changes clear history, while a cancelled file-open preserves it. History contains only UI navigation and is not persisted.
