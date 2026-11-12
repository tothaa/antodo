# Requirements and verification

This checklist maps the requested behavior to its implementation and automated regression tests. It describes the test scope, not a claim that every desktop environment has been manually exercised.

| Requirement | Implementation | Automated evidence |
| --- | --- | --- |
| Cargo-installable Rust application with Iced | `Cargo.toml`, `src/main.rs`, `src/ui/` | Full build, Clippy, `cargo package`; GUI state tests where applicable. |
| GPL-2.0-only; GitHub-ready local source | Manifest, `LICENSE`, `.gitignore`, release guide | Package manifest/file inspection. Remote publication remains a separate owner action; dependency licensing remains an unresolved release gate. |
| Strict core/UI separation; generic SQL functions | `src/core/`, `src/ui/`, `core::sql`, `AGENTS.md` | `cargo test --no-default-features`; `generic_sql_functions_parameterize_values_and_missing_rows`. |
| Reusable helpers preserve existing behavior | Shared calendar intervals/entries, ordered SQL writes, provider execution and UI save/control helpers | Full/default and core-only suites; `replacing_relationships_preserves_order_and_removes_stale_values`, `missing_record_commands_leave_a_clean_document_unchanged`, `failed_save_before_new_cancels_continuation_and_retains_work`, picker precision and MCP lookup regressions. |
| Rendered controls and background guards | Headless Iced layout/draw/click harness in `ui::views::render_tests` | Both themes and responsive widths; page labels, masked inputs, task/event/occurrence controls, modal click routing, table selection/sort and MCP/LLM controls. Native portals remain a manual check. |
| External commands preserve local drafts and can be cancelled | `core::llm`, `core::mcp`, `ui::app` | Local HTTP providers, fake CLI processes, blocked-stdin cancellation/timeout, malformed MCP traffic, expired requests, all local MCP guards, chat update/delete refresh and draft-preservation tests. |
| New/open/save/save as `.antodo` SQLite | `core::store::Document` and UI dialogs | `sqlite_file_round_trips_all_fields_and_relations`, `unsaved_edits_stay_in_memory_and_failed_save_preserves_work`, external-change and invalid-file tests. |
| Reopen last file; remember theme/content size | `core::preferences`, UI startup | All `tests/preferences.rs` tests; UI startup/state tests. |
| Every requested item property and UUIDv7 | `core::model`, schema, editor | Full-field round trip; dates/priority/name/UUID validation. |
| Full Markdown details and truncated list description | `ui::editor`, `ui::views` | State/view construction tests; visual desktop check below. |
| Direct task editing in a side drawer; top-left Close button | `ui::views`, guarded drawer-close intent | UI close/apply/discard/cancel and Escape tests; wide/narrow drawer previews. Current page, filters and working document changes are retained. |
| Date or datetime with timezone for all date fields | `Moment`, editor date inputs and calendar/time selector; searchable IANA timezone dropdown | `ordinary_tasks_preserve_independent_date_precision_and_timezones`, DST and date-only deadline tests; skipped/repeated midnight display, fold-preserving editor conversion, picker state and validation tests. |
| Start/end both set or both absent | Core validation | Date validation and recurrence pair tests. |
| Multiple parents with no cycles | `save_item`, graph validation | `multiple_parents_are_supported_and_cycles_rejected`. |
| Done blocked by open children; completion auto/clear | Domain commands | `done_requires_closed_children_and_reopening_preserves_invariant`, `completion_default_manual_preservation_and_clearing`, occurrence completion tests. |
| Colored task-editor status buttons | Edit task drawer and core `save_item` | UI staged/applied status-transition tests, including rejected child/parent transitions and completion dates. |
| User-defined colored tags and multicolor task stripes | Tag commands and task cards | `tags_are_predefined_unique_colored_and_cascade_cleanly`; visual check for stripes. |
| Finite RFC 5545 recurrence | `core::recurrence` | Daily/weekly/monthly/BYDAY/BYSETPOS/leap-year/COUNT/UNTIL/DST tests, start-rule mismatch and rejection/safety-limit tests. |
| Per-occurrence override table and calendar application | `event_overrides`, recurrence and editor | Moved-range, cancellation, status, duplicate/invalid override and restore tests. |
| Reminders and independent acknowledgement state | Query decisions, storage commands, UI timer | Reminder lead-window, old/closed suppression, occurrence move/cancellation, acknowledgement preservation tests. |
| Overview: overdue, ongoing, today, next days, nearest deadlines, workload and recent completions | `query::dashboard`, `query::report`, Overview view | `dashboard_prioritizes_overdue_and_respects_occurrence_completion`, `far_future_recurrence_is_still_a_nearest_deadline`, report tests and native preview. |
| All-task list and parent/child outline | Tasks/Outline views, hierarchy query | Shared reference, malformed-cycle and 10,000-level hierarchy tests. |
| Calendar start/end spans, deadlines and plans with clock times/timezones | `query::calendar_entries`, responsive Calendar view | Range/span, all-day, shifted marker, DST and occurrence tests; native timed calendar preview. |
| Completed-task reports and percentages in Overview | `query::report`, Overview view | `reports_distinguish_done_from_abandoned_and_aggregate_effort_tags`. |
| Property filters and name/description search | `Filter`, common view controls | `search_and_all_property_filters_compose`, effective occurrence status/date/notification filters and invalid-range tests; UI filter validation. |
| No editable technical IDs or database controls | Named choices in views | Source/view review and desktop check. |
| Light/dark and remembered content scaling | Theme palette and scale factor | Preference round trip and UI state tests; desktop check at smallest/largest scale. |
| Searchable Markdown Notes after Calendar; CRUD and internal UUIDv7 | `core::notes`, `ui::notes`, schema v3 | Notes storage/search/migration tests and guarded editor transitions. |
| Notes MCP read/search/create/update/delete capabilities | `core::mcp` | MCP tool discovery, read-only/mutation guards and Notes persistence tests. |
| Task context menus and Shift+click selection on all four task screens | `ui::task_interactions`, common task wrappers | Pointer-driven menu/selection tests and bulk action state tests. |
| Atomic hierarchy moves/additional parents, including multiple selections | `core::actions`, `Document::save_items`, hierarchy row parent identity | Cycle/rollback, displayed-parent preservation and final-graph completion tests; drag event tests. |
| Native icon and Wayland application identity | `ui::icon`, desktop entry, SVG/raster and desktop installer | Embedded dimensions/application-ID test; native KDE acceptance below. |
| Notes share tag catalog and show assigned tag color stripes | `core::notes`, `ui::notes`, shared `tag_stripes` | Relationship persistence and rendered stripe color/geometry tests. |
| Notes and tasks have editable many-to-many assignments | `note_tasks`, `save_note`, `save_item_with_notes`, named drawer selectors | Two-sided assignment, new/existing tasks, invalid-reference and forced-SQL rollback, cascade and unsaved-navigation tests. |
| MCP and LLM chat operate on tagged, linked Notes | Shared tool catalog; typed task/note chat context | Tool schema/filter/patch tests, mock provider note CRUD/links, read-only/local draft guards and clean editor refresh. |
| Protect unsaved edits; preserve failed operations | UI transition guards and atomic core operations | UI transition tests and storage rejection/failed-save tests. |

See [the completed validation run](VALIDATION.md) for measured test and coverage results.

## Automated commands

Run from the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --no-default-features
cargo llvm-cov --no-default-features --summary-only
cargo llvm-cov --all-features --summary-only
cargo package --locked
```

`cargo-llvm-cov` and the `llvm-tools-preview` rustup component must be installed first. The CI workflow repeats these checks on Linux. Core-only coverage isolates business logic; full-feature coverage includes UI code exercised by tests. Coverage percentages do not establish that native rendering, keyboard focus, desktop portals or window-manager behavior work on every platform.

## Native rendering harness

The `ui_preview` example opens a real Iced window with sample data, captures one screen to a PPM image, then exits. It creates an independent in-memory document and does not load the user's task file or preferences. Run it on an available desktop (or a configured virtual desktop):

```sh
cargo run --example ui_preview -- /tmp/antodo-preview 0
```

The final argument selects one screen per process:

| Index | Captured screen |
| --- | --- |
| 0 | Overview, dark theme |
| 1 | Overview, light theme |
| 2 | Task list |
| 3 | Parent/child hierarchy |
| 4 | Calendar |
| 5 | Date/time selector |
| 6 | Edit task drawer over the task list |
| 7 | Task editor drawer over the task list |
| 8 | Tags |
| 9 | Large content size |
| 10 | Date/time selector in a narrow window |
| 11 | Edit task drawer in a narrow window |
| 12 | Overview scrolled to workload, tag progress and recent completions |
| 13 | Task editor drawer in a narrow window |
| 14 | Task editor in light theme |
| 15 | Calendar in a narrow window |

The output directory receives a named `.ppm` image such as `overview-dark.ppm`. Convert it to PNG with an ordinary image converter if needed. Capture each screen in a separate invocation. The harness captures directly from its startup task: introducing application messages before capture can invalidate Iced's cached editor text references and produce an inaccurate screenshot. This harness verifies actual native drawing; it does not automate file portals, typing, window-manager interactions, or operating-system notifications. The sample's dates follow the current machine date, so snapshots are intended for visual review rather than pixel-identical comparison across days.

## Native desktop release checklist

Record the OS, desktop session (X11/Wayland), scaling, and result when exercising these checks:

1. Start without preferences; create a file; add tasks and tags; save; close; relaunch and verify the same file opens.
2. Open another valid file. Try a missing, non-SQLite and future-version file; retain the current workspace on failure.
3. Leave task and occurrence edits unapplied; switch view, open a file or close the window; verify save/discard/stay paths and dialog cancellation.
4. Read a long Markdown description, edit multiple paragraphs, resize the window, and confirm cards truncate while details retain the full content.
5. Assign several colored tags and several parents; verify visible stripe colors, shared hierarchy labels, and cycle/completion validation messages.
6. Navigate calendar months; create all-day and timed spans crossing month and clock-change boundaries; move/cancel/restore one occurrence. Confirm clock times and timezones are visible and all-day entries are labelled.
7. Filter every view and compare its result to the task fields. Verify occurrence-status filters after overriding a recurring task.
8. Exercise a reminder with the app open; dismiss, save, restart and ensure that occurrence does not repeat. Check a later occurrence independently.
9. Toggle both themes and content sizes 12–28; resize to the minimum window; confirm controls, scrollbars and text remain usable. After every theme switch, move the pointer across the main page repeatedly without opening a drawer or restarting; the background must stay uniform. Repeat after opening and closing a drawer.
10. Use keyboard shortcuts and native file dialogs; test Save As overwrite confirmation and cancelled dialogs.
11. Try a second editor against the same file; confirm changed-file protection. Keep a backup and inspect saved files with SQLite if desired.
12. Run the installed executable from outside the repository, including a filename containing spaces.
13. Change each status using task-editor status buttons. Apply the selection and verify completion is set/cleared, invalid parent/child changes are rejected, and Save persists successful changes.
14. Use the calendar/time selector on task and occurrence fields and date filters. Check date-only/timed selection, leap days, month navigation, cancellation, and validation of clock-change times. Search and select an IANA timezone from the dropdown.
15. Compare Overview workload/progress and recent completions with the task data, including empty results and active filters. Verify that navigation has no Insights tab.
16. Open a task from each page, edit it, add a new task, and edit an occurrence. Confirm each uses a right drawer while the current page remains visible. Move the pointer repeatedly across the drawer and its controls in both themes; colors must stay stable. Close using the top-left Close button, dimmed backdrop, and Escape; verify clean dismissal and apply/discard/stay paths for unapplied edits, with page and filters preserved.

The source supports desktop backends through Iced. Actual Windows/macOS release claims require running these checks on those platforms. No cloud sync, closed-app background scheduler, email notifications, or calendar import/export is included in this version.

17. Select tasks from Overview, Tasks, Hierarchy and Calendar; confirm the editable drawer opens directly, Apply closes it only after successful validation, Delete confirms, and no read-only details or Cancel editing control remains. Check rating arrows stop at 1/5, resizable Markdown source highlighting updates while typing, date/timezone controls share a row, and Calendar columns resize at wide/narrow widths.

18. Check Tags RGB picker, absence of New task/Clear/help copy, Calendar tag stripes and timezone selection. Verify all-day bounds across DST and the date line. Clicking empty day space prefills paired dates in the selected zone; clicking a task opens its editor.
19. Sort every table column both ways, configure visibility, and verify descriptions stay on one line. Check alphabetic siblings, root markers, cycling bullets and multiple-parent labels in Hierarchy.
20. Add/filter/detach parents, verify cycle rejection on Apply, resize Description, and check the combined Schedule section. Done immediately fills completion; user timestamps survive Apply and blank timestamps default to now.
21. Close/Quit with saved and unsaved work; cancel and save-failure paths must retain the document. Reports count Done and Won't Do as closed.

22. In All tasks → Columns, move visible columns left/right; check sorting survives reordering and first/last controls respect boundaries. Hover cells to inspect full text. Verify opaque status row colors and separate stripes for every tag in both themes.
23. On Tags, open Pick color, choose a swatch or adjust RGB, and check the live hex preview. Select transfers the value to the form; Cancel/Escape preserves it, and background actions remain blocked.
24. Check Overview deadlines beside task names, including moved recurring occurrences. In the wider editor, check inline Schedule labels and Plan for Today: successful validation applies all draft changes and closes; failures preserve the draft.

Additional preview indices: 16 = color dialog; 17 = table column controls.

25. Select Event and verify the Deadline, Planned, Completed, Effort/points and Plan for Today controls disappear; Start/End and recurrence remain. Apply, Save and reopen the event. Open a version-1 document and confirm migration does not touch disk until Save. Preview index 18 shows the Event editor.

26. While editing a task or occurrence, trigger a notification. Verify feedback stays in the main page alert area and does not appear above the drawer form.

27. With 10,000 tasks, scroll and drag the scrollbar from first to last row, resize, filter down to a few/zero rows, sort and reorder columns. Confirm full range, aligned rows, correct task selection/tooltips, and no blank list after returning from another page. Preview indices 19/20 show a large list at top/bottom. Run the manual construction benchmark with `cargo test benchmark_large_table_construction -- --ignored --nocapture`.

28. MCP navigation is disabled without a workspace. Open a workspace, inspect default settings, Start, initialize from an MCP client and list tools/tasks. Read-only is the default; enable editing before Start to create/update/delete. Check malformed config, occupied ports, token/Origin rejection, local-draft write protection, Stop/restart and shutdown on file close/replacement. Preview indices 21/22 show stopped/running MCP pages. Confirm `antodo --version` reports 1.0.0.


29. LLM page is available only with an open workspace. Configure local Ollama, OpenAI API, Gemini compatibility API or Codex CLI; verify instructions and masked key. After Use this connection, check Start chat on the main header and before Plan for Today in the editor. Test selected task reads, multi-turn context, read-only rejection, allowed mutations, domain validation, draft protection, UI refresh and Save persistence. Cancel/close a pending request, clear history, disconnect and change workspace. Check errors for unavailable models/credentials/commands; no keys/history should be written to preferences or task files. Inspect native preview indices 23 (setup) and 24 (task chat). Provider integrations require user-configured accounts/services for live acceptance; automated tests use an isolated mock HTTP provider.

30. Create and search Notes by name and Markdown content; edit/delete, test apply/discard/stay, save/reopen, and use the Notes MCP tools with read-only and editing configurations.
31. Right-click tasks in Overview, All tasks, Hierarchy and Calendar. Exercise submenus, timestamped Done, parent picker, Add Child, deletion and Shift+click bulk selection. Verify menus fit near every window edge and close on Escape/outside click.
32. Drag single and selected groups in Hierarchy onto tasks and empty space; repeat with Ctrl (Option on macOS) and inspect the + indicator. Verify only displayed links change, unrelated parents remain, and a cyclic/invalid group leaves every task unchanged.
33. Install the desktop launcher/icon with `sh assets/install-desktop.sh`, start the application on KDE Wayland and verify the taskbar shows the antodo icon and groups with its launcher. The native compositor check is separate from automated settings tests.

Additional preview indices: 25 = Notes; 26 = note editor; 27 = light note editor; 28 = task context/status menu; 29 = light task context/status menu.

34. Tag a Note with several colors and inspect separate stripes on Notes cards, note headers and task-side note choices. Assign the same Note to two tasks and several Notes to one task. Edit links from both drawers, save/reopen, and remove a task/tag without deleting the Note. Check apply/discard/stay when opening linked records with dirty selections.
35. Start chat from a saved Note, confirm Note chat context, ask to read/edit content and manage tags/task links, and verify applied changes refresh the drawer. Dirty drafts and read-only chat must reject writes. Use MCP `list_notes` with tag/task filters and edit task `note_ids`; verify omitted arrays preserve links and empty arrays clear them.

Additional preview indices: 30 = task Notes selector; 31 = Note chat. Notes previews 25–27 now include tag stripes and shared task links.

36. In Edit task → Notes and Edit note → Assigned tasks, verify only assigned records appear with Detach and Add… controls. Add… opens Add note/Add task; filter by text/tag and, for tasks, status/priority. Select several candidates and verify each disappears from results, close with Close/backdrop/Escape, and check changes remain staged until Apply. Background actions and external writes are blocked while the dialog is open. Detach then discard must preserve stored links.

Preview indices 32/33 show Add note/Add task dialogs.

37. Open a linked Note from a task drawer, then a task from that Note, continuing for several levels. Close each level with its button, backdrop or Escape and verify the previous record returns. Repeat in the opposite direction. Check apply/discard/stay and validation failure, Apply/Delete returning one level, refreshed shared links, and page/workspace navigation clearing the return chain.

38. Right-click a task on Overview, All tasks, Hierarchy and Calendar and choose Link a Note. Filter by text/tag, link a note, and confirm the dialog closes and the assignment appears in both editors. Repeat with Shift-selected tasks and partially existing links; retain all existing links without duplicates. Notes linked to every selected task should be excluded. Cancel/Escape must leave links unchanged; stale/missing selections must reject the whole operation and keep the dialog open with an error.
