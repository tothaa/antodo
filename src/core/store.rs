//! Transactional domain commands over a SQLite working copy.
//!
//! Editing never touches the document on disk until Save. A failed open or save
//! therefore cannot replace the caller's current document or its unsaved work.
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use rusqlite::{Connection, OpenFlags, Row, params};
use serde::de::DeserializeOwned;
use uuid::Uuid;

use super::recurrence::{validate_override, validate_rule};
use super::sql::{execute, query, query_optional, replace_ordered};
use super::{EventOverride, Item, Moment, Reminder, Result, Status, Tag, invalid, local_timezone};

const APPLICATION_ID: i64 = 0x414E544F;
const SCHEMA_VERSION: i64 = 4;
const SCHEMA: &str = r#"
PRAGMA application_id = 1095652431;
PRAGMA user_version = 4;
CREATE TABLE items (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL CHECK(length(trim(name)) > 0),
    description TEXT NOT NULL DEFAULT '',
    start_date TEXT, end_date TEXT, deadline TEXT,
    status TEXT NOT NULL CHECK(status IN ('Pending','WIP','Done','On-Hold','Won''t Do','Someday','Event')),
    is_recurring INTEGER NOT NULL CHECK(is_recurring IN (0,1)),
    rrule TEXT NOT NULL DEFAULT '',
    reminder INTEGER CHECK(reminder >= 0),
    reminded INTEGER NOT NULL CHECK(reminded IN (0,1)),
    priority INTEGER NOT NULL DEFAULT 3 CHECK(priority BETWEEN 1 AND 5),
    severity INTEGER NOT NULL DEFAULT 3 CHECK(severity BETWEEN 1 AND 5),
    effort INTEGER NOT NULL DEFAULT 0 CHECK(effort >= 0),
    completion_date TEXT, planned_date TEXT,
    CHECK((start_date IS NULL) = (end_date IS NULL))
);
CREATE TABLE tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK(length(trim(name)) > 0),
    color TEXT NOT NULL
);
CREATE TABLE item_parents (
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    parent_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY(item_id, parent_id), CHECK(item_id <> parent_id)
);
CREATE INDEX children_of_parent ON item_parents(parent_id);
CREATE TABLE item_tags (
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    tag_id TEXT NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    PRIMARY KEY(item_id, tag_id)
);
CREATE TABLE participants (
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    participant TEXT NOT NULL,
    position INTEGER NOT NULL,
    PRIMARY KEY(item_id, participant)
);
CREATE TABLE event_overrides (
    id TEXT PRIMARY KEY NOT NULL,
    item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,
    original_start_date TEXT NOT NULL,
    original_end_date TEXT NOT NULL,
    new_start_date TEXT,
    new_end_date TEXT,
    cancelled_sign INTEGER NOT NULL CHECK(cancelled_sign IN (0,1)),
    status TEXT NOT NULL CHECK(status IN ('Pending','WIP','Done','On-Hold','Won''t Do','Someday','Event')),
    reminded INTEGER NOT NULL CHECK(reminded IN (0,1)),
    UNIQUE(item_id, original_start_date),
    CHECK((new_start_date IS NULL) = (new_end_date IS NULL))
);
"#;

// Rebuild only the status-constrained tables in the in-memory working copy.
// The original document is untouched until the user explicitly saves.
fn migrate_database(connection: &mut Connection) -> Result<bool> {
    let version: i64 =
        query_optional(connection, "PRAGMA user_version", [], |r| r.get(0))?.unwrap_or(0);
    if version >= SCHEMA_VERSION {
        return Ok(false);
    }
    connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let transaction = connection.transaction()?;
    for table in ["items", "event_overrides"]
        .into_iter()
        .filter(|_| version == 1)
    {
        let prefix = format!("CREATE TABLE {table} (");
        let definition = SCHEMA
            .split(&prefix)
            .nth(1)
            .ok_or_else(|| invalid("Missing migration schema"))?
            .split(";\n")
            .next()
            .ok_or_else(|| invalid("Missing migration table"))?;
        // All SQL identifiers and definitions here are application constants.
        transaction.execute_batch(&format!("CREATE TABLE {table}_v2 ({definition}; INSERT INTO {table}_v2 SELECT * FROM {table}; DROP TABLE {table}; ALTER TABLE {table}_v2 RENAME TO {table};"))?;
    }
    if version < 3 {
        transaction.execute_batch(super::notes::SCHEMA)?;
    }
    transaction.execute_batch(super::notes::RELATION_SCHEMA)?;
    transaction.execute_batch("PRAGMA user_version = 4;")?;
    transaction.commit()?;
    Ok(true)
}

/// An independent, in-memory document with explicit atomic Save/Save As.
pub struct Document {
    pub(super) connection: Connection,
    path: Option<PathBuf>,
    disk_fingerprint: Option<u64>,
    pub(super) dirty: bool,
}

impl Document {
    pub fn new() -> Result<Self> {
        let connection = Connection::open_in_memory()?;
        connection.execute_batch(SCHEMA)?;
        connection.execute_batch(super::notes::SCHEMA)?;
        connection.execute_batch(super::notes::RELATION_SCHEMA)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Self {
            connection,
            path: None,
            disk_fingerprint: None,
            dirty: false,
        })
    }

    pub fn open(path: &Path) -> Result<Self> {
        check_extension(path)?;
        let path = path.canonicalize()?;
        let source = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        validate_database(&source)?;
        let mut connection = Connection::open_in_memory()?;
        copy_database(&source, &mut connection)?;
        let migrated = migrate_database(&mut connection)?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        let document = Self {
            connection,
            disk_fingerprint: Some(fingerprint(&path)?),
            path: Some(path),
            dirty: migrated,
        };
        // Validate domain content as well as SQLite's physical/schema integrity.
        let items = document.items()?;
        let tags = document.tags()?;
        for note in document.notes()? {
            super::notes::validate_note(&note)?;
            super::notes::validate_note_references(&document.connection, &note)?;
        }
        for tag in &tags {
            validate_tag(tag)?;
        }
        validate_graph(&items)?;
        for item in &items {
            validate_item(item)?;
            for id in &item.tags {
                if !tags.iter().any(|tag| tag.id == *id) {
                    return Err(invalid("The document contains a missing tag"));
                }
            }
            if (item.status == Status::Done) != item.completion.is_some() {
                return Err(invalid(
                    "The document contains inconsistent completion dates",
                ));
            }
        }
        let overrides = document.overrides()?;
        let occurrence_statuses = OccurrenceStatuses::new(&overrides);
        for event in &overrides {
            let item = items
                .iter()
                .find(|item| item.id == event.item_id)
                .ok_or_else(|| invalid("The document contains an orphan occurrence"))?;
            validate_uuid(event.id)?;
            validate_override(item, event)?;
            if !event.cancelled {
                validate_occurrence_status(item.id, event.status, &items, &occurrence_statuses)?;
            }
        }
        super::query::validate_calendar(&items, &overrides)?;
        Ok(document)
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn save(&mut self) -> Result<()> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| invalid("Choose Save As for a new document"))?;
        self.save_as(&path)
    }

    /// Saves atomically. Existing destinations must be valid antodo documents;
    /// the UI obtains overwrite confirmation before calling this for Save As.
    pub fn save_as(&mut self, path: &Path) -> Result<()> {
        check_extension(path)?;
        let parent = path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = parent.canonicalize()?;
        let filename = path
            .file_name()
            .ok_or_else(|| invalid("Choose a filename ending in .antodo"))?;
        let destination = parent.join(filename);
        let destination = if destination.exists() {
            destination.canonicalize()?
        } else {
            destination
        };
        let parent = destination
            .parent()
            .ok_or_else(|| invalid("Choose a writable folder"))?;
        let before_save = if destination.exists() {
            Some(fingerprint(&destination)?)
        } else {
            None
        };
        let same_file = self
            .path
            .as_ref()
            .is_some_and(|current| *current == destination);
        if destination.exists() {
            if same_file && self.disk_fingerprint != Some(fingerprint(&destination)?) {
                return Err(invalid(
                    "This file changed on disk after it was opened. Use Save As with a different filename to preserve both versions",
                ));
            }
            let existing =
                Connection::open_with_flags(&destination, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
            validate_database(&existing)?;
            for suffix in ["-wal", "-shm", "-journal"] {
                let mut sidecar = destination.as_os_str().to_owned();
                sidecar.push(suffix);
                let sidecar = PathBuf::from(sidecar);
                if sidecar.exists() {
                    return Err(invalid(
                        "The destination is in use by another SQLite session. Close it or choose a different filename",
                    ));
                }
            }
        } else if same_file && self.disk_fingerprint.is_some() {
            return Err(invalid(
                "This file was removed after it was opened. Use Save As with a new filename",
            ));
        }
        let temporary = tempfile::NamedTempFile::new_in(parent)?;
        {
            let mut output = Connection::open(temporary.path())?;
            copy_database(&self.connection, &mut output)?;
        }
        temporary.as_file().sync_all()?;
        let current = if destination.exists() {
            Some(fingerprint(&destination)?)
        } else {
            None
        };
        if current != before_save {
            return Err(invalid(
                "The destination changed while saving. Save again or choose a different filename",
            ));
        }
        temporary
            .persist(&destination)
            .map_err(|error| error.error)?;
        self.disk_fingerprint = Some(fingerprint(&destination)?);
        self.path = Some(destination);
        self.dirty = false;
        Ok(())
    }

    pub fn items(&self) -> Result<Vec<Item>> {
        read_items(&self.connection)
    }
    pub fn tags(&self) -> Result<Vec<Tag>> {
        query(
            &self.connection,
            "SELECT id,name,color FROM tags ORDER BY name COLLATE NOCASE,id",
            [],
            |row| {
                Ok(Tag {
                    id: row_uuid(row, 0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                })
            },
        )
    }
    pub fn overrides(&self) -> Result<Vec<EventOverride>> {
        query(
            &self.connection,
            "SELECT id,item_id,original_start_date,original_end_date,new_start_date,new_end_date,cancelled_sign,status,reminded FROM event_overrides ORDER BY item_id,original_start_date",
            [],
            |row| {
                Ok(EventOverride {
                    id: row_uuid(row, 0)?,
                    item_id: row_uuid(row, 1)?,
                    original_start: row_json(row, 2)?,
                    original_end: row_json(row, 3)?,
                    new_start: row_optional_json(row, 4)?,
                    new_end: row_optional_json(row, 5)?,
                    cancelled: row.get(6)?,
                    status: row_status(row, 7)?,
                    reminded: row.get(8)?,
                })
            },
        )
    }

    pub fn save_item(&mut self, item: Item) -> Result<Item> {
        Ok(self.save_items(vec![item])?.remove(0))
    }

    /// Save a task and its complete note selection in the same transaction.
    /// The normal task commands preserve existing note links.
    pub fn save_item_with_notes(&mut self, item: Item, mut note_ids: Vec<Uuid>) -> Result<Item> {
        unique(&mut note_ids);
        let id = item.id;
        Ok(self
            .save_items_with_note_links(vec![item], Some((id, note_ids)))?
            .remove(0))
    }

    /// Validate the complete resulting graph before saving the batch atomically.
    /// Parent/child status transitions do not depend on the input ordering.
    pub fn save_items(&mut self, items: Vec<Item>) -> Result<Vec<Item>> {
        self.save_items_with_note_links(items, None)
    }

    fn save_items_with_note_links(
        &mut self,
        items: Vec<Item>,
        task_notes: Option<(Uuid, Vec<Uuid>)>,
    ) -> Result<Vec<Item>> {
        if items.is_empty() {
            return Ok(items);
        }
        if let Some((_, note_ids)) = &task_notes {
            super::notes::validate_task_note_ids(&self.connection, note_ids)?;
        }
        let existing = self.items()?;
        let tags = self.tags()?;
        let mut ids = HashSet::new();
        let mut changes = Vec::with_capacity(items.len());
        for item in items {
            if !ids.insert(item.id) {
                return Err(invalid("A task can only be saved once in a batch"));
            }
            let previous = existing.iter().find(|old| old.id == item.id);
            let change = prepare_item(item, previous)?;
            if change
                .item
                .tags
                .iter()
                .any(|id| !tags.iter().any(|tag| tag.id == *id))
            {
                return Err(invalid("Choose tags from the defined tag list"));
            }
            changes.push(change);
        }
        let mut prospective: Vec<_> = existing
            .into_iter()
            .filter(|item| !ids.contains(&item.id))
            .collect();
        prospective.extend(changes.iter().map(|change| change.item.clone()));
        validate_graph(&prospective)?;
        let surviving: Vec<_> = self
            .overrides()?
            .into_iter()
            .filter(|event| {
                !changes
                    .iter()
                    .any(|change| change.schedule_changed && change.item.id == event.item_id)
            })
            .collect();
        let occurrence_statuses = OccurrenceStatuses::new(&surviving);
        for event in &surviving {
            if !event.cancelled {
                validate_occurrence_status(
                    event.item_id,
                    event.status,
                    &prospective,
                    &occurrence_statuses,
                )?;
            }
        }
        super::query::validate_calendar(&prospective, &surviving)?;
        let transaction = self.connection.transaction()?;
        // Insert all records first so that newly created tasks may refer to
        // newly created parents in the same batch.
        for change in &changes {
            write_item(&transaction, &change.item)?;
        }
        for change in &changes {
            write_item_relations(&transaction, change)?;
        }
        if let Some((id, note_ids)) = task_notes {
            super::notes::replace_task_notes(&transaction, id, &note_ids)?;
        }
        transaction.commit()?;
        self.dirty = true;
        Ok(changes.into_iter().map(|change| change.item).collect())
    }

    pub fn delete_item(&mut self, id: Uuid) -> Result<()> {
        self.execute_existing(
            "DELETE FROM items WHERE id=?1",
            id,
            "This task no longer exists",
        )
    }

    pub fn save_tag(&mut self, mut tag: Tag) -> Result<Tag> {
        tag.name = tag.name.trim().to_owned();
        tag.color = tag.color.to_ascii_uppercase();
        validate_tag(&tag)?;
        if self
            .tags()?
            .iter()
            .any(|other| other.id != tag.id && other.name.to_lowercase() == tag.name.to_lowercase())
        {
            return Err(invalid("A tag with this name already exists"));
        }
        execute(
            &self.connection,
            "INSERT INTO tags(id,name,color) VALUES (?1,?2,?3) ON CONFLICT(id) DO UPDATE SET name=excluded.name,color=excluded.color",
            params![tag.id.to_string(), tag.name, tag.color],
        )?;
        self.dirty = true;
        Ok(tag)
    }

    pub fn delete_tag(&mut self, id: Uuid) -> Result<()> {
        self.execute_existing(
            "DELETE FROM tags WHERE id=?1",
            id,
            "This tag no longer exists",
        )
    }

    pub fn save_override(&mut self, mut event: EventOverride) -> Result<EventOverride> {
        validate_uuid(event.id)?;
        let items = self.items()?;
        let item = items
            .iter()
            .find(|item| item.id == event.item_id)
            .ok_or_else(|| invalid("This task no longer exists"))?;
        validate_override(item, &event)?;
        event.original_start = Moment::from_utc(
            event.original_start.utc()?,
            &event.original_start.timezone,
            event.original_start.is_date(),
        )?;
        event.original_end = Moment::from_utc(
            event.original_end.utc()?,
            &event.original_end.timezone,
            event.original_end.is_date(),
        )?;
        let overrides = self.overrides()?;
        for old in &overrides {
            if old.item_id == event.item_id
                && old.original_start.utc()? == event.original_start.utc()?
            {
                event.id = old.id;
                event.original_start = old.original_start.clone();
                event.original_end = old.original_end.clone();
                event.reminded |= old.reminded;
                if !same_moment(
                    Some(old.new_start.as_ref().unwrap_or(&old.original_start)),
                    Some(event.new_start.as_ref().unwrap_or(&event.original_start)),
                ) || !same_moment(
                    Some(old.new_end.as_ref().unwrap_or(&old.original_end)),
                    Some(event.new_end.as_ref().unwrap_or(&event.original_end)),
                ) || old.cancelled != event.cancelled
                    || old.status.is_closed() && !event.status.is_closed()
                {
                    event.reminded = false;
                }
            } else if old.id == event.id {
                return Err(invalid(
                    "An occurrence identity cannot be reassigned to another date",
                ));
            }
        }
        let mut prospective: Vec<_> = overrides
            .into_iter()
            .filter(|old| old.id != event.id)
            .collect();
        prospective.push(event.clone());
        if !event.cancelled {
            validate_occurrence_status(
                item.id,
                event.status,
                &items,
                &OccurrenceStatuses::new(&prospective),
            )?;
        }
        super::query::validate_calendar(std::slice::from_ref(item), &prospective)?;
        execute(
            &self.connection,
            "INSERT INTO event_overrides (id,item_id,original_start_date,original_end_date,new_start_date,new_end_date,cancelled_sign,status,reminded) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET new_start_date=excluded.new_start_date,new_end_date=excluded.new_end_date,cancelled_sign=excluded.cancelled_sign,status=excluded.status,reminded=excluded.reminded",
            params![
                event.id.to_string(),
                event.item_id.to_string(),
                serde_json::to_string(&event.original_start)?,
                serde_json::to_string(&event.original_end)?,
                json_optional(&event.new_start)?,
                json_optional(&event.new_end)?,
                event.cancelled,
                event.status.key(),
                event.reminded
            ],
        )?;
        self.dirty = true;
        Ok(event)
    }

    pub fn delete_override(&mut self, id: Uuid) -> Result<()> {
        let mut overrides = self.overrides()?;
        let old = overrides
            .iter()
            .find(|event| event.id == id)
            .ok_or_else(|| invalid("This occurrence change no longer exists"))?;
        let item = self
            .items()?
            .into_iter()
            .find(|item| item.id == old.item_id)
            .ok_or_else(|| invalid("This task no longer exists"))?;
        overrides.retain(|event| event.id != id);
        super::query::validate_calendar(std::slice::from_ref(&item), &overrides)?;
        self.execute_existing(
            "DELETE FROM event_overrides WHERE id=?1",
            id,
            "This occurrence change no longer exists",
        )
    }

    pub fn mark_reminded(&mut self, reminder: &Reminder) -> Result<()> {
        if let Some(occurrence) = &reminder.occurrence {
            if occurrence.item_id != reminder.item_id {
                return Err(invalid("The reminder belongs to a different task"));
            }
            let existing = self.overrides()?.into_iter().find(|event| {
                event.item_id == reminder.item_id
                    && event.original_start.utc().ok() == occurrence.original_start.utc().ok()
            });
            let mut event = existing.unwrap_or_else(|| EventOverride {
                id: Uuid::now_v7(),
                item_id: reminder.item_id,
                original_start: occurrence.original_start.clone(),
                original_end: occurrence.original_end.clone(),
                new_start: None,
                new_end: None,
                cancelled: false,
                status: occurrence.status,
                reminded: false,
            });
            event.reminded = true;
            self.save_override(event)?;
        } else {
            self.execute_existing(
                "UPDATE items SET reminded=1 WHERE id=?1",
                reminder.item_id,
                "This task no longer exists",
            )?;
        }
        Ok(())
    }

    fn execute_existing(&mut self, statement: &str, id: Uuid, missing: &str) -> Result<()> {
        if execute(&self.connection, statement, [id.to_string()])? == 0 {
            return Err(invalid(missing));
        }
        self.dirty = true;
        Ok(())
    }
}

struct ItemChange {
    item: Item,
    schedule_changed: bool,
    reminder_changed: bool,
}

fn prepare_item(mut item: Item, previous: Option<&Item>) -> Result<ItemChange> {
    item.name = item.name.trim().to_owned();
    item.participants = item
        .participants
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect();
    unique(&mut item.parents);
    unique(&mut item.tags);
    unique(&mut item.participants);
    if item.status == Status::Done {
        if item.completion.is_none() {
            item.completion = Some(Moment::from_utc(Utc::now(), &local_timezone(), false)?);
        }
    } else {
        item.completion = None;
    }
    if item.status == Status::Event {
        item.deadline = None;
        item.planned = None;
        item.completion = None;
        item.effort = 0;
    }
    validate_item(&item)?;
    let schedule_changed = previous.is_some_and(|old| {
        !same_moment(old.start.as_ref(), item.start.as_ref())
            || !same_moment(old.end.as_ref(), item.end.as_ref())
            || old.is_recurring != item.is_recurring
            || ((old.is_recurring || item.is_recurring)
                && normalized_rule(&old.rrule) != normalized_rule(&item.rrule))
    });
    let reminder_changed = previous.is_some_and(|old| {
        old.reminder != item.reminder
            || !same_moment(old.planned.as_ref(), item.planned.as_ref())
            || !same_moment(old.deadline.as_ref(), item.deadline.as_ref())
            || old.status.is_closed() && !item.status.is_closed()
    });
    if schedule_changed || reminder_changed {
        item.reminded = false;
    } else if let Some(old) = previous {
        // An editor may predate a reminder tick; technical state is owned
        // by the reminder service, never by the stale task form.
        item.reminded = old.reminded;
    }
    Ok(ItemChange {
        item,
        schedule_changed,
        reminder_changed,
    })
}

fn write_item(connection: &Connection, item: &Item) -> Result<()> {
    execute(
        connection,
        "INSERT INTO items (id,name,description,start_date,end_date,deadline,status,is_recurring,rrule,reminder,reminded,priority,severity,effort,completion_date,planned_date) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16) ON CONFLICT(id) DO UPDATE SET name=excluded.name,description=excluded.description,start_date=excluded.start_date,end_date=excluded.end_date,deadline=excluded.deadline,status=excluded.status,is_recurring=excluded.is_recurring,rrule=excluded.rrule,reminder=excluded.reminder,reminded=excluded.reminded,priority=excluded.priority,severity=excluded.severity,effort=excluded.effort,completion_date=excluded.completion_date,planned_date=excluded.planned_date",
        params![
            item.id.to_string(),
            item.name,
            item.description,
            json_optional(&item.start)?,
            json_optional(&item.end)?,
            json_optional(&item.deadline)?,
            item.status.key(),
            item.is_recurring,
            item.rrule,
            item.reminder,
            item.reminded,
            item.priority,
            item.severity,
            item.effort,
            json_optional(&item.completion)?,
            json_optional(&item.planned)?
        ],
    )?;
    Ok(())
}

fn write_item_relations(connection: &Connection, change: &ItemChange) -> Result<()> {
    let ItemChange {
        item,
        schedule_changed,
        reminder_changed,
    } = change;
    let item_id = item.id.to_string();
    replace_ordered(
        connection,
        "DELETE FROM item_parents WHERE item_id=?1",
        "INSERT INTO item_parents(item_id,parent_id,position) VALUES (?1,?2,?3)",
        &item_id,
        item.parents.iter().map(Uuid::to_string),
    )?;
    replace_ordered(
        connection,
        "DELETE FROM item_tags WHERE item_id=?1",
        "INSERT INTO item_tags(item_id,tag_id,position) VALUES (?1,?2,?3)",
        &item_id,
        item.tags.iter().map(Uuid::to_string),
    )?;
    replace_ordered(
        connection,
        "DELETE FROM participants WHERE item_id=?1",
        "INSERT INTO participants(item_id,participant,position) VALUES (?1,?2,?3)",
        &item_id,
        &item.participants,
    )?;
    if *schedule_changed {
        execute(
            connection,
            "DELETE FROM event_overrides WHERE item_id=?1",
            [item.id.to_string()],
        )?;
    } else if *reminder_changed {
        execute(
            connection,
            "UPDATE event_overrides SET reminded=0 WHERE item_id=?1",
            [item.id.to_string()],
        )?;
    }
    Ok(())
}

fn read_items(connection: &Connection) -> Result<Vec<Item>> {
    let mut items = query(
        connection,
        "SELECT id,name,description,start_date,end_date,deadline,status,is_recurring,rrule,reminder,reminded,priority,severity,effort,completion_date,planned_date FROM items ORDER BY name COLLATE NOCASE,id",
        [],
        |row| {
            Ok(Item {
                id: row_uuid(row, 0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                start: row_optional_json(row, 3)?,
                end: row_optional_json(row, 4)?,
                deadline: row_optional_json(row, 5)?,
                status: row_status(row, 6)?,
                is_recurring: row.get(7)?,
                rrule: row.get(8)?,
                reminder: row.get(9)?,
                reminded: row.get(10)?,
                priority: row.get(11)?,
                severity: row.get(12)?,
                effort: row.get(13)?,
                completion: row_optional_json(row, 14)?,
                planned: row_optional_json(row, 15)?,
                parents: vec![],
                tags: vec![],
                participants: vec![],
            })
        },
    )?;
    for item in &mut items {
        item.parents = query(
            connection,
            "SELECT parent_id FROM item_parents WHERE item_id=?1 ORDER BY position",
            [item.id.to_string()],
            |row| row_uuid(row, 0),
        )?;
        item.tags = query(
            connection,
            "SELECT tag_id FROM item_tags WHERE item_id=?1 ORDER BY position",
            [item.id.to_string()],
            |row| row_uuid(row, 0),
        )?;
        item.participants = query(
            connection,
            "SELECT participant FROM participants WHERE item_id=?1 ORDER BY position",
            [item.id.to_string()],
            |row| row.get(0),
        )?;
    }
    Ok(items)
}

fn validate_item(item: &Item) -> Result<()> {
    validate_uuid(item.id)?;
    if item.name.trim().is_empty() {
        return Err(invalid("Give the task a name"));
    }
    if !(1..=5).contains(&item.priority) || !(1..=5).contains(&item.severity) {
        return Err(invalid("Priority and severity must be between 1 and 5"));
    }
    match (&item.start, &item.end) {
        (Some(start), Some(end)) => {
            if start.utc()? > end.utc()? {
                return Err(invalid("End must not be before start"));
            }
        }
        (None, None) => {}
        _ => return Err(invalid("Provide both start and end, or leave both blank")),
    }
    for moment in [
        &item.start,
        &item.end,
        &item.deadline,
        &item.planned,
        &item.completion,
    ]
    .into_iter()
    .flatten()
    {
        moment.utc()?;
    }
    if item.reminder.is_some()
        && item.start.is_none()
        && item.planned.is_none()
        && item.deadline.is_none()
    {
        return Err(invalid(
            "A reminder needs a start, planned date, or deadline",
        ));
    }
    if item.reminder.is_some_and(|minutes| minutes > 525_600) {
        return Err(invalid(
            "Reminders must be between 0 and 525600 minutes before an occurrence",
        ));
    }
    validate_rule(item)
}

fn same_moment(left: Option<&Moment>, right: Option<&Moment>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => left.equivalent(right),
        _ => false,
    }
}

fn normalized_rule(rule: &str) -> String {
    let upper = rule.trim().to_ascii_uppercase();
    let mut parts: Vec<_> = upper
        .strip_prefix("RRULE:")
        .unwrap_or(&upper)
        .split(';')
        .collect();
    parts.sort_unstable();
    parts.join(";")
}

/// Check each edge once, then remove roots iteratively to detect cycles.
/// Sharing this pass keeps batch changes and deep document opens linear in
/// the number of tasks and parent links, without recursive stack growth.
fn validate_graph(items: &[Item]) -> Result<()> {
    let indices: HashMap<_, _> = items
        .iter()
        .enumerate()
        .map(|(index, item)| (item.id, index))
        .collect();
    let mut remaining: Vec<_> = items.iter().map(|item| item.parents.len()).collect();
    let mut children = vec![Vec::new(); items.len()];
    for (index, item) in items.iter().enumerate() {
        for parent in &item.parents {
            let &parent_index = indices
                .get(parent)
                .ok_or_else(|| invalid("A selected parent no longer exists"))?;
            if !item.status.is_closed() && items[parent_index].status == Status::Done {
                return Err(invalid(
                    "Complete all child tasks or mark them Won't Do before completing their parent. Reopen a completed parent before adding or reopening an unfinished child",
                ));
            }
            children[parent_index].push(index);
        }
    }
    let mut roots: Vec<_> = remaining
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect();
    let mut visited = 0;
    while let Some(index) = roots.pop() {
        visited += 1;
        for &child in &children[index] {
            remaining[child] -= 1;
            if remaining[child] == 0 {
                roots.push(child);
            }
        }
    }
    if visited != items.len() {
        return Err(invalid(
            "This parent would create a loop in the task hierarchy",
        ));
    }
    Ok(())
}

/// Summarize override states once per command. Relationship checks run for every
/// surviving override, so rescanning all overrides inside each check would make
/// task edits and document opens quadratic in the number of saved occurrences.
struct OccurrenceStatuses {
    done: HashSet<Uuid>,
    open: HashSet<Uuid>,
}

impl OccurrenceStatuses {
    fn new(overrides: &[EventOverride]) -> Self {
        let mut statuses = Self {
            done: HashSet::new(),
            open: HashSet::new(),
        };
        for event in overrides.iter().filter(|event| !event.cancelled) {
            if event.status == Status::Done {
                statuses.done.insert(event.item_id);
            } else if !event.status.is_closed() {
                statuses.open.insert(event.item_id);
            }
        }
        statuses
    }
}

fn validate_occurrence_status(
    id: Uuid,
    status: Status,
    items: &[Item],
    occurrences: &OccurrenceStatuses,
) -> Result<()> {
    if status == Status::Done
        && items.iter().any(|child| {
            child.parents.contains(&id)
                && (!child.status.is_closed() || occurrences.open.contains(&child.id))
        })
    {
        return Err(invalid(
            "Complete all child tasks or mark them Won't Do before completing this occurrence",
        ));
    }
    if !status.is_closed()
        && let Some(item) = items.iter().find(|item| item.id == id)
        && item.parents.iter().any(|parent| {
            items
                .iter()
                .any(|item| item.id == *parent && item.status == Status::Done)
                || occurrences.done.contains(parent)
        })
    {
        return Err(invalid(
            "Reopen the completed parent before reopening this occurrence",
        ));
    }
    Ok(())
}

fn validate_tag(tag: &Tag) -> Result<()> {
    validate_uuid(tag.id)?;
    if tag.name.trim().is_empty() {
        return Err(invalid("Give the tag a name"));
    }
    if tag.color.len() != 7
        || !tag.color.starts_with('#')
        || !tag.color.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(invalid("Use a six-digit hex color, for example #8B5CF6"));
    }
    Ok(())
}

pub(super) fn validate_uuid(id: Uuid) -> Result<()> {
    if id.get_version_num() != 7 {
        return Err(invalid("Record identifiers must be UUIDv7"));
    }
    Ok(())
}

fn validate_database(connection: &Connection) -> Result<()> {
    let application: i64 =
        query_optional(connection, "PRAGMA application_id", [], |row| row.get(0))?.unwrap_or(0);
    let version: i64 =
        query_optional(connection, "PRAGMA user_version", [], |row| row.get(0))?.unwrap_or(0);
    if application != APPLICATION_ID {
        return Err(invalid("This file is not an antodo document"));
    }
    if !(1..=SCHEMA_VERSION).contains(&version) {
        return Err(invalid(
            "This document uses an unsupported format version. Open it with the matching version of antodo",
        ));
    }
    let integrity: Vec<String> = query(connection, "PRAGMA quick_check", [], |row| row.get(0))?;
    if integrity != ["ok"] {
        return Err(invalid("The document failed SQLite's integrity check"));
    }
    let foreign_keys: Vec<String> =
        query(connection, "PRAGMA foreign_key_check", [], |row| row.get(0))?;
    if !foreign_keys.is_empty() {
        return Err(invalid("The document has broken task relationships"));
    }
    Ok(())
}

fn check_extension(path: &Path) -> Result<()> {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("antodo"))
    {
        return Err(invalid("Choose a filename ending in .antodo"));
    }
    Ok(())
}

fn fingerprint(path: &Path) -> Result<u64> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    std::fs::read(path)?.hash(&mut hasher);
    Ok(hasher.finish())
}

fn copy_database(source: &Connection, destination: &mut Connection) -> Result<()> {
    rusqlite::backup::Backup::new(source, destination)?.run_to_completion(
        128,
        Duration::from_millis(10),
        None,
    )?;
    Ok(())
}

pub(super) fn unique<T: Eq + Hash + Clone>(values: &mut Vec<T>) {
    let mut seen = HashSet::new();
    values.retain(|value| seen.insert(value.clone()));
}

fn json_optional<T: serde::Serialize>(value: &Option<T>) -> Result<Option<String>> {
    value
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(Into::into)
}

fn conversion(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, rusqlite::types::Type::Text, Box::new(error))
}

pub(super) fn row_uuid(row: &Row<'_>, index: usize) -> rusqlite::Result<Uuid> {
    let value: String = row.get(index)?;
    Uuid::parse_str(&value).map_err(|error| conversion(index, error))
}
fn row_json<T: DeserializeOwned>(row: &Row<'_>, index: usize) -> rusqlite::Result<T> {
    let value: String = row.get(index)?;
    serde_json::from_str(&value).map_err(|error| conversion(index, error))
}
fn row_optional_json<T: DeserializeOwned>(
    row: &Row<'_>,
    index: usize,
) -> rusqlite::Result<Option<T>> {
    let value: Option<String> = row.get(index)?;
    value
        .map(|value| serde_json::from_str(&value).map_err(|error| conversion(index, error)))
        .transpose()
}
fn row_status(row: &Row<'_>, index: usize) -> rusqlite::Result<Status> {
    let value: String = row.get(index)?;
    Status::parse(&value).map_err(|error| conversion(index, error))
}

#[cfg(test)]
mod graph_tests {
    use super::*;

    #[test]
    fn whole_graph_validation_handles_deep_chains_and_rejects_a_closing_cycle() {
        let mut items: Vec<_> = (0..10_000)
            .map(|index| Item::new(index.to_string()))
            .collect();
        for index in 1..items.len() {
            let parent = items[index - 1].id;
            items[index].parents.push(parent);
        }
        validate_graph(&items).unwrap();
        let leaf = items.last().unwrap().id;
        items[0].parents.push(leaf);
        assert!(
            validate_graph(&items)
                .unwrap_err()
                .to_string()
                .contains("loop")
        );
    }
}

#[cfg(test)]
mod event_migration_tests {
    use super::*;
    #[test]
    fn version_one_migrates_in_memory_and_saves_event_status() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("legacy.antodo");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                &SCHEMA
                    .replace("user_version = 4", "user_version = 1")
                    .replace("'Someday','Event'", "'Someday'"),
            )
            .unwrap();
        drop(legacy);
        let before = std::fs::read(&path).unwrap();
        let mut document = Document::open(&path).unwrap();
        assert!(document.is_dirty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        let mut item = Item::new("Appointment");
        item.status = Status::Event;
        let item = document.save_item(item).unwrap();
        document.save().unwrap();
        let reopened = Document::open(&path).unwrap();
        assert_eq!(reopened.items().unwrap()[0], item);
        assert!(!reopened.is_dirty());
    }
}
