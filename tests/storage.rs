use antodo::core::{Document, EventOverride, Item, Moment, Reminder, Status, Tag};
use chrono::{Duration, Utc};
use rusqlite::Connection;
use uuid::Uuid;

fn moment(value: &str) -> Moment {
    Moment::parse(value, "Europe/Budapest").unwrap()
}
fn recurring() -> Item {
    let mut item = Item::new("Weekly review");
    item.start = Some(moment("2026-11-10 09:00"));
    item.end = Some(moment("2026-11-10 10:00"));
    item.is_recurring = true;
    item.rrule = "FREQ=WEEKLY;COUNT=4".into();
    item
}
fn event(item: &Item) -> EventOverride {
    EventOverride {
        id: Uuid::now_v7(),
        item_id: item.id,
        original_start: item.start.clone().unwrap(),
        original_end: item.end.clone().unwrap(),
        new_start: None,
        new_end: None,
        cancelled: false,
        status: Status::Pending,
        reminded: false,
    }
}

#[test]
fn sqlite_file_round_trips_all_fields_and_relations() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    let tag = document.save_tag(Tag::new("Important", "#f97316")).unwrap();
    let parent = document.save_item(Item::new("Project")).unwrap();
    let other = document.save_item(Item::new("Another project")).unwrap();
    let mut item = recurring();
    item.description = "# Full Markdown\n\nA long **description** with Unicode: Árvíz".into();
    item.tags = vec![tag.id];
    item.parents = vec![parent.id, other.id];
    item.participants = vec!["Alice".into(), "bob@example.com".into()];
    item.priority = 1;
    item.severity = 5;
    item.effort = 13;
    item.reminder = Some(30);
    item.planned = Some(moment("2026-11-09"));
    item.deadline = Some(moment("2026-12-01 12:30"));
    let item = document.save_item(item).unwrap();
    let mut override_event = event(&item);
    override_event.new_start = Some(moment("2026-11-10 11:00"));
    override_event.new_end = Some(moment("2026-11-10 12:00"));
    override_event.status = Status::Wip;
    let override_event = document.save_override(override_event).unwrap();
    assert!(document.is_dirty());
    document.save_as(&path).unwrap();
    assert!(!document.is_dirty());
    assert_eq!(&std::fs::read(&path).unwrap()[..16], b"SQLite format 3\0");
    let reopened = Document::open(&path).unwrap();
    assert_eq!(
        reopened
            .items()
            .unwrap()
            .into_iter()
            .find(|saved| saved.id == item.id)
            .unwrap(),
        item
    );
    assert_eq!(reopened.tags().unwrap(), vec![tag]);
    assert_eq!(reopened.overrides().unwrap(), vec![override_event]);
    assert!(!reopened.is_dirty());
}

#[test]
fn replacing_relationships_preserves_order_and_removes_stale_values() {
    let mut document = Document::new().unwrap();
    let parents: Vec<_> = ["First parent", "Second parent", "Third parent"]
        .into_iter()
        .map(|name| document.save_item(Item::new(name)).unwrap().id)
        .collect();
    let tags: Vec<_> = ["First tag", "Second tag", "Third tag"]
        .into_iter()
        .map(|name| document.save_tag(Tag::new(name, "#123456")).unwrap().id)
        .collect();
    let mut item = Item::new("Related task");
    item.parents = parents.clone();
    item.tags = tags.clone();
    item.participants = vec!["Alice".into(), "Bob".into(), "Carol".into()];
    item = document.save_item(item).unwrap();

    item.parents = vec![parents[2], parents[0]];
    item.tags = vec![tags[2], tags[0]];
    item.participants = vec!["Carol".into(), "D'Angelo; --".into()];
    item = document.save_item(item).unwrap();
    let stored = document
        .items()
        .unwrap()
        .into_iter()
        .find(|stored| stored.id == item.id)
        .unwrap();
    assert_eq!(stored, item);

    let mut rejected = item.clone();
    rejected.parents = vec![rejected.id];
    rejected.tags.clear();
    rejected.participants.clear();
    assert!(document.save_item(rejected).is_err());
    assert!(document.items().unwrap().contains(&item));

    item.parents.clear();
    item.tags.clear();
    item.participants.clear();
    let item = document.save_item(item).unwrap();
    assert!(document.items().unwrap().contains(&item));
    assert_eq!(document.items().unwrap().len(), 4);
    assert_eq!(document.tags().unwrap().len(), 3);
}

#[test]
fn missing_record_commands_leave_a_clean_document_unchanged() {
    let mut document = Document::new().unwrap();
    let missing = Uuid::now_v7();
    assert!(document.delete_item(missing).is_err());
    assert!(document.delete_tag(missing).is_err());
    assert!(document.delete_override(missing).is_err());
    assert!(
        document
            .mark_reminded(&Reminder {
                item_id: missing,
                name: "Missing".into(),
                at: moment("2026-11-10"),
                occurrence: None,
            })
            .is_err()
    );
    assert!(!document.is_dirty());
    assert!(document.items().unwrap().is_empty());
    assert!(document.tags().unwrap().is_empty());
    assert!(document.overrides().unwrap().is_empty());
}

#[test]
fn dates_priority_names_and_uuid_are_validated_atomically() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new(" ");
    assert!(document.save_item(item.clone()).is_err());
    item.name = "Task".into();
    item.id = Uuid::nil();
    assert!(document.save_item(item.clone()).is_err());
    item.id = Uuid::now_v7();
    item.priority = 0;
    assert!(document.save_item(item.clone()).is_err());
    item.priority = 3;
    item.severity = 6;
    assert!(document.save_item(item.clone()).is_err());
    item.severity = 3;
    item.start = Some(moment("2026-11-10"));
    assert!(document.save_item(item.clone()).is_err());
    item.end = Some(moment("2026-11-09"));
    assert!(document.save_item(item.clone()).is_err());
    item.end = Some(moment("2026-11-10"));
    item.deadline = Some(Moment {
        value: "bad".into(),
        timezone: "UTC".into(),
        offset_seconds: None,
    });
    assert!(document.save_item(item.clone()).is_err());
    item.deadline = None;
    assert_eq!(document.items().unwrap().len(), 0);
    document.save_item(item).unwrap();
    assert_eq!(document.items().unwrap().len(), 1);
}

#[test]
fn ambiguous_times_require_an_explicit_fold_offset_in_storage() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("ambiguous.antodo");
    let mut document = Document::new().unwrap();
    let mut item = document
        .save_item(Item::new("Repeated clock time"))
        .unwrap();
    document.save_as(&path).unwrap();
    let ambiguous = Moment {
        value: "2026-10-25 02:30".into(),
        timezone: "Europe/Budapest".into(),
        offset_seconds: None,
    };
    assert!(ambiguous.utc().is_err());
    item.planned = Some(ambiguous.clone());
    assert!(document.save_item(item.clone()).is_err());
    assert!(!document.is_dirty());
    assert_eq!(document.items().unwrap()[0].planned, None);

    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "UPDATE items SET planned_date=?1 WHERE id=?2",
            [
                serde_json::to_string(&ambiguous).unwrap(),
                item.id.to_string(),
            ],
        )
        .unwrap();
    drop(connection);
    let bytes = std::fs::read(&path).unwrap();
    assert!(Document::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);

    let mut explicit = ambiguous;
    explicit.offset_seconds = Some(3600);
    item.planned = Some(explicit.clone());
    assert_eq!(document.save_item(item).unwrap().planned, Some(explicit));
}

#[test]
fn multiple_parents_are_supported_and_cycles_rejected() {
    let mut document = Document::new().unwrap();
    let a = document.save_item(Item::new("A")).unwrap();
    let b = document.save_item(Item::new("B")).unwrap();
    let mut c = Item::new("C");
    c.parents = vec![a.id, b.id];
    let c = document.save_item(c).unwrap();
    let mut d = Item::new("D");
    d.parents = vec![c.id];
    let d = document.save_item(d).unwrap();
    let mut changed = a.clone();
    changed.parents = vec![d.id];
    assert!(
        document
            .save_item(changed)
            .unwrap_err()
            .to_string()
            .contains("loop")
    );
    let mut changed = b.clone();
    changed.parents = vec![b.id];
    assert!(document.save_item(changed).is_err());
    let mut missing = b;
    missing.parents = vec![Uuid::now_v7()];
    assert!(document.save_item(missing).is_err());
    assert!(
        document
            .items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == a.id)
            .unwrap()
            .parents
            .is_empty()
    );
}

#[test]
fn done_requires_closed_children_and_reopening_preserves_invariant() {
    let mut document = Document::new().unwrap();
    let mut parent = document.save_item(Item::new("Parent")).unwrap();
    let mut child = Item::new("Child");
    child.parents = vec![parent.id];
    child = document.save_item(child).unwrap();
    parent.status = Status::Done;
    assert!(document.save_item(parent.clone()).is_err());
    child.status = Status::WontDo;
    child = document.save_item(child).unwrap();
    parent = document.save_item(parent).unwrap();
    assert!(parent.completion.is_some());
    child.status = Status::Pending;
    assert!(document.save_item(child.clone()).is_err());
    let mut new_child = Item::new("New child");
    new_child.parents = vec![parent.id];
    assert!(document.save_item(new_child).is_err());
    parent.status = Status::Wip;
    parent = document.save_item(parent).unwrap();
    assert!(parent.completion.is_none());
    document.save_item(child).unwrap();
}

#[test]
fn completion_default_manual_preservation_and_clearing() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Done");
    item.status = Status::Done;
    let before = Utc::now() - Duration::seconds(1);
    item = document.save_item(item).unwrap();
    assert!(item.completion.as_ref().unwrap().utc().unwrap() >= before);
    item.completion = Some(moment("2020-01-01"));
    item = document.save_item(item).unwrap();
    item.completion = None;
    item = document.save_item(item).unwrap();
    assert!(item.completion.as_ref().unwrap().utc().unwrap() >= before);
    item.completion = Some(moment("2025-01-01"));
    item = document.save_item(item).unwrap();
    assert_eq!(item.completion.as_ref().unwrap().value, "2025-01-01");
    item.status = Status::OnHold;
    item = document.save_item(item).unwrap();
    assert!(item.completion.is_none());
}

#[test]
fn tags_are_predefined_unique_colored_and_cascade_cleanly() {
    let mut document = Document::new().unwrap();
    assert!(document.save_tag(Tag::new(" ", "#FFFFFF")).is_err());
    assert!(document.save_tag(Tag::new("Bad", "red")).is_err());
    let mut tag = document.save_tag(Tag::new("Work", "#ff0000")).unwrap();
    assert_eq!(tag.color, "#FF0000");
    assert!(document.save_tag(Tag::new("work", "#000000")).is_err());
    tag.color = "#00FF00".into();
    document.save_tag(tag.clone()).unwrap();
    let mut item = Item::new("Tagged");
    item.tags = vec![Uuid::now_v7()];
    assert!(document.save_item(item.clone()).is_err());
    item.tags = vec![tag.id, tag.id];
    item.participants = vec![" Alice ".into(), "Alice".into(), " ".into()];
    let item = document.save_item(item).unwrap();
    assert_eq!(item.tags, vec![tag.id]);
    assert_eq!(item.participants, vec!["Alice"]);
    document.delete_tag(tag.id).unwrap();
    assert!(document.items().unwrap()[0].tags.is_empty());
    assert!(document.delete_tag(tag.id).is_err());
}

#[test]
fn overrides_validate_master_dates_and_upsert_semantic_occurrence() {
    let mut document = Document::new().unwrap();
    let item = document.save_item(recurring()).unwrap();
    let mut changed = event(&item);
    changed.original_start = moment("2026-11-11 09:00");
    changed.original_end = moment("2026-11-11 10:00");
    assert!(document.save_override(changed).is_err());
    let first = document.save_override(event(&item)).unwrap();
    let mut second = event(&item);
    second.original_start = moment("2026-11-10 09:00:00");
    second.original_end = moment("2026-11-10 10:00:00");
    second.status = Status::Done;
    let second = document.save_override(second).unwrap();
    assert_eq!(first.id, second.id);
    assert_eq!(document.overrides().unwrap().len(), 1);
    document.delete_override(first.id).unwrap();
    assert!(document.overrides().unwrap().is_empty());
    assert!(document.delete_override(first.id).is_err());
}

#[test]
fn reminder_acknowledgement_preserves_occurrence_moves() {
    let mut document = Document::new().unwrap();
    let mut item = recurring();
    item.reminder = Some(15);
    let item = document.save_item(item).unwrap();
    let mut moved = event(&item);
    moved.new_start = Some(moment("2026-11-11 09:00"));
    moved.new_end = Some(moment("2026-11-11 10:00"));
    moved.status = Status::Wip;
    let moved = document.save_override(moved).unwrap();
    let occurrences = antodo::core::recurrence::expand(
        &item,
        std::slice::from_ref(&moved),
        moment("2026-11-11").utc().unwrap(),
        moment("2026-11-12").utc().unwrap(),
    )
    .unwrap();
    let occurrence = occurrences[0].clone();
    document
        .mark_reminded(&Reminder {
            item_id: item.id,
            name: item.name.clone(),
            at: occurrence.start.clone(),
            occurrence: Some(occurrence),
        })
        .unwrap();
    let saved = document.overrides().unwrap().remove(0);
    assert!(saved.reminded);
    assert_eq!(saved.new_start, moved.new_start);
    assert_eq!(saved.status, Status::Wip);
    let mut plain = Item::new("Plain");
    plain.planned = Some(moment("2026-11-10"));
    plain.reminder = Some(1);
    let plain = document.save_item(plain).unwrap();
    document
        .mark_reminded(&Reminder {
            item_id: plain.id,
            name: plain.name,
            at: plain.planned.unwrap(),
            occurrence: None,
        })
        .unwrap();
    assert!(
        document
            .items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == plain.id)
            .unwrap()
            .reminded
    );
}

#[test]
fn changing_schedule_clears_stale_overrides_and_rearms_reminders() {
    let mut document = Document::new().unwrap();
    let mut item = document.save_item(recurring()).unwrap();
    document.save_override(event(&item)).unwrap();
    item.reminded = true;
    item.start = Some(moment("2026-11-12 09:00"));
    item.end = Some(moment("2026-11-12 10:00"));
    let item = document.save_item(item).unwrap();
    assert!(!item.reminded);
    assert!(document.overrides().unwrap().is_empty());
}

#[test]
fn deletion_cascades_relationships_and_occurrences_but_keeps_children() {
    let mut document = Document::new().unwrap();
    let parent = document.save_item(recurring()).unwrap();
    document.save_override(event(&parent)).unwrap();
    let mut child = Item::new("Child");
    child.parents = vec![parent.id];
    document.save_item(child).unwrap();
    document.delete_item(parent.id).unwrap();
    assert!(document.overrides().unwrap().is_empty());
    assert_eq!(document.items().unwrap().len(), 1);
    assert!(document.items().unwrap()[0].parents.is_empty());
    assert!(document.delete_item(parent.id).is_err());
}

#[test]
fn unsaved_edits_stay_in_memory_and_failed_save_preserves_work() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    assert!(document.save().is_err());
    document.save_as(&path).unwrap();
    document.save_item(Item::new("Unsaved")).unwrap();
    assert!(Document::open(&path).unwrap().items().unwrap().is_empty());
    assert!(
        document
            .save_as(&directory.path().join("missing").join("fail.antodo"))
            .is_err()
    );
    assert!(document.is_dirty());
    assert_eq!(document.items().unwrap().len(), 1);
    document.save().unwrap();
    assert_eq!(Document::open(&path).unwrap().items().unwrap().len(), 1);
}

#[test]
fn unrelated_future_and_changed_files_are_not_overwritten() {
    let directory = tempfile::tempdir().unwrap();
    let unrelated = directory.path().join("foreign.antodo");
    let connection = Connection::open(&unrelated).unwrap();
    connection
        .execute("CREATE TABLE secret(value TEXT)", [])
        .unwrap();
    drop(connection);
    let original = std::fs::read(&unrelated).unwrap();
    let mut document = Document::new().unwrap();
    assert!(document.save_as(&unrelated).is_err());
    assert_eq!(original, std::fs::read(&unrelated).unwrap());
    assert!(Document::open(&unrelated).is_err());
    assert!(Document::open(&directory.path().join("missing.antodo")).is_err());
    assert!(
        document
            .save_as(&directory.path().join("wrong.sqlite"))
            .is_err()
    );
    let path = directory.path().join("work.antodo");
    document.save_as(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 999).unwrap();
    drop(connection);
    assert!(Document::open(&path).is_err());
    assert!(document.save().is_err());
}

#[test]
fn reminder_requires_schedule_and_bounded_minutes() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Reminder");
    item.reminder = Some(15);
    assert!(document.save_item(item.clone()).is_err());
    item.deadline = Some(moment("2026-11-10"));
    item.reminder = Some(525_601);
    assert!(document.save_item(item.clone()).is_err());
    item.reminder = Some(0);
    document.save_item(item).unwrap();
}

#[test]
fn generic_sql_functions_parameterize_values_and_missing_rows() {
    use antodo::core::sql::{execute, query, query_optional};
    let connection = Connection::open_in_memory().unwrap();
    execute(&connection, "CREATE TABLE values_table(value TEXT)", []).unwrap();
    let dangerous = "'; DROP TABLE values_table; --";
    execute(
        &connection,
        "INSERT INTO values_table(value) VALUES (?1)",
        [dangerous],
    )
    .unwrap();
    let results: Vec<String> = query(&connection, "SELECT value FROM values_table", [], |row| {
        row.get(0)
    })
    .unwrap();
    assert_eq!(results, vec![dangerous]);
    let missing: Option<String> = query_optional(
        &connection,
        "SELECT value FROM values_table WHERE value=?1",
        ["missing"],
        |row| row.get(0),
    )
    .unwrap();
    assert!(missing.is_none());
}

#[test]
fn ordinary_tasks_preserve_independent_date_precision_and_timezones() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Travel");
    item.start = Some(Moment::parse("2026-11-10", "Europe/Budapest").unwrap());
    item.end = Some(Moment::parse("2026-11-10 12:00", "America/New_York").unwrap());
    let saved = document.save_item(item.clone()).unwrap();
    assert_eq!(saved.start, item.start);
    assert_eq!(saved.end, item.end);
}

#[test]
fn occurrence_completion_respects_children_and_reopening_parent() {
    let mut document = Document::new().unwrap();
    let mut parent = document.save_item(Item::new("Parent")).unwrap();
    let mut item = recurring();
    item.parents = vec![parent.id];
    let mut item = document.save_item(item).unwrap();
    let mut child = Item::new("Child");
    child.parents = vec![item.id];
    let mut child = document.save_item(child).unwrap();
    let mut occurrence = event(&item);
    occurrence.status = Status::Done;
    assert!(document.save_override(occurrence.clone()).is_err());
    child.status = Status::Done;
    document.save_item(child).unwrap();
    document.save_override(occurrence.clone()).unwrap();
    item.status = Status::Done;
    document.save_item(item).unwrap();
    parent.status = Status::Done;
    document.save_item(parent).unwrap();
    occurrence.status = Status::Pending;
    assert!(document.save_override(occurrence).is_err());
}

#[test]
fn removing_file_after_open_does_not_silently_recreate_it() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    document.save_as(&path).unwrap();
    document.save_item(Item::new("Keep me")).unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(document.save().is_err());
    assert!(!path.exists());
    assert!(document.is_dirty());
    let recovered = directory.path().join("recovered.antodo");
    document.save_as(&recovered).unwrap();
    assert_eq!(
        Document::open(&recovered).unwrap().items().unwrap().len(),
        1
    );
}

#[test]
fn bad_documents_are_rejected_without_modifying_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("bad.antodo");
    let mut document = Document::new().unwrap();
    let a = document.save_item(Item::new("A")).unwrap();
    let b = document.save_item(Item::new("B")).unwrap();
    document.save_as(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute(
            "INSERT INTO item_parents(item_id,parent_id,position) VALUES (?1,?2,0)",
            [a.id.to_string(), b.id.to_string()],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO item_parents(item_id,parent_id,position) VALUES (?1,?2,0)",
            [b.id.to_string(), a.id.to_string()],
        )
        .unwrap();
    drop(connection);
    let bytes = std::fs::read(&path).unwrap();
    assert!(Document::open(&path).is_err());
    assert_eq!(bytes, std::fs::read(&path).unwrap());
}

#[test]
fn editing_a_stale_task_snapshot_does_not_rearm_delivered_reminder() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Remember");
    item.planned = Some(moment("2026-11-10 09:00"));
    item.reminder = Some(10);
    let mut stale = document.save_item(item).unwrap();
    document
        .mark_reminded(&Reminder {
            item_id: stale.id,
            name: stale.name.clone(),
            at: stale.planned.clone().unwrap(),
            occurrence: None,
        })
        .unwrap();
    stale.name = "Renamed after the reminder".into();
    let saved = document.save_item(stale).unwrap();
    assert!(saved.reminded);
}

#[test]
fn completed_occurrences_cannot_gain_or_reopen_unfinished_children() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    let item = document.save_item(recurring()).unwrap();
    let mut occurrence = event(&item);
    occurrence.status = Status::Done;
    document.save_override(occurrence).unwrap();
    let mut child = Item::new("New child");
    child.parents = vec![item.id];
    assert!(document.save_item(child.clone()).is_err());
    child.status = Status::WontDo;
    let mut child = document.save_item(child).unwrap();
    child.status = Status::Pending;
    assert!(document.save_item(child).is_err());
    document.save_as(&path).unwrap();
    assert!(Document::open(&path).is_ok());
}

#[test]
fn completing_parent_checks_open_overrides_of_completed_master_child() {
    let mut document = Document::new().unwrap();
    let mut parent = document.save_item(Item::new("Parent")).unwrap();
    let mut item = recurring();
    item.parents = vec![parent.id];
    item.status = Status::Done;
    let item = document.save_item(item).unwrap();
    document.save_override(event(&item)).unwrap();
    parent.status = Status::Done;
    assert!(document.save_item(parent).is_err());
}

#[test]
fn completed_parent_occurrences_reject_reopening_child_occurrences_atomically() {
    let mut document = Document::new().unwrap();
    let parent = document.save_item(recurring()).unwrap();
    let mut child = recurring();
    child.name = "Recurring child".into();
    child.parents = vec![parent.id];
    child.status = Status::Done;
    let child = document.save_item(child).unwrap();
    let mut completed = event(&parent);
    completed.status = Status::Done;
    let completed = document.save_override(completed).unwrap();

    assert!(document.save_override(event(&child)).is_err());
    assert_eq!(document.overrides().unwrap(), vec![completed]);
    assert_eq!(document.items().unwrap().len(), 2);
}

#[test]
fn parent_occurrence_completion_checks_existing_child_occurrence_changes() {
    let mut document = Document::new().unwrap();
    let parent = document.save_item(recurring()).unwrap();
    let mut child = recurring();
    child.name = "Recurring child".into();
    child.parents = vec![parent.id];
    child.status = Status::Done;
    let child = document.save_item(child).unwrap();
    let reopened = document.save_override(event(&child)).unwrap();
    let mut completed = event(&parent);
    completed.status = Status::Done;

    assert!(document.save_override(completed.clone()).is_err());
    assert_eq!(document.overrides().unwrap(), vec![reopened.clone()]);
    let mut cancelled = reopened;
    cancelled.cancelled = true;
    document.save_override(cancelled).unwrap();
    document.save_override(completed).unwrap();
}

#[test]
fn cancelled_occurrences_do_not_block_parent_completion() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    let mut parent = document.save_item(Item::new("Parent")).unwrap();
    let mut item = recurring();
    item.parents = vec![parent.id];
    item.status = Status::Done;
    let item = document.save_item(item).unwrap();
    let mut cancelled = event(&item);
    cancelled.cancelled = true;
    document.save_override(cancelled).unwrap();
    parent.status = Status::Done;
    document.save_item(parent).unwrap();
    document.save_as(&path).unwrap();
    assert!(Document::open(&path).is_ok());
}

#[test]
fn formatting_only_schedule_edits_preserve_overrides_and_reminders() {
    let mut document = Document::new().unwrap();
    let mut item = document.save_item(recurring()).unwrap();
    let mut changed = event(&item);
    changed.reminded = true;
    document.save_override(changed).unwrap();
    item.start = Some(moment("2026-11-10 09:00:00"));
    item.end = Some(moment("2026-11-10 10:00:00"));
    item.rrule = "rrule:count=4;freq=weekly".into();
    document.save_item(item).unwrap();
    let saved = document.overrides().unwrap();
    assert_eq!(saved.len(), 1);
    assert!(saved[0].reminded);
}

#[test]
fn invalid_future_calendar_marker_is_rejected_before_changing_document() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Clock change");
    item.start = Some(moment("2026-03-28 01:00"));
    item.end = Some(moment("2026-03-28 01:30"));
    item.deadline = Some(moment("2026-03-28 02:30"));
    item.is_recurring = true;
    item.rrule = "FREQ=DAILY;COUNT=2".into();
    let error = document.save_item(item).unwrap_err().to_string();
    assert!(error.contains("Clock change"), "{error}");
    assert!(error.contains("2026-03-29"), "{error}");
    assert!(document.items().unwrap().is_empty());
}

#[test]
fn invalid_calendar_marker_from_moved_occurrence_is_rejected_atomically() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Moved review");
    item.start = Some(moment("2026-03-28 01:00"));
    item.end = Some(moment("2026-03-28 01:30"));
    item.planned = Some(moment("2026-03-28 02:30"));
    item.is_recurring = true;
    item.rrule = "FREQ=WEEKLY;COUNT=2".into();
    let item = document.save_item(item).unwrap();
    let mut moved = event(&item);
    moved.new_start = Some(moment("2026-03-29 01:00"));
    moved.new_end = Some(moment("2026-03-29 01:30"));
    assert!(document.save_override(moved).is_err());
    assert!(document.overrides().unwrap().is_empty());
}

#[test]
fn rating_steps_are_bounded_for_priority_and_severity() {
    use antodo::core::step_rating;
    assert_eq!(step_rating(3, 1), 4);
    assert_eq!(step_rating(3, -1), 2);
    assert_eq!(step_rating(5, 1), 5);
    assert_eq!(step_rating(1, -1), 1);
    assert_eq!(step_rating(255, 127), 5);
    assert_eq!(step_rating(0, -128), 1);
}

#[test]
fn event_status_clears_task_only_properties_and_preserves_schedule_and_relations() {
    let mut doc = Document::new().unwrap();
    let parent = doc.save_item(Item::new("Parent")).unwrap();
    let tag = doc.save_tag(Tag::new("Meetings", "#123456")).unwrap();
    let mut item = recurring();
    item.status = Status::Event;
    item.parents.push(parent.id);
    item.tags.push(tag.id);
    item.deadline = Some(moment("2026-11-11"));
    item.planned = Some(moment("2026-11-10"));
    item.completion = Some(moment("2026-11-10 10:00"));
    item.effort = 10;
    let saved = doc.save_item(item).unwrap();
    assert_eq!(saved.status, Status::Event);
    assert!(saved.deadline.is_none() && saved.planned.is_none() && saved.completion.is_none());
    assert_eq!(saved.effort, 0);
    assert!(saved.start.is_some() && saved.end.is_some() && saved.is_recurring);
    assert_eq!(saved.parents, vec![parent.id]);
    assert_eq!(saved.tags, vec![tag.id]);
    let mut override_value = event(&saved);
    override_value.status = Status::Event;
    doc.save_override(override_value).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("event.antodo");
    doc.save_as(&path).unwrap();
    let reopened = Document::open(&path).unwrap();
    assert_eq!(
        reopened
            .items()
            .unwrap()
            .into_iter()
            .find(|i| i.id == saved.id)
            .unwrap(),
        saved
    );
    assert_eq!(reopened.overrides().unwrap()[0].status, Status::Event);
}

#[test]
fn saved_date_only_offsets_are_rejected_without_mutating_the_document() {
    let mut document = Document::new().unwrap();
    let mut item = Item::new("Date-only task");
    let mut date = moment("2026-11-10");
    date.offset_seconds = Some(3600);
    assert!(date.utc().unwrap_err().to_string().contains("date-only"));
    item.planned = Some(date);
    assert!(document.save_item(item).is_err());
    assert!(document.items().unwrap().is_empty());
    assert!(!document.is_dirty());
}

#[test]
fn sqlite_sidecars_block_saving_and_preserve_disk_and_working_changes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("work.antodo");
    let mut document = Document::new().unwrap();
    document.save_item(Item::new("Saved")).unwrap();
    document.save_as(&path).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let unsaved = document.save_item(Item::new("Unsaved")).unwrap();
    for suffix in ["-wal", "-shm", "-journal"] {
        let mut name = path.as_os_str().to_owned();
        name.push(suffix);
        let sidecar = std::path::PathBuf::from(name);
        std::fs::write(&sidecar, []).unwrap();
        let error = document.save().unwrap_err().to_string();
        assert!(
            error.contains("another SQLite session"),
            "{suffix}: {error}"
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert!(document.items().unwrap().contains(&unsaved));
        assert!(document.is_dirty());
        assert_eq!(document.path().unwrap(), path.canonicalize().unwrap());
        std::fs::remove_file(sidecar).unwrap();
    }
    document.save().unwrap();
    assert!(!document.is_dirty());
    assert!(
        Document::open(&path)
            .unwrap()
            .items()
            .unwrap()
            .contains(&unsaved)
    );
}

#[test]
fn corrupted_sqlite_values_and_relationships_are_rejected_without_rewriting_files() {
    for statement in [
        "UPDATE event_overrides SET id='not-a-uuid'",
        "UPDATE items SET planned_date='invalid-json'",
        "UPDATE items SET status='Done',completion_date=NULL",
        "UPDATE event_overrides SET original_start_date='invalid-json'",
        "INSERT INTO item_tags(item_id,tag_id,position) SELECT id,'missing-tag',0 FROM items",
        "PRAGMA ignore_check_constraints=ON; UPDATE items SET priority=0",
    ] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("corrupted.antodo");
        let mut document = Document::new().unwrap();
        let item = document.save_item(recurring()).unwrap();
        document.save_override(event(&item)).unwrap();
        document.save_as(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch("PRAGMA foreign_keys=OFF").unwrap();
        connection.execute_batch(statement).unwrap();
        drop(connection);
        let bytes = std::fs::read(&path).unwrap();
        assert!(Document::open(&path).is_err(), "accepted {statement}");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(document.items().unwrap(), vec![item]);
        assert!(!document.is_dirty());
    }
}

#[test]
fn occurrence_identity_cannot_be_reused_for_a_different_original_date() {
    let mut document = Document::new().unwrap();
    let item = document.save_item(recurring()).unwrap();
    let original = document.save_override(event(&item)).unwrap();
    let mut reassigned = original.clone();
    reassigned.original_start = moment("2026-11-17 09:00");
    reassigned.original_end = moment("2026-11-17 10:00");
    assert!(
        document
            .save_override(reassigned)
            .unwrap_err()
            .to_string()
            .contains("identity")
    );
    assert_eq!(document.overrides().unwrap(), vec![original]);
}

#[test]
fn first_occurrence_reminder_creates_one_acknowledgement_and_rejects_wrong_task() {
    use antodo::core::query::due_reminders;
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("reminders.antodo");
    let mut document = Document::new().unwrap();
    let mut item = recurring();
    item.reminder = Some(15);
    let item = document.save_item(item).unwrap();
    document.save_as(&path).unwrap();
    let now = moment("2026-11-10 08:45").utc().unwrap();
    let reminder = due_reminders(std::slice::from_ref(&item), &[], now)
        .unwrap()
        .remove(0);
    let mut wrong_task = reminder.clone();
    wrong_task.item_id = Uuid::now_v7();
    assert!(document.mark_reminded(&wrong_task).is_err());
    assert!(!document.is_dirty());
    assert!(document.overrides().unwrap().is_empty());
    document.mark_reminded(&reminder).unwrap();
    document.mark_reminded(&reminder).unwrap();
    let changes = document.overrides().unwrap();
    assert_eq!(changes.len(), 1);
    assert!(changes[0].reminded);
    assert!(changes[0].new_start.is_none() && changes[0].new_end.is_none());
    assert!(
        due_reminders(std::slice::from_ref(&item), &changes, now)
            .unwrap()
            .is_empty()
    );
    document.save().unwrap();
    assert_eq!(Document::open(&path).unwrap().overrides().unwrap(), changes);
    assert_eq!(
        due_reminders(&[item], &changes, now + Duration::weeks(1))
            .unwrap()
            .len(),
        1
    );
}
