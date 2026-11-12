# Changelog

## Unreleased

- Add Link a Note to task context menus, with searchable single/bulk assignment through the shared picker.

- Keep a return chain when opening linked task and Note drawers; closing a level restores the previous record.

- Show assigned Notes/Tasks with Detach buttons and add filtered assignment dialogs from both drawers.

- Add shared Notes tags and task links, colored note stripes, task-side note assignment, and note-aware MCP/LLM chat.

- Add searchable Markdown Notes, protected note editing, schema migration and Notes MCP tools.
- Add task context menus, Shift+click multiple selection, atomic bulk edits and hierarchy drag/drop with additional-parent modifier.
- Bundle the application window icon and match the Linux application identity to the desktop launcher.


## 1.0.0 — Unreleased

- Expanded domain, provider, protocol and headless GUI regression coverage. Fixed all-day calendar display at skipped/repeated midnights, cancellation when Codex does not read its input, and malformed Markdown link highlighting; removed unused large-card rendering.

- Simplified shared calendar/interval calculations, ordered relationship writes, provider execution, save handling and UI controls while preserving existing behavior and adding regression coverage.

- Added the LLM setup page and task-aware session chat for Ollama, OpenAI/Gemini compatible APIs and local Codex CLI, with validated task actions, read-only defaults and bounded in-memory conversation history.

- Open tasks directly in Edit task drawers; colored status buttons, Delete confirmation and bounded rating arrows replace the separate task viewer.
- Markdown source highlighting with live formatted preview, inline timezone selectors and responsive Calendar columns.

- Initial Rust/Iced desktop planner with light/dark themes and remembered content scaling.
- Portable `.antodo` SQLite workspaces with explicit atomic Save/Save As and last-file reopening.
- Complete task editor with Markdown descriptions, timezone-aware dates, participants, priorities, severity and effort.
- Colored tags, multiple-parent hierarchies, cycle prevention, and completion invariants.
- Finite RFC 5545 recurrence with per-occurrence rescheduling, cancellation, status and reminder acknowledgements.
- Overview with workload, progress and recent completions, searchable/filterable task list, hierarchy, and month calendar with clock times and timezones.
- Task-viewer action buttons for every status, with the same completion and parent/child rules as the editor.
- Task details and editing open in a side drawer with a top-left Close button, Escape dismissal, and protection for unapplied changes.
- Remove blurred drawer shadows that accumulated darkness during partial software-renderer redraws.
- Paint the full-window theme background explicitly so reused drawing buffers cannot retain the previous theme after switching modes.
- Calendar/time selectors for task dates, occurrence dates and date filters, plus searchable IANA timezone dropdowns.
- Require an explicit offset for stored ambiguous clock-change times, and enforce completion rules against parent occurrence statuses.
- In-app reminders with a catch-up window while the app runs.
- Separate core/UI architecture with generic SQL access, regression tests, coverage commands, CI and Cargo package metadata.

### Workspace refinements
- Added configurable sortable task table, compact alphabetical hierarchy, and filtered parent attachment picker.
- Added tag RGB picker, calendar tag stripes, selected display timezone and empty-day task creation.
- Simplified editor sections and copy, removed rendered preview, added description height control and richer inline highlighting.
- Apply closes the drawer on success; Done fills an editable completion timestamp immediately.
- Added guarded Close/Quit actions and counted Done/Won't Do together in progress reports.

### Table and editor polish
- Added column reordering controls, full-cell tooltips, opaque status row colors and per-tag stripes.
- Added a palette/RGB color dialog with Select/Cancel and hex output on Tags.
- Added inline Overview deadlines, including rescheduled occurrence deadlines, and enlarged the navigation motto.
- Widened task drawers, put Schedule labels beside inputs, and added Plan for Today with guarded apply-and-close behavior.

### Event status
- Added Event to status controls, filters, storage and occurrences. Event hides task-only dates, effort and Plan for Today; applying it clears those dates and zeroes effort. Version-1 documents migrate safely in memory to version 2.

- Fixed notification placement: workspace feedback stays on the main page while task and occurrence drawers are open.

- Improved large All tasks lists by rendering only visible/nearby rows and caching the sorted/filtered order across scroll updates. Preserved full scroll range, stable task identity, tooltips and column controls.

### MCP and 1.0.0
- Added an MCP page with prefilled loopback/port/token settings, optional editing permission, Start/Stop controls and copyable Codex setup instructions.
- Added authenticated stateless Streamable HTTP task/tag tools, core validation, local-edit protection and automatic shutdown when closing or replacing the workspace.
- Updated application and package version to 1.0.0.
