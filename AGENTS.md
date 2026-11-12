# antodo contributor instructions

## Required architecture

Keep **all application/domain logic in `src/core/`** and **all GUI handling in `src/ui/`**. `src/main.rs` only starts Iced. The core must compile and pass tests with `--no-default-features`, without Iced. UI may translate input into domain values and render results, but must not implement task invariants, recurrence, reports, or persistence.

Access SQLite using **generic parameterized SQL functions against a database connection** (`core::sql`). No ORM, per-entity repository framework, or SQL inside UI code. Keep schema and migrations in core. Use transactional changes for items, relationships, tags, and occurrences. Never concatenate user input into SQL.

## Invariants

- UUIDv7 record IDs are internal and never rendered as editable GUI fields.
- Multiple parents form a directed acyclic graph. Reject cycles atomically.
- Done requires every child to be Done or Won't Do. Reopening a child must not invalidate a completed ancestor.
- Start/end are both absent or both supplied, in chronological order.
- Dates retain date-only versus date-time precision and their IANA timezone. Reject ambiguous/nonexistent user-entered local times rather than silently shifting them. Generated RFC occurrences skip nonexistent times and select the first repeated time; retain explicit fold offsets when needed.
- Completion is set on transition to Done unless explicitly supplied, and cleared otherwise.
- Recurrence requires exactly one finite COUNT or UNTIL end criterion. Bound expansion and report limits; never silently drop occurrences. Event overrides participate in calendar expansion and reminders.
- File operations must preserve the previous document on errors. Never overwrite an unrelated file without explicit user selection/confirmation. Protect unsaved changes.

## Development

Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, `cargo test --no-default-features`, `cargo llvm-cov --all-features --summary-only`, and `cargo package` before release. Add meaningful regression tests for business rules, storage, recurrence, and UI transitions. Keep README and the requirement checklist current. Do not publish or create remote repositories without a user request. License is GPL-2.0-only.
