//! Shared task commands for single and multiple task selections.
use std::collections::HashSet;

use chrono::Utc;
use uuid::Uuid;

use super::{Document, Item, Moment, Result, Status, invalid, local_timezone, sql};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskAction {
    Status(Status),
    PlanToday,
    Delete,
    ToggleTag(Uuid),
    Priority(u8),
    Severity(u8),
    LinkParent(Uuid),
}

/// A task at a particular point in a hierarchy with multiple parents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TaskOccurrence {
    pub item_id: Uuid,
    pub parent_id: Option<Uuid>,
}

impl Document {
    /// Apply one command atomically to distinct selected tasks.
    pub fn apply_task_action(&mut self, ids: &[Uuid], action: TaskAction) -> Result<()> {
        let mut selected = selected_items(&self.items()?, ids)?;
        if action == TaskAction::Delete {
            let transaction = self.connection.transaction()?;
            for item in selected {
                sql::execute(
                    &transaction,
                    "DELETE FROM items WHERE id=?1",
                    [item.id.to_string()],
                )?;
            }
            transaction.commit()?;
            self.dirty = true;
            return Ok(());
        }
        let now = Utc::now();
        for item in &mut selected {
            match action {
                TaskAction::Status(status) => item.status = status,
                TaskAction::PlanToday => {
                    if item.status == Status::Event {
                        return Err(invalid("Events use their start date and cannot be planned"));
                    }
                    let timezone = item
                        .planned
                        .as_ref()
                        .map(|planned| planned.timezone.clone())
                        .unwrap_or_else(local_timezone);
                    item.planned = Some(Moment::from_utc(now, &timezone, true)?);
                }
                TaskAction::ToggleTag(tag) => {
                    if item.tags.contains(&tag) {
                        item.tags.retain(|id| *id != tag);
                    } else {
                        item.tags.push(tag);
                    }
                }
                TaskAction::Priority(value) => item.priority = value,
                TaskAction::Severity(value) => item.severity = value,
                TaskAction::LinkParent(parent) => {
                    if !item.parents.contains(&parent) {
                        item.parents.push(parent);
                    }
                }
                TaskAction::Delete => unreachable!(),
            }
        }
        self.save_items(selected)?;
        Ok(())
    }

    /// Move selected hierarchy occurrences, or add an additional parent when
    /// `additive` is true. Only the displayed old links are removed. Every
    /// change is validated together; failure preserves the entire document.
    pub fn relink_items(
        &mut self,
        occurrences: &[TaskOccurrence],
        new_parent: Option<Uuid>,
        additive: bool,
    ) -> Result<()> {
        let existing = self.items()?;
        let ids: Vec<_> = occurrences.iter().map(|entry| entry.item_id).collect();
        let mut selected = selected_items(&existing, &ids)?;
        for occurrence in occurrences {
            let original = existing
                .iter()
                .find(|item| item.id == occurrence.item_id)
                .unwrap();
            if let Some(parent) = occurrence.parent_id
                && !original.parents.contains(&parent)
            {
                return Err(invalid("This parent link no longer exists"));
            }
        }
        if additive && new_parent.is_none() {
            return Ok(());
        }
        for item in &mut selected {
            if !additive {
                item.parents.retain(|parent| {
                    !occurrences
                        .iter()
                        .any(|entry| entry.item_id == item.id && entry.parent_id == Some(*parent))
                });
            }
            if let Some(parent) = new_parent
                && !item.parents.contains(&parent)
            {
                item.parents.push(parent);
            }
        }
        self.save_items(selected)?;
        Ok(())
    }
}

fn selected_items(items: &[Item], ids: &[Uuid]) -> Result<Vec<Item>> {
    if ids.is_empty() {
        return Err(invalid("Select at least one task"));
    }
    let mut seen = HashSet::new();
    ids.iter()
        .filter(|id| seen.insert(**id))
        .map(|id| {
            items
                .iter()
                .find(|item| item.id == *id)
                .cloned()
                .ok_or_else(|| invalid("A selected task no longer exists"))
        })
        .collect()
}
