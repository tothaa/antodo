//! Markdown notes and their ordered tag/task links in the document working copy.
use rusqlite::{Connection, Row, params};
use uuid::Uuid;

use super::sql::{execute, query, query_optional, replace_ordered};
use super::store::{row_uuid, unique, validate_uuid};
use super::{Document, Note, Result, invalid};

pub(super) const SCHEMA: &str = "CREATE TABLE notes (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
    content TEXT NOT NULL DEFAULT ''
);";

pub(super) const RELATION_SCHEMA: &str = "CREATE TABLE note_tags (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY(note_id, tag_id)
);
CREATE INDEX notes_for_tag ON note_tags(tag_id);
CREATE TABLE note_tasks (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    task_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY(note_id, task_id)
);
CREATE INDEX notes_for_task ON note_tasks(task_id);";

impl Document {
    pub fn notes(&self) -> Result<Vec<Note>> {
        let mut notes = query(
            &self.connection,
            "SELECT id,name,content FROM notes ORDER BY name COLLATE NOCASE,id",
            [],
            read_note,
        )?;
        for note in &mut notes {
            read_relations(&self.connection, note)?;
        }
        Ok(notes)
    }

    /// Case-insensitive literal search, including non-ASCII text and Markdown.
    /// SQL wildcard characters in the query retain their literal meaning.
    pub fn search_notes(&self, search: &str) -> Result<Vec<Note>> {
        self.search_notes_filtered(search, None, None)
    }

    pub fn search_notes_filtered(
        &self,
        search: &str,
        tag: Option<Uuid>,
        task: Option<Uuid>,
    ) -> Result<Vec<Note>> {
        let search = search.to_lowercase();
        let mut notes = self.notes()?;
        notes.retain(|note| {
            (note.name.to_lowercase().contains(&search)
                || note.content.to_lowercase().contains(&search))
                && tag.is_none_or(|id| note.tags.contains(&id))
                && task.is_none_or(|id| note.tasks.contains(&id))
        });
        Ok(notes)
    }

    pub fn notes_for_task(&self, task: Uuid) -> Result<Vec<Note>> {
        validate_links(
            &self.connection,
            &[task],
            "SELECT 1 FROM items WHERE id=?1",
            "Task not found",
        )?;
        self.search_notes_filtered("", None, Some(task))
    }

    pub fn note(&self, id: Uuid) -> Result<Note> {
        let mut note = query_optional(
            &self.connection,
            "SELECT id,name,content FROM notes WHERE id=?1",
            [id.to_string()],
            read_note,
        )?
        .ok_or_else(|| invalid("Note not found"))?;
        read_relations(&self.connection, &mut note)?;
        Ok(note)
    }

    pub fn save_note(&mut self, mut note: Note) -> Result<Note> {
        note.name = note.name.trim().to_owned();
        unique(&mut note.tags);
        unique(&mut note.tasks);
        validate_note(&note)?;
        validate_note_references(&self.connection, &note)?;
        let transaction = self.connection.transaction()?;
        execute(
            &transaction,
            "INSERT INTO notes(id,name,content) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,content=excluded.content",
            params![note.id.to_string(), note.name, note.content],
        )?;
        for (delete, insert, values) in [
            (
                "DELETE FROM note_tags WHERE note_id=?1",
                "INSERT INTO note_tags(note_id,tag_id,position) VALUES (?1,?2,?3)",
                &note.tags,
            ),
            (
                "DELETE FROM note_tasks WHERE note_id=?1",
                "INSERT INTO note_tasks(note_id,task_id,position) VALUES (?1,?2,?3)",
                &note.tasks,
            ),
        ] {
            replace_ordered(
                &transaction,
                delete,
                insert,
                &note.id.to_string(),
                values.iter().map(Uuid::to_string),
            )?;
        }
        transaction.commit()?;
        self.dirty = true;
        Ok(note)
    }

    /// Add a shared Note to all selected tasks atomically, preserving other links.
    pub fn link_note_to_tasks(&mut self, note_id: Uuid, task_ids: &[Uuid]) -> Result<()> {
        if task_ids.is_empty() {
            return Err(invalid("Select at least one task"));
        }
        let mut note = self.note(note_id)?;
        note.tasks.extend_from_slice(task_ids);
        self.save_note(note)?;
        Ok(())
    }

    pub fn delete_note(&mut self, id: Uuid) -> Result<()> {
        let transaction = self.connection.transaction()?;
        if execute(
            &transaction,
            "DELETE FROM notes WHERE id=?1",
            [id.to_string()],
        )? == 0
        {
            return Err(invalid("Note not found"));
        }
        transaction.commit()?;
        self.dirty = true;
        Ok(())
    }
}

pub(super) fn validate_note(note: &Note) -> Result<()> {
    validate_uuid(note.id)?;
    if note.name.trim().is_empty() {
        return Err(invalid("Give the note a name"));
    }
    Ok(())
}

pub(super) fn validate_note_references(connection: &Connection, note: &Note) -> Result<()> {
    validate_links(
        connection,
        &note.tags,
        "SELECT 1 FROM tags WHERE id=?1",
        "Choose tags from the defined tag list",
    )?;
    validate_links(
        connection,
        &note.tasks,
        "SELECT 1 FROM items WHERE id=?1",
        "A linked task no longer exists",
    )
}

pub(super) fn validate_task_note_ids(connection: &Connection, ids: &[Uuid]) -> Result<()> {
    validate_links(
        connection,
        ids,
        "SELECT 1 FROM notes WHERE id=?1",
        "A linked note no longer exists",
    )
}

fn validate_links(
    connection: &Connection,
    ids: &[Uuid],
    statement: &str,
    missing: &str,
) -> Result<()> {
    for id in ids {
        validate_uuid(*id)?;
        if query_optional(connection, statement, [id.to_string()], |row| {
            row.get::<_, i64>(0)
        })?
        .is_none()
        {
            return Err(invalid(missing));
        }
    }
    Ok(())
}

/// Replace a task's note selection without disturbing the ordering of tasks
/// linked from each retained note. Called inside the task-save transaction.
pub(super) fn replace_task_notes(
    connection: &Connection,
    task: Uuid,
    notes: &[Uuid],
) -> Result<()> {
    let existing = query(
        connection,
        "SELECT note_id FROM note_tasks WHERE task_id=?1",
        [task.to_string()],
        |row| row_uuid(row, 0),
    )?;
    for note in existing.iter().filter(|id| !notes.contains(id)) {
        execute(
            connection,
            "DELETE FROM note_tasks WHERE task_id=?1 AND note_id=?2",
            params![task.to_string(), note.to_string()],
        )?;
    }
    for note in notes.iter().filter(|id| !existing.contains(id)) {
        execute(
            connection,
            "INSERT INTO note_tasks(note_id,task_id,position) SELECT ?1,?2,COALESCE(MAX(position)+1,0) FROM note_tasks WHERE note_id=?1",
            params![note.to_string(), task.to_string()],
        )?;
    }
    Ok(())
}

fn read_note(row: &Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row_uuid(row, 0)?,
        name: row.get(1)?,
        content: row.get(2)?,
        tags: vec![],
        tasks: vec![],
    })
}

fn read_relations(connection: &Connection, note: &mut Note) -> Result<()> {
    note.tags = query(
        connection,
        "SELECT tag_id FROM note_tags WHERE note_id=?1 ORDER BY position,tag_id",
        [note.id.to_string()],
        |row| row_uuid(row, 0),
    )?;
    note.tasks = query(
        connection,
        "SELECT task_id FROM note_tasks WHERE note_id=?1 ORDER BY position,task_id",
        [note.id.to_string()],
        |row| row_uuid(row, 0),
    )?;
    Ok(())
}
