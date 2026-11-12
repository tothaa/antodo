use antodo::core::{Document, Item, Note, Status, Tag};
use rusqlite::Connection;
use uuid::Uuid;

#[test]
fn markdown_notes_round_trip_with_unsaved_change_protection() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("notes.antodo");
    let mut document = Document::new().unwrap();
    let task = document.save_item(Item::new("Existing task")).unwrap();
    document.save_as(&path).unwrap();
    let mut note = Note::new("  Design notes  ");
    assert_eq!(note.id.get_version_num(), 7);
    note.content = "# Heading\n\n- **Bold** and `code`\n\n[Link](https://example.com)\n".into();
    let note = document.save_note(note).unwrap();
    assert_eq!(note.name, "Design notes");
    assert!(document.is_dirty());
    assert!(Document::open(&path).unwrap().notes().unwrap().is_empty());
    assert!(
        document
            .save_as(&directory.path().join("missing/fail.antodo"))
            .is_err()
    );
    assert_eq!(document.note(note.id).unwrap(), note);
    document.save().unwrap();
    let mut reopened = Document::open(&path).unwrap();
    assert!(!reopened.is_dirty());
    assert_eq!(reopened.notes().unwrap(), std::slice::from_ref(&note));
    assert_eq!(reopened.items().unwrap(), std::slice::from_ref(&task));

    let mut changed = note.clone();
    changed.name = "Revised".into();
    changed.content = "".into();
    let changed = reopened.save_note(changed).unwrap();
    assert_eq!(changed.id, note.id);
    assert_eq!(reopened.notes().unwrap(), std::slice::from_ref(&changed));
    assert_eq!(Document::open(&path).unwrap().note(note.id).unwrap(), note);
    reopened.save().unwrap();
    assert_eq!(
        Document::open(&path).unwrap().note(note.id).unwrap(),
        changed
    );
    reopened.delete_note(note.id).unwrap();
    assert!(reopened.note(note.id).is_err());
    assert!(reopened.notes().unwrap().is_empty());
    assert_eq!(reopened.items().unwrap(), [task]);
    reopened.save().unwrap();
    assert!(Document::open(&path).unwrap().notes().unwrap().is_empty());
}

#[test]
fn note_search_matches_names_markdown_unicode_and_literal_sql_metacharacters() {
    let mut document = Document::new().unwrap();
    let mut first = Note::new("ÁRVÍZTŰRŐ notes");
    first.content = "## Project Phoenix\n100%_complete\n'; DROP TABLE notes; --".into();
    let first = document.save_note(first).unwrap();
    let second = document.save_note(Note::new("Another project")).unwrap();
    for query in ["árvíztűrő", "PHOENIX", "%_", "'; DROP TABLE notes; --"] {
        assert_eq!(
            document.search_notes(query).unwrap(),
            std::slice::from_ref(&first)
        );
    }
    assert_eq!(document.search_notes("").unwrap().len(), 2);
    assert_eq!(document.search_notes("another").unwrap(), [second]);
    assert!(document.search_notes("missing").unwrap().is_empty());
    assert_eq!(document.notes().unwrap().len(), 2);
}

#[test]
fn invalid_notes_and_missing_deletions_preserve_document_and_dirty_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::new().unwrap();
    let saved = document.save_note(Note::new("Keep me")).unwrap();
    document
        .save_as(&directory.path().join("notes.antodo"))
        .unwrap();
    for invalid in [
        Note {
            name: " \t\n ".into(),
            ..saved.clone()
        },
        Note {
            id: Uuid::nil(),
            ..saved.clone()
        },
    ] {
        assert!(document.save_note(invalid).is_err());
        assert_eq!(document.notes().unwrap(), std::slice::from_ref(&saved));
        assert!(!document.is_dirty());
    }
    assert!(document.delete_note(Uuid::now_v7()).is_err());
    assert!(!document.is_dirty());
    assert_eq!(document.notes().unwrap(), [saved]);
}

#[test]
fn old_documents_migrate_in_memory_and_preserve_tasks() {
    for version in [1, 2, 3] {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("old.antodo");
        let mut document = Document::new().unwrap();
        let task = document.save_item(Item::new("Old task")).unwrap();
        let old_note =
            (version == 3).then(|| document.save_note(Note::new("Existing note")).unwrap());
        document.save_as(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("DROP TABLE note_tags; DROP TABLE note_tasks;")
            .unwrap();
        if version < 3 {
            connection.execute_batch("DROP TABLE notes").unwrap();
        }
        connection
            .pragma_update(None, "user_version", version)
            .unwrap();
        drop(connection);
        let original = std::fs::read(&path).unwrap();
        let mut migrated = Document::open(&path).unwrap();
        assert!(migrated.is_dirty());
        assert_eq!(
            migrated.notes().unwrap(),
            old_note.iter().cloned().collect::<Vec<_>>()
        );
        assert_eq!(migrated.items().unwrap(), std::slice::from_ref(&task));
        let mut note = Note::new("New note");
        note.tasks = vec![task.id];
        let note = migrated.save_note(note).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), original);
        migrated.save().unwrap();
        let reopened = Document::open(&path).unwrap();
        assert!(!reopened.is_dirty());
        assert_eq!(reopened.note(note.id).unwrap(), note);
        if let Some(old_note) = old_note {
            assert_eq!(reopened.note(old_note.id).unwrap(), old_note);
        }
        let connection = Connection::open(&path).unwrap();
        assert_eq!(
            connection
                .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
                .unwrap(),
            4
        );
    }
}

#[test]
fn documents_with_invalid_note_ids_are_rejected_without_changing_the_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid.antodo");
    let mut document = Document::new().unwrap();
    document.save_note(Note::new("Note")).unwrap();
    document.save_as(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection
        .execute("UPDATE notes SET id=?1", [Uuid::nil().to_string()])
        .unwrap();
    drop(connection);
    let before = std::fs::read(&path).unwrap();
    assert!(Document::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn notes_store_ordered_tags_and_many_to_many_task_links() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("relations.antodo");
    let mut document = Document::new().unwrap();
    let first = document.save_item(Item::new("First")).unwrap();
    let second = document.save_item(Item::new("Second")).unwrap();
    let red = document.save_tag(Tag::new("Red", "#FF0000")).unwrap();
    let blue = document.save_tag(Tag::new("Blue", "#0000FF")).unwrap();
    let mut a = Note::new("A");
    a.content = "# Shared reference".into();
    a.tags = vec![blue.id, red.id, blue.id];
    a.tasks = vec![second.id, first.id, second.id];
    let a = document.save_note(a).unwrap();
    let mut b = Note::new("B");
    b.tags = vec![red.id];
    b.tasks = vec![first.id];
    let b = document.save_note(b).unwrap();
    assert_eq!(a.tags, [blue.id, red.id]);
    assert_eq!(a.tasks, [second.id, first.id]);
    assert_eq!(document.notes_for_task(first.id).unwrap(), [a.clone(), b]);
    assert_eq!(
        document.notes_for_task(second.id).unwrap(),
        std::slice::from_ref(&a)
    );
    assert!(document.notes_for_task(Uuid::now_v7()).is_err());
    assert_eq!(
        document
            .search_notes_filtered("shared", Some(red.id), Some(second.id))
            .unwrap(),
        std::slice::from_ref(&a)
    );
    assert!(
        document
            .search_notes_filtered("b", Some(blue.id), None)
            .unwrap()
            .is_empty()
    );
    document.save_as(&path).unwrap();
    assert_eq!(Document::open(&path).unwrap().note(a.id).unwrap(), a);

    let mut revised = a;
    revised.tags = vec![red.id];
    revised.tasks = vec![first.id];
    let revised = document.save_note(revised).unwrap();
    assert!(document.notes_for_task(second.id).unwrap().is_empty());
    assert_eq!(document.note(revised.id).unwrap(), revised);
}

#[test]
fn missing_note_relationships_do_not_change_content_or_dirty_state() {
    let directory = tempfile::tempdir().unwrap();
    let mut document = Document::new().unwrap();
    let task = document.save_item(Item::new("Task")).unwrap();
    let tag = document.save_tag(Tag::new("Tag", "#123456")).unwrap();
    let mut note = Note::new("Keep");
    note.tags = vec![tag.id];
    note.tasks = vec![task.id];
    let saved = document.save_note(note).unwrap();
    document
        .save_as(&directory.path().join("keep.antodo"))
        .unwrap();
    for use_tag in [false, true] {
        let mut changed = saved.clone();
        changed.name = "Must roll back".into();
        changed.content = "Different Markdown".into();
        if use_tag {
            changed.tags.push(Uuid::now_v7());
        } else {
            changed.tasks.push(Uuid::now_v7());
        }
        assert!(document.save_note(changed).is_err());
        assert_eq!(document.note(saved.id).unwrap(), saved);
        assert!(!document.is_dirty());
    }
}

#[test]
fn deleting_entities_only_cascades_their_note_links() {
    let mut document = Document::new().unwrap();
    let first = document.save_item(Item::new("First")).unwrap();
    let second = document.save_item(Item::new("Second")).unwrap();
    let red = document.save_tag(Tag::new("Red", "#FF0000")).unwrap();
    let blue = document.save_tag(Tag::new("Blue", "#0000FF")).unwrap();
    let mut note = Note::new("Shared");
    note.tags = vec![red.id, blue.id];
    note.tasks = vec![first.id, second.id];
    let note = document.save_note(note).unwrap();
    document.delete_item(first.id).unwrap();
    document.delete_tag(red.id).unwrap();
    let retained = document.note(note.id).unwrap();
    assert_eq!(retained.tasks, [second.id]);
    assert_eq!(retained.tags, [blue.id]);
    document.delete_note(note.id).unwrap();
    assert_eq!(document.items().unwrap(), std::slice::from_ref(&second));
    assert_eq!(document.tags().unwrap(), [blue]);
    assert!(document.notes_for_task(second.id).unwrap().is_empty());
}

#[test]
fn task_side_note_selection_is_atomic_preserves_shared_links_and_normal_task_saves() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("task-notes.antodo");
    let mut document = Document::new().unwrap();
    let first = document.save_item(Item::new("First")).unwrap();
    let second = document.save_item(Item::new("Second")).unwrap();
    let mut shared = Note::new("Shared");
    shared.tasks = vec![second.id, first.id];
    let shared = document.save_note(shared).unwrap();
    let extra = document.save_note(Note::new("Extra")).unwrap();
    let mut changed = first.clone();
    changed.name = "Changed".into();
    document
        .save_item_with_notes(changed.clone(), vec![extra.id, shared.id, extra.id])
        .unwrap();
    assert_eq!(
        document.note(shared.id).unwrap().tasks,
        [second.id, first.id]
    );
    assert_eq!(document.note(extra.id).unwrap().tasks, [first.id]);
    changed.priority = 4;
    document.save_item(changed.clone()).unwrap();
    assert_eq!(document.notes_for_task(first.id).unwrap().len(), 2);
    document.save_as(&path).unwrap();
    let mut invalid = changed.clone();
    invalid.name = "No commit".into();
    assert!(
        document
            .save_item_with_notes(invalid, vec![Uuid::now_v7()])
            .is_err()
    );
    assert_eq!(
        document
            .items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == first.id)
            .unwrap(),
        changed
    );
    assert_eq!(document.notes_for_task(first.id).unwrap().len(), 2);
    assert!(!document.is_dirty());
    document.save_item_with_notes(changed, vec![]).unwrap();
    assert_eq!(document.note(shared.id).unwrap().tasks, [second.id]);
    assert!(document.note(extra.id).unwrap().tasks.is_empty());
}

#[test]
fn failed_task_validation_does_not_assign_notes_and_new_tasks_can_assign_them() {
    let mut document = Document::new().unwrap();
    let note = document.save_note(Note::new("Reference")).unwrap();
    let parent = document.save_item(Item::new("Parent")).unwrap();
    let mut child = Item::new("Child");
    child.parents = vec![parent.id];
    document.save_item(child).unwrap();
    let mut changed = parent.clone();
    changed.status = Status::Done;
    assert!(
        document
            .save_item_with_notes(changed, vec![note.id])
            .is_err()
    );
    assert!(document.note(note.id).unwrap().tasks.is_empty());
    let created = document
        .save_item_with_notes(Item::new("New"), vec![note.id])
        .unwrap();
    assert_eq!(document.note(note.id).unwrap().tasks, [created.id]);
    assert_eq!(
        document
            .items()
            .unwrap()
            .into_iter()
            .find(|item| item.id == parent.id)
            .unwrap(),
        parent
    );
}

#[test]
fn sql_failure_rolls_back_note_fields_tags_and_both_sides_of_task_links() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("rollback.antodo");
    let mut document = Document::new().unwrap();
    let task = document.save_item(Item::new("Task")).unwrap();
    let tag = document.save_tag(Tag::new("Tag", "#123456")).unwrap();
    let mut first = Note::new("First");
    first.tasks = vec![task.id];
    let first = document.save_note(first).unwrap();
    let second = document.save_note(Note::new("Second")).unwrap();
    document.save_as(&path).unwrap();
    let connection = Connection::open(&path).unwrap();
    connection.execute_batch("CREATE TRIGGER fail_note_link BEFORE INSERT ON note_tasks BEGIN SELECT RAISE(ABORT, 'injected link failure'); END;").unwrap();
    drop(connection);
    let mut document = Document::open(&path).unwrap();
    let before = document.notes().unwrap();
    let mut changed_note = first;
    changed_note.content = "Must roll back".into();
    changed_note.tags = vec![tag.id];
    assert!(document.save_note(changed_note).is_err());
    assert_eq!(document.notes().unwrap(), before);
    assert!(!document.is_dirty());
    let mut changed_task = task.clone();
    changed_task.name = "Must also roll back".into();
    assert!(
        document
            .save_item_with_notes(changed_task, vec![second.id])
            .is_err()
    );
    assert_eq!(document.items().unwrap(), [task]);
    assert_eq!(document.notes().unwrap(), before);
    assert!(!document.is_dirty());
}

#[test]
fn legacy_serialized_notes_default_to_empty_relations() {
    let id = Uuid::now_v7();
    let note: Note =
        serde_json::from_value(serde_json::json!({"id":id,"name":"Legacy","content":"# Markdown"}))
            .unwrap();
    assert!(note.tags.is_empty());
    assert!(note.tasks.is_empty());
}

#[test]
fn corrupt_note_links_are_rejected_without_rewriting_the_document() {
    let directory = tempfile::tempdir().unwrap();
    for table in ["note_tags", "note_tasks"] {
        let path = directory.path().join(format!("{table}.antodo"));
        let mut document = Document::new().unwrap();
        let note = document.save_note(Note::new("Note")).unwrap();
        document.save_as(&path).unwrap();
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch("PRAGMA foreign_keys=OFF;")
            .unwrap();
        let statement = if table == "note_tags" {
            "INSERT INTO note_tags(note_id,tag_id,position) VALUES (?1,?2,0)"
        } else {
            "INSERT INTO note_tasks(note_id,task_id,position) VALUES (?1,?2,0)"
        };
        connection
            .execute(statement, [note.id.to_string(), Uuid::now_v7().to_string()])
            .unwrap();
        drop(connection);
        let before = std::fs::read(&path).unwrap();
        assert!(Document::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
}

#[test]
fn context_note_linking_preserves_links_and_rejects_a_partial_batch() {
    let mut document = Document::new().unwrap();
    let first = document.save_item(Item::new("First")).unwrap();
    let second = document.save_item(Item::new("Second")).unwrap();
    let third = document.save_item(Item::new("Third")).unwrap();
    let mut note = Note::new("Shared");
    note.tasks = vec![first.id];
    let note = document.save_note(note).unwrap();
    let directory = tempfile::tempdir().unwrap();
    document
        .save_as(&directory.path().join("links.antodo"))
        .unwrap();
    assert!(
        document
            .link_note_to_tasks(note.id, &[second.id, Uuid::now_v7()])
            .is_err()
    );
    assert_eq!(document.note(note.id).unwrap(), note);
    assert!(!document.is_dirty());
    assert!(document.link_note_to_tasks(note.id, &[]).is_err());
    document
        .link_note_to_tasks(note.id, &[first.id, second.id, third.id, second.id])
        .unwrap();
    assert_eq!(
        document.note(note.id).unwrap().tasks,
        [first.id, second.id, third.id]
    );
    document.link_note_to_tasks(note.id, &[second.id]).unwrap();
    assert_eq!(
        document.note(note.id).unwrap().tasks,
        [first.id, second.id, third.id]
    );
}
