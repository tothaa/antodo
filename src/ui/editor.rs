use crate::core::{
    EventOverride, Item, Moment, Note, Occurrence, Result, Status, invalid, local_timezone,
};
use iced::widget::{combo_box, text_editor};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum Field {
    Name,
    Participants,
    Priority,
    Severity,
    Effort,
    Reminder,
    Rule,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSlot {
    Start,
    End,
    Deadline,
    Planned,
    Completion,
}

#[derive(Debug, Clone)]
pub struct DateInput {
    pub value: String,
    pub timezone: String,
    pub timezones: combo_box::State<String>,
}
impl DateInput {
    pub fn from(value: &Option<Moment>) -> Self {
        Self {
            value: value.as_ref().map(|v| v.value.clone()).unwrap_or_default(),
            timezone: value
                .as_ref()
                .map(|v| v.timezone.clone())
                .unwrap_or_else(local_timezone),
            timezones: super::datetime::timezones(),
        }
    }
    pub fn set(&mut self, timezone: bool, value: String) {
        *if timezone {
            &mut self.timezone
        } else {
            &mut self.value
        } = value;
    }
    pub fn parse_preserving(&self, original: &Option<Moment>) -> Result<Option<Moment>> {
        if let Some(value) = original
            && value.value == self.value
            && value.timezone == self.timezone
        {
            return Ok(Some(value.clone()));
        }
        self.parse()
    }
    pub fn parse(&self) -> Result<Option<Moment>> {
        if self.value.trim().is_empty() {
            Ok(None)
        } else {
            Moment::parse(&self.value, &self.timezone).map(Some)
        }
    }
}

pub struct Editor {
    pub original: Item,
    pub name: String,
    pub description: text_editor::Content,
    pub status: Status,
    pub start: DateInput,
    pub end: DateInput,
    pub deadline: DateInput,
    pub planned: DateInput,
    pub completion: DateInput,
    pub tags: Vec<Uuid>,
    pub parents: Vec<Uuid>,
    pub notes: Vec<Uuid>,
    pub participants: String,
    pub priority: String,
    pub severity: String,
    pub effort: String,
    pub reminder: String,
    pub recurring: bool,
    pub rule: String,
    pub dirty: bool,
}
impl Editor {
    pub fn new(item: Item) -> Self {
        Self {
            name: item.name.clone(),
            description: text_editor::Content::with_text(&item.description),
            status: item.status,
            start: DateInput::from(&item.start),
            end: DateInput::from(&item.end),
            deadline: DateInput::from(&item.deadline),
            planned: DateInput::from(&item.planned),
            completion: DateInput::from(&item.completion),
            tags: item.tags.clone(),
            parents: item.parents.clone(),
            notes: Vec::new(),
            participants: item.participants.join(", "),
            priority: item.priority.to_string(),
            severity: item.severity.to_string(),
            effort: item.effort.to_string(),
            reminder: item.reminder.map(|v| v.to_string()).unwrap_or_default(),
            recurring: item.is_recurring,
            rule: item.rrule.clone(),
            original: item,
            dirty: false,
        }
    }
    pub fn with_notes(item: Item, notes: &[Note]) -> Self {
        let id = item.id;
        let mut editor = Self::new(item);
        editor.notes = notes
            .iter()
            .filter(|note| note.tasks.contains(&id))
            .map(|note| note.id)
            .collect();
        editor
    }
    pub fn date_mut(&mut self, field: DateSlot) -> &mut DateInput {
        match field {
            DateSlot::Start => &mut self.start,
            DateSlot::End => &mut self.end,
            DateSlot::Deadline => &mut self.deadline,
            DateSlot::Planned => &mut self.planned,
            DateSlot::Completion => &mut self.completion,
        }
    }
    pub fn field_mut(&mut self, field: Field) -> &mut String {
        match field {
            Field::Name => &mut self.name,
            Field::Participants => &mut self.participants,
            Field::Priority => &mut self.priority,
            Field::Severity => &mut self.severity,
            Field::Effort => &mut self.effort,
            Field::Reminder => &mut self.reminder,
            Field::Rule => &mut self.rule,
        }
    }
    pub fn item(&self) -> Result<Item> {
        let mut item = self.original.clone();
        item.name = self.name.clone();
        item.description = self.description.text();
        item.status = self.status;
        item.start = self.start.parse_preserving(&self.original.start)?;
        item.end = self.end.parse_preserving(&self.original.end)?;
        item.deadline = if self.status == Status::Event {
            None
        } else {
            self.deadline.parse_preserving(&self.original.deadline)?
        };
        item.planned = if self.status == Status::Event {
            None
        } else {
            self.planned.parse_preserving(&self.original.planned)?
        };
        item.completion = if self.status == Status::Event {
            None
        } else {
            self.completion
                .parse_preserving(&self.original.completion)?
        };
        item.tags = self.tags.clone();
        item.parents = self.parents.clone();
        item.participants = self
            .participants
            .split(',')
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_string)
            .collect();
        item.priority = self
            .priority
            .parse()
            .map_err(|_| invalid("Priority must be a number from 1 to 5"))?;
        item.severity = self
            .severity
            .parse()
            .map_err(|_| invalid("Severity must be a number from 1 to 5"))?;
        item.effort = if self.status == Status::Event {
            0
        } else {
            self.effort
                .parse()
                .map_err(|_| invalid("Effort must be a non-negative whole number"))?
        };
        item.reminder = if self.reminder.trim().is_empty() {
            None
        } else {
            Some(
                self.reminder
                    .parse()
                    .map_err(|_| invalid("Reminder must be a non-negative number of minutes"))?,
            )
        };
        item.is_recurring = self.recurring;
        item.rrule = if self.recurring {
            self.rule.clone()
        } else {
            String::new()
        };
        Ok(item)
    }
}

pub struct OccurrenceEditor {
    pub name: String,
    pub occurrence: Occurrence,
    pub start: DateInput,
    pub end: DateInput,
    pub status: Status,
    pub cancelled: bool,
    pub dirty: bool,
}
impl OccurrenceEditor {
    pub fn new(name: String, occurrence: Occurrence) -> Self {
        Self {
            name,
            start: DateInput::from(&Some(occurrence.start.clone())),
            end: DateInput::from(&Some(occurrence.end.clone())),
            status: occurrence.status,
            cancelled: false,
            occurrence,
            dirty: false,
        }
    }
    pub fn date_mut(&mut self, end: bool) -> &mut DateInput {
        if end { &mut self.end } else { &mut self.start }
    }
    pub fn value(&self, existing: &[EventOverride]) -> Result<EventOverride> {
        let previous = existing.iter().find(|o| {
            o.item_id == self.occurrence.item_id
                && o.original_start.utc().ok() == self.occurrence.original_start.utc().ok()
        });
        Ok(EventOverride {
            id: previous.map(|o| o.id).unwrap_or_else(Uuid::now_v7),
            item_id: self.occurrence.item_id,
            original_start: self.occurrence.original_start.clone(),
            original_end: self.occurrence.original_end.clone(),
            new_start: self
                .start
                .parse_preserving(&Some(self.occurrence.start.clone()))?,
            new_end: self
                .end
                .parse_preserving(&Some(self.occurrence.end.clone()))?,
            cancelled: self.cancelled,
            status: self.status,
            reminded: previous.is_some_and(|o| o.reminded),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn editor_preserves_full_description_and_timezones() {
        let mut item = Item::new("Example");
        item.description = "# Heading\n\nSome **bold** text".into();
        item.planned = Some(Moment::parse("2026-11-10 09:00", "Europe/Budapest").unwrap());
        let editor = Editor::new(item.clone());
        assert_eq!(editor.item().unwrap(), item);
    }
    #[test]
    fn editor_validates_numbers_and_optional_inputs() {
        let mut editor = Editor::new(Item::new("Task"));
        editor.priority = "bad".into();
        assert!(editor.item().is_err());
        editor.priority = "3".into();
        editor.participants = "Alice, alice@example.com, ".into();
        editor.recurring = false;
        editor.rule = "stale".into();
        let item = editor.item().unwrap();
        assert_eq!(item.participants.len(), 2);
        assert!(item.rrule.is_empty());
        editor.reminder = "-1".into();
        assert!(editor.item().is_err());
    }
    #[test]
    fn editors_preserve_fold_instants_and_existing_override_identity() {
        let instant = chrono::DateTime::parse_from_rfc3339("2030-10-27T01:30:45Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let start = Moment::from_utc(instant, "Europe/Budapest", false).unwrap();
        let end = Moment::from_utc(
            instant + chrono::Duration::hours(1),
            "Europe/Budapest",
            false,
        )
        .unwrap();
        let mut item = Item::new("Repeated hour");
        item.start = Some(start.clone());
        item.end = Some(end.clone());
        assert_eq!(Editor::new(item.clone()).item().unwrap(), item);
        let occurrence = Occurrence {
            item_id: item.id,
            original_start: start.clone(),
            original_end: end.clone(),
            start,
            end,
            status: Status::Pending,
            reminded: true,
            overridden: false,
        };
        let mut editor = OccurrenceEditor::new(item.name, occurrence.clone());
        let mut previous = editor.value(&[]).unwrap();
        previous.reminded = true;
        let saved = editor.value(&[previous.clone()]).unwrap();
        assert_eq!(saved.id, previous.id);
        assert!(saved.reminded);
        assert_eq!(saved.new_start, Some(occurrence.start));
        assert_eq!(saved.new_end, Some(occurrence.end));
        editor.date_mut(false).set(false, "2030-10-27 02:45".into());
        assert!(
            editor.value(&[previous]).is_err(),
            "new ambiguous input must not inherit the old fold offset"
        );
    }

    #[test]
    fn hidden_event_inputs_do_not_block_conversion_but_visible_inputs_do() {
        let mut editor = Editor::new(Item::new("Event"));
        editor.status = Status::Event;
        for date in [
            &mut editor.deadline,
            &mut editor.planned,
            &mut editor.completion,
        ] {
            date.value = "not a date".into();
        }
        editor.effort = "not a number".into();
        let event = editor.item().unwrap();
        assert!(event.deadline.is_none() && event.planned.is_none() && event.completion.is_none());
        assert_eq!(event.effort, 0);
        editor.severity = "invalid".into();
        assert!(editor.item().is_err());
        editor.severity = "3".into();
        editor.start.value = "invalid".into();
        assert!(editor.item().is_err());
    }
}
