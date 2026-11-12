use antodo::core::actions::{TaskAction, TaskOccurrence};
use antodo::core::query::hierarchy;
use antodo::core::{Document, EventOverride, Item, Moment, Status, Tag, local_timezone};
use chrono::{Duration, Utc};
use uuid::Uuid;

fn save(document: &mut Document, name: &str, parents: &[Uuid]) -> Item {
    let mut item = Item::new(name);
    item.parents = parents.to_vec();
    document.save_item(item).unwrap()
}

fn stored(document: &Document, id: Uuid) -> Item {
    document
        .items()
        .unwrap()
        .into_iter()
        .find(|item| item.id == id)
        .unwrap()
}

fn occurrence(item: &Item, parent: Option<Uuid>) -> TaskOccurrence {
    TaskOccurrence {
        item_id: item.id,
        parent_id: parent,
    }
}

#[test]
fn bulk_completion_and_reopening_validate_final_graph_in_any_order() {
    let mut document = Document::new().unwrap();
    let parent = save(&mut document, "Parent", &[]);
    let child = save(&mut document, "Child", &[parent.id]);
    let before = Utc::now() - Duration::seconds(1);
    document
        .apply_task_action(&[parent.id, child.id], TaskAction::Status(Status::Done))
        .unwrap();
    for id in [parent.id, child.id] {
        let item = stored(&document, id);
        assert_eq!(item.status, Status::Done);
        let completion = item.completion.unwrap();
        assert!(!completion.is_date());
        assert!(completion.utc().unwrap() >= before);
        assert!(completion.utc().unwrap() <= Utc::now());
    }
    let completed = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&[child.id], TaskAction::Status(Status::Pending))
            .is_err()
    );
    assert_eq!(document.items().unwrap(), completed);
    document
        .apply_task_action(&[child.id, parent.id], TaskAction::Status(Status::Pending))
        .unwrap();
    assert!(
        document
            .items()
            .unwrap()
            .iter()
            .all(|item| item.status == Status::Pending && item.completion.is_none())
    );
}

#[test]
fn bulk_failure_preserves_dirty_state_and_every_item() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::new().unwrap();
    let free = save(&mut document, "Free", &[]);
    let parent = save(&mut document, "Parent", &[]);
    save(&mut document, "Open child", &[parent.id]);
    document
        .save_as(&directory.path().join("batch.antodo"))
        .unwrap();
    let before = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&[free.id, parent.id], TaskAction::Status(Status::Done))
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    assert!(!document.is_dirty());
    assert!(
        document
            .apply_task_action(&[free.id, Uuid::now_v7()], TaskAction::Delete)
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    assert!(!document.is_dirty());
}

#[test]
fn ratings_tags_planning_and_delete_share_atomic_commands() {
    let mut document = Document::new().unwrap();
    let first = save(&mut document, "First", &[]);
    let second = save(&mut document, "Second", &[first.id]);
    let tag = document.save_tag(Tag::new("Work", "#123456")).unwrap();
    let ids = [first.id, second.id, first.id];
    document
        .apply_task_action(&ids, TaskAction::ToggleTag(tag.id))
        .unwrap();
    assert!(
        document
            .items()
            .unwrap()
            .iter()
            .all(|item| item.tags == [tag.id])
    );
    document
        .apply_task_action(&[first.id], TaskAction::ToggleTag(tag.id))
        .unwrap();
    document
        .apply_task_action(&ids, TaskAction::ToggleTag(tag.id))
        .unwrap();
    assert_eq!(stored(&document, first.id).tags, [tag.id]);
    assert!(stored(&document, second.id).tags.is_empty());
    document
        .apply_task_action(&ids, TaskAction::Priority(1))
        .unwrap();
    document
        .apply_task_action(&ids, TaskAction::Severity(5))
        .unwrap();
    let before = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&ids, TaskAction::Priority(0))
            .is_err()
    );
    assert!(
        document
            .apply_task_action(&ids, TaskAction::Severity(6))
            .is_err()
    );
    assert!(
        document
            .apply_task_action(&ids, TaskAction::ToggleTag(Uuid::now_v7()))
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    document
        .apply_task_action(&ids, TaskAction::PlanToday)
        .unwrap();
    let today = Moment::from_utc(Utc::now(), &local_timezone(), true).unwrap();
    assert!(
        document
            .items()
            .unwrap()
            .iter()
            .all(|item| item.planned == Some(today.clone())
                && item.priority == 1
                && item.severity == 5)
    );
    document
        .apply_task_action(&ids, TaskAction::Delete)
        .unwrap();
    assert!(document.items().unwrap().is_empty());
}

#[test]
fn move_removes_only_selected_parent_and_additive_drop_preserves_links() {
    let mut document = Document::new().unwrap();
    let a = save(&mut document, "A", &[]);
    let b = save(&mut document, "B", &[]);
    let c = save(&mut document, "C", &[]);
    let child = save(&mut document, "Child", &[a.id, b.id]);
    document
        .relink_items(&[occurrence(&child, Some(a.id))], Some(c.id), false)
        .unwrap();
    assert_eq!(stored(&document, child.id).parents, [b.id, c.id]);
    document
        .relink_items(&[occurrence(&child, Some(c.id))], Some(a.id), true)
        .unwrap();
    assert_eq!(stored(&document, child.id).parents, [b.id, c.id, a.id]);
    document
        .relink_items(&[occurrence(&child, Some(c.id))], None, false)
        .unwrap();
    assert_eq!(stored(&document, child.id).parents, [b.id, a.id]);
    document
        .relink_items(&[occurrence(&child, Some(a.id))], None, true)
        .unwrap();
    assert_eq!(stored(&document, child.id).parents, [b.id, a.id]);
    document
        .relink_items(&[occurrence(&a, None)], Some(c.id), false)
        .unwrap();
    assert_eq!(stored(&document, a.id).parents, [c.id]);
}

#[test]
fn any_failed_relink_rolls_back_all_selected_occurrences() {
    let mut document = Document::new().unwrap();
    let old = save(&mut document, "Old", &[]);
    let first = save(&mut document, "First", &[old.id]);
    let second = save(&mut document, "Second", &[old.id]);
    let descendant = save(&mut document, "Descendant", &[second.id]);
    let selection = [
        occurrence(&first, Some(old.id)),
        occurrence(&second, Some(old.id)),
    ];
    let before = document.items().unwrap();
    for additive in [false, true] {
        assert!(
            document
                .relink_items(&selection, Some(descendant.id), additive)
                .is_err()
        );
        assert_eq!(document.items().unwrap(), before);
    }
    assert!(
        document
            .relink_items(&selection, Some(Uuid::now_v7()), false)
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    assert!(
        document
            .relink_items(
                &[selection[0], occurrence(&second, Some(first.id))],
                None,
                false
            )
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    document.relink_items(&selection, None, false).unwrap();
    assert!(stored(&document, first.id).parents.is_empty());
    assert!(stored(&document, second.id).parents.is_empty());
}

#[test]
fn linking_to_completed_parent_is_rejected_for_entire_batch() {
    let mut document = Document::new().unwrap();
    let parent = save(&mut document, "Parent", &[]);
    document
        .apply_task_action(&[parent.id], TaskAction::Status(Status::Done))
        .unwrap();
    let closed = save(&mut document, "Closed", &[]);
    document
        .apply_task_action(&[closed.id], TaskAction::Status(Status::WontDo))
        .unwrap();
    let open = save(&mut document, "Open", &[]);
    let before = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&[closed.id, open.id], TaskAction::LinkParent(parent.id))
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
    document
        .apply_task_action(&[closed.id], TaskAction::LinkParent(parent.id))
        .unwrap();
    assert_eq!(stored(&document, closed.id).parents, [parent.id]);
}

#[test]
fn batch_creates_related_tasks_and_keeps_explicit_completion() {
    let mut document = Document::new().unwrap();
    let mut parent = Item::new("Parent");
    parent.status = Status::Done;
    parent.completion = Some(Moment::parse("2026-11-01 10:00", "Europe/Budapest").unwrap());
    let mut child = Item::new("Child");
    child.parents.push(parent.id);
    child.status = Status::WontDo;
    document
        .save_items(vec![child.clone(), parent.clone()])
        .unwrap();
    assert_eq!(stored(&document, parent.id).completion, parent.completion);
    assert_eq!(stored(&document, child.id).parents, [parent.id]);
    assert!(document.save_items(vec![parent.clone(), parent]).is_err());
}

#[test]
fn hierarchy_rows_retain_the_exact_parent_of_shared_tasks() {
    let a = Item::new("A");
    let b = Item::new("B");
    let mut child = Item::new("Shared");
    child.parents = vec![a.id, b.id];
    let rows = hierarchy(&[a.clone(), b.clone(), child.clone()]);
    let links: Vec<_> = rows
        .iter()
        .filter(|row| row.item_id == child.id)
        .map(|row| row.parent_id)
        .collect();
    assert_eq!(links, [Some(a.id), Some(b.id)]);
    assert!(
        rows.iter()
            .filter(|row| row.depth == 0)
            .all(|row| row.parent_id.is_none())
    );
}

#[test]
fn occurrence_parent_invariants_survive_bulk_changes() {
    let mut document = Document::new().unwrap();
    let mut parent = Item::new("Recurring parent");
    parent.is_recurring = true;
    parent.start = Some(Moment::parse("2026-11-11 10:00", "UTC").unwrap());
    parent.end = Some(Moment::parse("2026-11-11 11:00", "UTC").unwrap());
    parent.rrule = "FREQ=DAILY;COUNT=2".into();
    let parent = document.save_item(parent).unwrap();
    document
        .save_override(EventOverride {
            id: Uuid::now_v7(),
            item_id: parent.id,
            original_start: parent.start.clone().unwrap(),
            original_end: parent.end.clone().unwrap(),
            new_start: None,
            new_end: None,
            cancelled: false,
            status: Status::Done,
            reminded: false,
        })
        .unwrap();
    let child = save(&mut document, "Child", &[]);
    let before = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&[child.id], TaskAction::LinkParent(parent.id))
            .is_err()
    );
    assert_eq!(document.items().unwrap(), before);
}

#[test]
fn bulk_event_planning_and_empty_selections_are_rejected() {
    let mut document = Document::new().unwrap();
    let first = save(&mut document, "Task", &[]);
    let mut event = Item::new("Event");
    event.status = Status::Event;
    let event = document.save_item(event).unwrap();
    let before = document.items().unwrap();
    assert!(
        document
            .apply_task_action(&[first.id, event.id], TaskAction::PlanToday)
            .is_err()
    );
    assert!(document.apply_task_action(&[], TaskAction::Delete).is_err());
    assert!(document.relink_items(&[], None, false).is_err());
    assert_eq!(document.items().unwrap(), before);
}

#[test]
fn sql_failure_during_a_batch_rolls_back_earlier_writes_and_links() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rollback.antodo");
    let mut original = Document::new().unwrap();
    let old = save(&mut original, "Old", &[]);
    let target = save(&mut original, "Target", &[]);
    let first = save(&mut original, "First", &[old.id]);
    let second = save(&mut original, "Second", &[old.id]);
    original.save_as(&path).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER simulate_storage_error BEFORE INSERT ON item_parents
         WHEN (SELECT name FROM items WHERE id=NEW.item_id) = 'Second'
         BEGIN SELECT RAISE(ABORT, 'simulated storage error'); END;",
        )
        .unwrap();
    drop(connection);
    let mut document = Document::open(&path).unwrap();
    let before = document.items().unwrap();
    let result = document.relink_items(
        &[
            occurrence(&first, Some(old.id)),
            occurrence(&second, Some(old.id)),
        ],
        Some(target.id),
        false,
    );
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("simulated storage error")
    );
    assert_eq!(document.items().unwrap(), before);
    assert!(!document.is_dirty());
}

#[test]
fn plan_today_preserves_each_tasks_planned_timezone() {
    let mut document = Document::new().unwrap();
    let mut east = Item::new("East");
    east.planned = Some(Moment::parse("2020-01-01 09:00", "Pacific/Kiritimati").unwrap());
    let mut west = Item::new("West");
    west.planned = Some(Moment::parse("2020-01-01", "Pacific/Honolulu").unwrap());
    let items = document.save_items(vec![east, west]).unwrap();
    document
        .apply_task_action(
            &items.iter().map(|item| item.id).collect::<Vec<_>>(),
            TaskAction::PlanToday,
        )
        .unwrap();
    for item in items {
        let timezone = item.planned.unwrap().timezone;
        assert_eq!(
            stored(&document, item.id).planned,
            Some(Moment::from_utc(Utc::now(), &timezone, true).unwrap())
        );
    }
}
