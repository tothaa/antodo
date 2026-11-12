## Context-menu Link a Note — 2026-11-11

Added Link a Note beside Link a Parent in every task context menu, including multi-selection. It reuses the assignment dialog with text/tag search and excludes Notes already linked to every selected task. Selecting a Note applies its links to all selected tasks and closes the dialog, matching Link a Parent. Existing Note/task links are retained without duplicates. Changes remain in the working document until Save. Cancellation is read-only; validation failures retain the picker and show the error.

Three added regressions cover actual menu/picker clicks on Overview, All tasks, Hierarchy and Calendar; filtered results and existing-link exclusion; bulk linking and preserving existing links; cancellation; stale-task failure; and core atomic rejection/idempotence. The shared core helper delegates to transactional Note validation/persistence.

Validation passed: formatting; Clippy with warnings denied; **269 full-feature tests** (one intentionally ignored benchmark); **158 core-only tests**; full-feature coverage (**93.50% lines**, **93.21% regions**); release build; and `cargo package --locked --allow-dirty --offline` including extracted-package compilation. Cargo used `CCACHE_DISABLE=1`; existing loopback mock tests ran outside the restricted network sandbox. The local release executable was rebuilt.

## Linked drawer return chain — 2026-11-11

Fixed task-to-Note and Note-to-task navigation replacing the previous drawer. Linked navigation now captures a return target before unsaved-edit decisions and keeps a stack for repeated/deep navigation. Close, successful Apply and successful Delete return one level, with fresh working records/relationships. Page/workspace navigation clears the stack. Missing/deleted records are skipped.

Four new regressions cover both navigation directions, repeated records, deep chains, actual linked-name/Close button dispatch, apply/discard/stay, validation failure retaining the current level, refreshed links, deletion and navigation reset. Existing task/Note editor, MCP/Chat guard and file-safety tests continue to pass.

Validation: **266 tests passed** (one intentionally ignored benchmark), **157 core-only tests passed**, formatting and Clippy with warnings denied passed, full-feature coverage passed (**93.42% lines**, **93.13% regions**), release build passed, and `cargo package --locked --allow-dirty --offline` passed with extracted-package compilation. Cargo used `CCACHE_DISABLE=1`; isolated loopback mock services required the existing sandbox escalation. The release executable for the local launcher was rebuilt.

## Assignment lists and Add dialogs — 2026-11-11

Edit task now lists only its assigned Notes, and Edit note lists only its assigned tasks. Both sections have Detach per link and Add… to open a modal selector. The selectors exclude existing assignments, allow repeated selections, and filter by text/tag; task selection also supports status/priority. Adding and detaching stage editor changes until Apply. Closing the selector with Close, Escape or its backdrop preserves staged changes, and existing apply/discard/stay protection remains in effect.

Two new UI regressions exercise rendered buttons, filtering, excluded assignments, staged add/detach, background message guards, Escape and discard. Native previews were captured and inspected for both drawers and [Add note](../assets/screenshots/add-note.png)/[Add task](../assets/screenshots/add-task.png). Preview indices 32 and 33 reproduce the dialogs.

Validation passed: formatting; Clippy with warnings denied; **262 full-feature tests** (one intentionally ignored benchmark); **157 core-only tests**; full-feature coverage (**93.34% lines**, **93.00% regions**); release build; and `cargo package --locked --allow-dirty --offline` including compilation of the extracted package. Cargo used `CCACHE_DISABLE=1`; loopback mock-service tests ran outside the restricted network sandbox. The local release executable was rebuilt for the existing launcher.

## Tagged Notes, shared task links and Note chat — 2026-11-11

Notes now use the existing tag catalog and show a separate stripe for each assigned tag. Notes and tasks have shared many-to-many links, editable from either drawer with searchable named choices and the usual apply/discard/stay protection. Schema v4 adds ordered link tables with cascading cleanup. The same core transactions power GUI, MCP and embedded chat; ordinary task commands preserve note links.

MCP Note tools accept tags/task links and support combined text/tag/task filters. Task tools expose and edit `note_ids`. Embedded chat carries an explicit task or Note context, reads applied state and can manage Notes and links when editing is enabled. Successful changes refresh clean drawers; unapplied local content or relationship drafts reject writes. Existing files migrate only in the working copy until Save.

Final verification:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test` | **260 passed**, 1 intentionally ignored large-table benchmark |
| `cargo test --no-default-features` | **157 passed** without Iced |
| `cargo llvm-cov --all-features --summary-only` | **93.34% line coverage**, **93.05% region coverage**; full suite passed |
| `cargo package --locked --allow-dirty --offline` | Passed with extracted package compilation |
| `cargo build --release --offline` | Passed |

Twenty added tests cover schema v1/v2/v3 migration, relationship round trips, many-to-many assignment, deletion cleanup, invalid-reference and SQL-trigger rollback, two-sided editor apply/discard, MCP patches/filtering/permissions, Note chat context and mocked provider actions, and local draft protection. Native previews exposed a zero-height stripe layout in the scrolling Notes list; it was fixed, and the rendering regression now checks positive bounded stripe rectangles inside the actual scroll viewport at both narrow and wide sizes and in both themes.

Inspected native previews: [tagged Notes](../assets/screenshots/notes.png), [Note assignments](../assets/screenshots/note-editor.png), [task Notes selector](../assets/screenshots/task-notes.png) and [Note chat](../assets/screenshots/note-chat.png). The chat preview uses sample responses; integration tests use local mock providers, not live service accounts. The release executable is rebuilt at `target/release/antodo`, used by the existing local launcher. Restart an already-running instance to load it.

Checks used `CCACHE_DISABLE=1`; loopback tests required execution outside the restricted network sandbox. Packaging allows the existing uncommitted workspace and performs no publication or remote changes.

## Notes, task menus and hierarchy gestures — 2026-11-11

Implemented searchable Markdown Notes with UUIDv7 identities, transactional persistence, schema v3 migrations and five MCP tools. Notes reuse the task Markdown highlighter and protected editing lifecycle. Added shared task context menus, Shift-click selection and atomic bulk actions across Overview, All tasks, Hierarchy and Calendar. Hierarchy moves retain the exact displayed parent, support Ctrl/Option additional links and validate the whole resulting graph before writing.

New regression coverage includes Notes CRUD/search/migration, MCP permissions and draft protection, order-independent bulk completion, SQL-error rollback, cycles and shared links, actual Iced pointer events on all four task screens, scrolled menu coordinates, interrupted drag cancellation, hidden/deleted selection cleanup and scrollable menus at minimum window/maximum font size. Graph validation is iterative and linear; a 10,000-task chain regression protects against stack overflow.

Final checks:

| Command | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test` | **240 passed**, 1 intentionally ignored large-table benchmark |
| `cargo test --no-default-features` | **143 passed** without Iced |
| `cargo llvm-cov --all-features --summary-only` | **93.30% line coverage**, **92.97% region coverage**; full suite passed |
| `cargo package --locked --allow-dirty --offline` | Passed, including extracted package compilation |
| `cargo build --release --offline` | Passed |
| `desktop-file-validate assets/antodo.desktop`, `sh -n assets/install-desktop.sh` | Passed |

Cargo commands used `CCACHE_DISABLE=1`; loopback test services required execution outside the restricted network sandbox. `--allow-dirty` packages the existing uncommitted workspace without changing Git history. No publishing or remote operations were performed.

Native isolated previews of [Notes](../assets/screenshots/notes.png), [the Markdown note editor](../assets/screenshots/note-editor.png) and [the hierarchy status menu](../assets/screenshots/task-menu.png) were captured and inspected. Headless tests additionally draw both themes at wide and narrow sizes. Window settings embed the SVG-derived raster and use application ID `antodo`; the user desktop entry/SVG were installed and KDE's application cache refreshed, with the local launcher pointing to the rebuilt release executable. An already-running instance must be restarted. Native taskbar appearance and cross-platform pointer/file-portal behavior remain manual acceptance checks; screenshots establish application rendering, not compositor behavior.

Earlier validation records follow.

## Coverage and accuracy review — 2026-11-11

Reviewed the existing test cases against LLVM's uncovered-line report and added **43 regression tests**. The work covers domain boundaries and failures, provider/process handling, malformed MCP requests, UI polling, and real Iced widget layout/drawing and click routing. Tests use temporary documents, ephemeral loopback services and fake provider executables; no live provider account or inference was used.

Three defects were demonstrated and fixed:

- Calendar display rejected valid all-day dates with skipped or repeated local midnight. It now uses the existing resolved day-start/end boundaries. Regressions include São Paulo, Havana and Samoa's skipped date.
- A Codex process that did not read stdin could block a large prompt write before cancellation/timeout polling began. Private-file stdin avoids that blocking write; tests verify the full prompt, cancellation, deadlines and child reaping.
- Markdown text such as `[plain text)` was highlighted as a link. Highlighting now requires `](` and a closing parenthesis.

The review also removed 78 lines of unreachable large-card rendering instead of introducing artificial tests for a private mode with no callers.

| Check | Result |
| --- | --- |
| `cargo fmt --check`, `git diff --check` | Passed |
| `cargo clippy --all-targets --offline -- -D warnings` | Passed |
| `cargo test --offline -- --include-ignored` | **206 passed**, including the manual large-table benchmark; none ignored |
| `cargo test --no-default-features --offline` | **122 passed**, without Iced |
| `cargo llvm-cov --all-features --summary-only --offline` | **205 passed**, one manual benchmark ignored; **93.63% lines**, **92.98% regions** |
| `cargo package --locked --allow-dirty --offline` | Passed, including compilation of the extracted source package |

All-feature line coverage increased from **65.52% to 93.63%** using the same unfiltered coverage command. Uncovered executable lines fell from 2,929 to 608. These figures include inline tests and the preview harness; no files were excluded. Selected module line coverage:

| Module | Before | After |
| --- | ---: | ---: |
| `core::llm` | 76.52% | 98.75% |
| `core::mcp` | 89.66% | 96.87% |
| `core::recurrence` | 88.69% | 94.86% |
| `core::store` | 93.11% | 96.74% |
| `ui::app` | 89.17% | 95.10% |
| `ui::views` | 8.36% | 94.31% |
| `ui::table` | 19.61% | 90.74% |
| `ui::datetime` | 58.73% | 95.87% |

The headless harness uses Iced's renderer and widget tree, exercises responsive builders and styles, validates finite bounded layouts, inspects widget labels/input values, and dispatches mouse events. Tests cover both themes and widths, welcome/empty/error states, drawers and modal background protection, event-only controls, table selection/sorting, masked credentials and MCP/LLM controls. Polling tests check each local MCP blocker, chat update/delete refresh, dirty draft preservation, cancellation before a proposed action is applied, and explicit Save persistence.

Remaining uncovered paths include OS startup/preferences wrappers, native file-dialog callbacks and screenshot capture, plus exceptional I/O failures. Headless tests do not replace native window-manager, portal or cross-platform acceptance checks. Commands used `CCACHE_DISABLE=1`; network tests ran with loopback enabled. The local HTML report is generated by `cargo llvm-cov report --html --output-dir coverage` at `coverage/html/index.html` and is intentionally ignored by Git.

---

## Source simplification review — 2026-11-11

Reviewed all application source modules, entry points and the preview harness. Extracted reusable calendar-entry and interval logic, semantic date comparison, ordered SQL writes, mutation bookkeeping, MCP lookup/response handling, provider process execution, save completion, editor resets and repeated UI widgets. Core/UI boundaries, public core APIs, transactional validation and visible layouts are preserved. Combined `src/core` and `src/ui` source fell from 10,678 to 10,544 lines, including the new inline regression tests.

Validation:
- `cargo fmt --check`, `git diff --check` and `cargo clippy --all-targets --offline -- -D warnings` passed.
- `cargo test --offline`: **162 passed**, one existing manual scrolling benchmark ignored. `cargo test --no-default-features --offline`: **99 passed**, without Iced.
- `cargo llvm-cov --all-features --summary-only --offline`: **65.52% line coverage**, **65.33% region coverage**, with all tests passing.
- `cargo package --locked --allow-dirty --offline` passed, including compilation of the extracted package. `--allow-dirty` retains the existing uncommitted work; no commit or publication was made.
- New regressions cover failed Save/Save As continuations, ordered relationship replacement/removal, missing-record mutations preserving clean state, MCP lookup failures and protocol fallback, and date-picker precision/default times.
- Native isolated previews of Overview, All tasks, Calendar, date/time picker and task editor were captured and inspected. Automated tests cover UI transitions; native previews check rendering rather than desktop file-portal or pointer behavior.

Cargo commands used `CCACHE_DISABLE=1`; socket integration tests ran with loopback networking enabled. An independent review of the save/state helper extraction found no behavior regressions.

---

## LLM setup and session chat — 2026-11-11

Added the LLM configuration page, workspace/task-context chat buttons and modal, Ollama/OpenAI/Gemini compatible HTTP requests, and local Codex CLI structured-output integration. Core orchestrates bounded session history and validated tools against the GUI-owned working document. API keys/history remain in memory. HTTP uses curl; Codex uses its installed login. See [setup instructions](LLM.md).

Validation:
- Formatting, whitespace checks and Clippy with all targets / warnings denied passed. `cargo package --locked --allow-dirty --offline` passed, including compilation of the extracted source.
- Default-feature tests: **157 passed**, one existing manual scrolling benchmark ignored. Core-only tests: **96 passed**, without Iced.
- All-feature coverage: **64.63% lines**, **64.57% regions**, with all tests passing.
- Mock HTTP integration covers task context, database reads, multi-turn memory, mutation success, read-only/draft rejection, core validation, bounded history, malformed provider replies and cancellation. Unit/UI tests cover Codex JSON-stream decoding, tool/response limits, setup/navigation guards and session reset/disconnect.
- Native isolated setup and task-chat previews inspected: [LLM page](../assets/screenshots/llm.png), [task chat](../assets/screenshots/chat.png). The editor button appears before Plan for Today. New replies scroll into view.
- The core-only run exposed an intermittent MCP TCP reset on early HTTP rejection. Responses now half-close and perform a bounded drain so clients receive the error before the connection closes; repeated authentication rejection is covered.
- Installed Codex CLI argument parsing was checked without making a model request. No live Ollama service, cloud account, API charge or authenticated Codex inference was used; actual provider/model availability remains a user-side setup check. Socket integration tests ran with local networking enabled.

---

## MCP server and version 1.0.0 — 2026-11-11

Added the MCP page with default loopback IP/port, generated masked token, optional edits, Start/Stop, copyable Codex configuration and local/cloud connection guidance. The server is scoped to the open working document and stops on close/replacement. All transport/protocol/domain commands live in core; the GUI owns input handling and serializes queued commands against its existing document.

Validation:
- `cargo fmt --check`, `git diff --check`, and Clippy with all targets and warnings denied passed.
- Default-feature tests: 148 passed, one existing manual scrolling benchmark ignored. Core-only tests: 88 passed.
- All-feature coverage passed: 64.52% lines, 64.45% regions. Socket tests require loopback networking; they ran outside the restricted network sandbox.
- Installed Python MCP SDK interoperability passed: initialize, tools/list, create, update with completion, list and delete against an isolated in-memory server.
- Regression tests cover configuration/port errors, authentication, hostile Origin, malformed JSON, notification 202, optional GET 405, handshake, read-only catalog, pagination, tool errors, local-draft mutation guards, DAG/completion/DST rules, Stop/restart and workspace lifecycle.
- Native isolated MCP running-page preview inspected: [MCP page](../assets/screenshots/mcp.png). No real workspace or token was used in that capture. No external LLM account or ChatGPT tunnel was configured; those require client-side setup documented in MCP.md.

Connection guidance follows the [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports), [official Codex MCP documentation](https://learn.chatgpt.com/docs/extend/mcp), and [OpenAI Secure MCP Tunnel documentation](https://developers.openai.com/api/docs/guides/secure-mcp-tunnels).

---

## Large-list scrolling — 2026-11-11

All tasks now builds a viewport-sized window of keyed rows with 12 rows of overscan on each side; spacers preserve the full scroll range. The actual viewport height determines the window during layout, including resizing. Filter/sort results are cached; scroll-only messages retain the cache and derived dashboard/calendar caches. Page switches and workspace resets clear the remembered offset.

The manual debug widget-construction benchmark with 10,000 tasks measured 123.39 ms for constructing every row versus 0.615 ms per construction of 42 visible/overscan rows (100 runs). This measures widget construction only, not complete rendering/frame time or a guaranteed FPS gain. Native preview at the bottom of 10,000 tasks showed the final rows through Task 09999 with correct scrollbar extent.

Checks: 141 default-feature tests passed (one manual benchmark ignored in the normal run); 82 core-only tests passed; the manual benchmark passed separately. Formatting, Clippy and all-feature coverage passed. Coverage: 63.74% lines, 63.86% regions. Regression tests check bounded row count, viewport coverage, constant scroll extent, empty/short lists and clamped offsets. Full native wheel/trackpad latency was not instrumented.

---

## Event status — 2026-11-11

Added Event to the model, status controls, filters and SQLite constraints. The Event editor hides Deadline, Planned, Completed, Effort/points and Plan for Today. Core save commands clear inapplicable dates and zero effort; hidden stale inputs do not block saving. Start/End, recurrence, reminders and relationships remain supported.

Version-1 files migrate transactionally in the in-memory working copy to schema version 2; disk bytes remain untouched until Save. Regression tests cover a genuine version-1 schema, Event/occurrence round trips, dates and effort normalization, schedule/relationship retention, and the Plan for Today guard.

Validation: 140 default-feature tests and 82 core-only tests passed. Formatting and Clippy passed. All-feature coverage passed (64.34% lines, 64.37% regions). Native Event editor preview verified the controls are hidden. Package validation compiles the extracted source.

---

## Table and editor polish — 2026-11-11

- Added left/right column ordering, full-cell hover tooltips, status-colored table rows and a separate stripe for every tag.
- Added a modal palette/RGB color widget with live hex preview, Select/Cancel, and background input protection.
- Added inline Overview deadlines (including moved recurring deadlines), enlarged the navigation motto, widened the editor and moved Schedule labels onto input rows.
- Added Plan for Today: sets the date in the Planned field's timezone, applies the full draft and closes only on success.

Checks: formatting, Clippy (`--all-targets -- -D warnings`), 137 default-feature tests, 80 core-only tests, and all-feature coverage passed. Coverage: 64.03% lines, 64.04% regions. `cargo package --locked --allow-dirty --offline` passed with extracted-source compilation. Native isolated previews inspected for the column controls, color dialog, wider editor and Overview deadlines. Hover tooltips use Iced's built-in tooltip widget; pointer behavior was not separately automated.

Regression tests cover ordering boundaries and hidden columns, dialog commit/cancel/Escape, modal background guards, Plan for Today success/validation failure, and shifted occurrence deadlines.

Screenshots: [table controls](../assets/screenshots/table-columns.png), [color dialog](../assets/screenshots/color-picker.png), [editor](../assets/screenshots/editor.png).

---

## Workspace refinements — 2026-11-11

Implemented the configurable sortable task table, compact alphabetic hierarchy, RGB tag picker, calendar display timezone and tag stripes, empty-day task creation, filtered parent attachment popup, editor cleanup and resize control, guarded Close/Quit, and closed-status reports.

Validation on the updated source:
- `cargo fmt --check` and `git diff --check`: passed.
- `cargo clippy --all-targets --offline -- -D warnings`: passed.
- `cargo test --offline`: 133 tests passed.
- `cargo test --no-default-features --offline`: 79 core tests passed.
- `cargo llvm-cov --all-features --summary-only --offline`: passed; 64.79% line coverage, 64.63% region coverage.
- `cargo package --locked --allow-dirty --offline`: passed, including compilation of the extracted package.
- Native isolated previews inspected for task table, Tags, editor, compact hierarchy and narrow Calendar. Automated UI transition tests cover parent attachment, closing files, completion timestamps and calendar prefills; they do not replace pointer testing on every desktop backend.

Regression coverage includes original-zone all-day conversion across DST/date-line boundaries, numeric/date table sorting, flattened descriptions, alphabetic siblings, Done/Won't Do report totals, immediate/manual/default completion timestamps, parent filtering/attach/detach, and save/discard protection when closing files or quitting.

Current screenshots: [editor](../assets/screenshots/editor.png), [Tags](../assets/screenshots/tags.png), [hierarchy](../assets/screenshots/hierarchy.png).

---

# Validation — 10 November 2026

## Current editor review

Task selection opens the editable drawer directly, and Apply keeps it editable. The read-only task-details mode and Cancel editing action have been removed. Colored status buttons, confirmed Delete, bounded priority/severity arrows, inline date/timezone controls, Markdown source highlighting and live formatted preview are implemented. Calendar day columns fill available width instead of a fixed-width horizontally scrolling canvas.

Updated regression tests cover direct editing from every page, staged/applied status changes and relationship rejection, rating bounds and invalid input, Markdown preview updates and persistence, and UTF-8 highlighting with fence invalidation. Fresh checks: `cargo fmt --check` and Clippy with warnings denied passed. `cargo test` passed **126 tests**; `cargo test --no-default-features` passed **77 core tests**. All-feature coverage passed the full suite with **66.19% line coverage** and **65.72% region coverage**. `cargo package --locked --allow-dirty --offline` passed, including compilation of the extracted package.

Native previews verified the new editor in dark/light themes and at wide/narrow widths, plus Calendar at wide/narrow widths. The editor shows colored status buttons, rating arrows, Delete, highlighted Markdown source and a live formatted preview. Calendar columns visibly expand/shrink to the available width. Refreshed [editor](../assets/screenshots/editor.png), [light editor](../assets/screenshots/editor-light.png) and [Calendar](../assets/screenshots/calendar.png) captures are retained.

## Previous drawer review

Theme switching now paints an opaque full-window background primitive. The software compositor keeps one clear-color value across its drawing buffers; after the first buffer changes theme, an older buffer can receive only a small hover repaint and retain the previous background elsewhere. A standalone renderer probe reproduced stale pixels in both dark-to-light and light-to-dark transitions, then verified that explicit background drawing matches a complete repaint in both directions without opening a drawer.

The drawer uses a solid border without a blurred shadow. Tiny-skia 0.14.1 paints shadow pixmaps without the damage clip mask, causing unchanged pixels outside partial redraw regions to darken repeatedly. A standalone renderer probe reproduced this with the old shadow and verified byte-identical pixels after 30 partial redraws without it. This checks rendering directly, without a desktop screenshot or mouse automation.

Task details, task editing, new tasks and occurrence editing now open in a right-side drawer over the current page. The top-left Close button, dimmed backdrop and Escape use the existing unapplied-change guard; closing the drawer retains the page and filters.

Formatting, Clippy, regression tests and the core-only build pass: **120 full-feature tests** and **76 core tests**. All-feature coverage is **65.67% of lines** and **64.61% of regions**, with all 120 tests passing. `cargo package --locked --allow-dirty --offline` passed, including compilation of the extracted source package. The completed results below verify the previous interface review.

Regression checks cover clean dismissal, applying or discarding task and occurrence edits, cancelling close confirmation, invalid new tasks, Escape priority, and blocking shortcuts from modifying a draft behind its close confirmation. The workspace keeps a stable alert slot so notices and reminders do not replace the underlying page's scrollable widget.

Native Iced captures verified wide and narrow task viewers and editors, plus wide and narrow date/time selectors above the drawer. The current page remains visible behind the drawer; its fixed header has a top-left Close control and visible task actions. Status buttons wrap, date controls fit, and long task content scrolls inside the drawer. Accepted captures: [task drawer](../assets/screenshots/drawer.png), [editor drawer](../assets/screenshots/editor.png), and [date/time selector](../assets/screenshots/date-time-picker.png).

## Previous interface review

This review adds task-viewer status actions, calendar/time selectors for task and occurrence dates and date filters, searchable IANA timezone choices, Calendar clock times and timezones, and Overview workload, tag progress, effort and recent completions. Insights has been removed from navigation; its useful progress information is available in Overview. Startup argument handling now lives in the UI module, leaving `main.rs` as the Iced entry point.

Core corrections require an explicit offset when loading stored ambiguous local times and check completed parent occurrences when changing child statuses. Regression tests cover both cases.

Fresh checks for this review:

| Check | Result |
| --- | --- |
| `cargo fmt --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test` | **114 passed**, no failures |
| `cargo test --no-default-features` | **76 core tests passed**, no failures |
| `cargo llvm-cov --all-features --summary-only` | **65.71% line coverage**, 64.58% region coverage; all 114 tests passed |
| `cargo package --locked --allow-dirty` | Passed; extracted source package compiled successfully |

Native Iced previews were captured and visually inspected for dark/light Overview, Calendar, task details, task editor, date/time selector, enlarged content, narrow selector and narrow details, and Overview scrolled to its progress sections. The date/time selector is centered, its calendar and clock controls fit the narrow window, status actions are visible in task details, and Calendar entries show clock times or all-day labels. The Overview workload, tag-progress and recent-completion sections are readable. Review prompted corrections to selector alignment and content padding beside scrollbars.

Updated captures: [Overview](../assets/screenshot.png), [light theme](../assets/screenshots/light.png), [Calendar](../assets/screenshots/calendar.png), [editor](../assets/screenshots/editor.png), [date/time selector](../assets/screenshots/date-time-picker.png), and [Overview progress](../assets/screenshots/overview-progress.png).

Native previews use independent in-memory sample data. File-portal interaction and pointer/keyboard paths remain in the expanded manual checklist in [REQUIREMENTS.md](REQUIREMENTS.md). Automated tests exercise rejected status changes and invalid clock-change times; screenshots establish rendering, not those interactions. The measurements below are the earlier baseline.

## Earlier baseline

Tested on Linux with Rust 1.99.1 and Cargo 1.99.1. SQLite is bundled. Commands used `--offline` after fetching the locked dependencies and `CCACHE_DISABLE=1` because this environment's C compiler wrapper otherwise writes to a read-only cache.

## Completed steps

1. Established the Cargo package, GPL-2.0-only project license, core/UI boundary, generic SQL helpers, and local Git repository (`main`, local `user.name = tothaa`).
2. Implemented and tested SQLite documents, all requested fields, graph and completion constraints, finite recurrence, overrides, timezones, filters, reporting, preferences, and reminders.
3. Implemented and inspected the six Iced views, task/occurrence editing, Markdown details, colored tags, light/dark themes, scalable content, file actions and unsaved-change guards.
4. Corrected defects identified by tests and independent reviews, then ran the checks below.

| Check | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test` | **98 passed**, no failures |
| `cargo test --no-default-features` | **73 core tests passed**, no failures |
| `cargo llvm-cov --no-default-features --summary-only` | **91.52% line coverage**, 87.90% region coverage |
| `cargo llvm-cov --all-features --summary-only` | **64.45% line coverage**, 62.79% region coverage |
| `cargo build --release --locked` | Passed |
| `cargo package --locked --allow-dirty` | Passed, including compilation of the extracted crate |
| `cargo install --path . --locked --root /tmp/antodo-install` | Passed; installed executable reports `antodo 0.1.0` |
| Installed `--help` and `--version` without a display | Passed |
| Desktop launcher validation | Passed |

The 98 tests comprise 24 UI/editor tests, 1 CLI test, 3 preference tests, 26 query tests, 19 recurrence tests, and 25 storage tests. They cover success and failure paths, database round trips, atomic file safety, external modifications, parent cycles, child completion gates, date precision/timezones and DST, finite recurrence and moved/cancelled overrides, effective occurrence filters, reminder idempotency, and unsaved editor/file transitions.

Full-feature coverage includes the native rendering code, which is not executed by unit tests. Its lower percentage is reported without hiding those files. Screenshots are separate visual evidence, not test coverage. Both coverage commands ran their complete applicable test suites successfully.

## Native visual checks

The corrected preview harness launched and captured ten independent native Iced windows: dark/light overview, all tasks, hierarchy, calendar, insights, task details, the multiline editor, tags, and enlarged content in a smaller window. Visual review prompted fixes to task-card contrast, description previews, calendar cell sizing, scrolling at larger content sizes, and screenshot timing. The editor's initially blank capture was a harness issue; the corrected startup capture visibly contains the Markdown text.

Retained captures: [overview](../assets/screenshot.png), [calendar](../assets/screenshots/calendar.png), [editor](../assets/screenshots/editor.png), and [light theme](../assets/screenshots/light.png).

Native file-portal interaction, real keyboard/pointer focus paths, and cross-platform Windows/macOS behavior still need the manual release checklist in [REQUIREMENTS.md](REQUIREMENTS.md). Automated UI tests exercise the same application messages and returned file paths, without controlling the OS file picker.

## Release state

The source crate is packaged and build-verified; it has not been published. The local repository is initialized and source files are staged. No commit was created because no commit email was supplied; configure the email you want before the first commit. No GitHub repository was created or pushed.
