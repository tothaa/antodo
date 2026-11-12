//! Named Markdown notes, tag styling, and task assignment controls.
use super::app::{App, Intent, Message};
use super::views::{soft_button, tag_stripes};
use crate::core::{self, Note};
use iced::widget::{
    Column, button, checkbox, column, container, row, scrollable, slider, text, text_editor,
    text_input,
};
use iced::{Alignment, Element, Fill};
use uuid::Uuid;

pub struct Editor {
    pub original: Note,
    pub name: String,
    pub content: text_editor::Content,
    pub tags: Vec<Uuid>,
    pub tasks: Vec<Uuid>,
    pub dirty: bool,
}

impl Editor {
    pub fn new(note: Note) -> Self {
        Self {
            name: note.name.clone(),
            content: text_editor::Content::with_text(&note.content),
            tags: note.tags.clone(),
            tasks: note.tasks.clone(),
            original: note,
            dirty: false,
        }
    }

    pub fn note(&self) -> Note {
        Note {
            id: self.original.id,
            name: self.name.clone(),
            content: self.content.text(),
            tags: self.tags.clone(),
            tasks: self.tasks.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Action {
    Search(String),
    Name(String),
    Content(text_editor::Action),
    ToggleTag(Uuid, bool),
    ToggleTask(Uuid, bool),
    OpenAssignments,
    CloseAssignments,
    AssignmentSearch(String),
    AssignmentStatus(Option<core::Status>),
    AssignmentTag(Option<Uuid>),
    AssignmentPriority(String),
    Assign(Uuid),
    ToggleTaskNote(Uuid, bool),
    Delete,
}

fn select(values: &mut Vec<Uuid>, id: Uuid, selected: bool) {
    values.retain(|value| *value != id);
    if selected {
        values.push(id);
    }
}

impl App {
    pub(super) fn update_note(&mut self, action: Action) {
        match action {
            Action::Search(value) => self.note_search = value,
            Action::Name(value) => {
                if let Some(editor) = &mut self.note_editor {
                    editor.name = value;
                    editor.dirty = true;
                }
            }
            Action::Content(action) => {
                if let Some(editor) = &mut self.note_editor {
                    editor.dirty |= action.is_edit();
                    editor.content.perform(action);
                }
            }
            Action::ToggleTag(id, selected) | Action::ToggleTask(id, selected) => {
                if let Some(editor) = &mut self.note_editor {
                    let values = if matches!(action, Action::ToggleTag(_, _)) {
                        &mut editor.tags
                    } else {
                        &mut editor.tasks
                    };
                    select(values, id, selected);
                    editor.dirty = true;
                }
            }
            Action::OpenAssignments => {
                if self.editor.is_some() || self.note_editor.is_some() {
                    self.assignment_picker = Some(AssignmentPicker {
                        context_tasks: vec![],
                        tasks: self.note_editor.is_some(),
                        filter: Default::default(),
                        priority: String::new(),
                    });
                }
            }
            Action::CloseAssignments => self.assignment_picker = None,
            Action::AssignmentSearch(value) => {
                if let Some(picker) = &mut self.assignment_picker {
                    picker.filter.search = value;
                }
            }
            Action::AssignmentStatus(value) => {
                if let Some(picker) = &mut self.assignment_picker {
                    picker.filter.status = value;
                }
            }
            Action::AssignmentTag(value) => {
                if let Some(picker) = &mut self.assignment_picker {
                    picker.filter.tag = value;
                }
            }
            Action::AssignmentPriority(value) => {
                if let Some(picker) = &mut self.assignment_picker {
                    picker.filter.priority = value.parse().ok();
                    picker.priority = value;
                }
            }
            Action::Assign(id) => {
                if let Some(picker) = &self.assignment_picker {
                    if !picker.context_tasks.is_empty() {
                        let ids = picker.context_tasks.clone();
                        if let Some(document) = &mut self.document {
                            let result = document.link_note_to_tasks(id, &ids);
                            if result.is_ok() {
                                self.assignment_picker = None;
                                self.interactions.selected.clear();
                            }
                            self.changed(result);
                        }
                    } else if picker.tasks && self.items.iter().any(|task| task.id == id) {
                        self.update_note(Action::ToggleTask(id, true));
                    } else if !picker.tasks && self.notes.iter().any(|note| note.id == id) {
                        self.update_note(Action::ToggleTaskNote(id, true));
                    }
                }
            }
            Action::ToggleTaskNote(id, selected) => {
                if let Some(editor) = &mut self.editor {
                    select(&mut editor.notes, id, selected);
                    editor.dirty = true;
                }
            }
            Action::Delete => {
                if self.delete_confirm
                    && let (Some(document), Some(editor)) = (&mut self.document, &self.note_editor)
                {
                    let result = document.delete_note(editor.original.id);
                    if result.is_ok() {
                        self.note_editor = None;
                        self.delete_confirm = false;
                    }
                    let success = result.is_ok();
                    self.changed(result);
                    if success {
                        self.return_to_drawer();
                    }
                }
            }
        }
    }

    pub(super) fn apply_note(&mut self) -> core::Result<()> {
        if let Some(editor) = &self.note_editor {
            self.document
                .as_mut()
                .ok_or_else(|| core::invalid("Open a file first"))?
                .save_note(editor.note())?;
            self.note_editor = None;
        }
        Ok(())
    }
}

/// Tag stripes use the same renderer as task cards, including shared colors.
fn card(app: &App, note: &Note) -> Element<'static, Message> {
    let excerpt: String = note
        .content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(140)
        .collect();
    button(
        row![
            tag_stripes(app, &note.tags, 4.into(), 64.into()).spacing(2),
            container(column![text(note.name.clone()).size(19), text(excerpt).size(14)].spacing(6))
                .padding(16)
                .width(Fill),
        ]
        .align_y(Alignment::Center),
    )
    .width(Fill)
    .padding(0)
    .style(soft_button)
    .on_press(Message::Intent(Intent::SelectNote(note.id)))
    .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let search = text_input("Search note names and Markdown content…", &app.note_search)
        .on_input(|value| Message::Note(Action::Search(value)))
        .padding(12);
    let notes = app
        .document
        .as_ref()
        .map(|doc| doc.search_notes(&app.note_search));
    let mut list = Column::new().spacing(10);
    match notes {
        Some(Ok(notes)) if !notes.is_empty() => {
            for note in notes {
                list = list.push(card(app, &note));
            }
        }
        Some(Err(error)) => list = list.push(text(error.to_string())),
        _ => {
            list = list.push(text(if app.note_search.trim().is_empty() {
                "No notes yet. Add a note to keep ideas and reference material here."
            } else {
                "No notes match your search."
            }))
        }
    }
    column![search, scrollable(list).height(Fill)]
        .spacing(16)
        .into()
}

fn tag_options<'a>(app: &'a App, editor: &Editor) -> Element<'a, Message> {
    let mut options = Column::new().spacing(8);
    for tag in &app.tags {
        let id = tag.id;
        options = options.push(
            row![
                tag_stripes(app, [&id], 4.into(), 24.into()),
                checkbox(editor.tags.contains(&id))
                    .label(&tag.name)
                    .on_toggle(move |value| Message::Note(Action::ToggleTag(id, value))),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    if app.tags.is_empty() {
        options = options.push(text("Create your first tag in the Tags view.").size(13));
    }
    column![
        text("Tags").size(20),
        scrollable(options.padding(iced::Padding::new(0.0).right(12))).height(110)
    ]
    .spacing(10)
    .into()
}

pub struct AssignmentPicker {
    /// Nonempty for immediate context-menu linking; empty for a drawer draft.
    pub context_tasks: Vec<Uuid>,
    pub tasks: bool,
    pub filter: core::query::Filter,
    pub priority: String,
}

fn assignment_header(title: String) -> Element<'static, Message> {
    row![
        text(title).size(22).width(Fill),
        button("Add…").on_press(Message::Note(Action::OpenAssignments))
    ]
    .spacing(10)
    .into()
}

fn assigned_row<'a>(
    app: &App,
    name: &'a str,
    tags: &[Uuid],
    open: Intent,
    detach: Action,
) -> Element<'a, Message> {
    row![
        tag_stripes(app, tags, 4.into(), 30.into()).spacing(2),
        button(text(name))
            .style(button::text)
            .width(Fill)
            .on_press(Message::Intent(open)),
        button("Detach")
            .style(soft_button)
            .on_press(Message::Note(detach))
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn task_options<'a>(app: &'a App, editor: &Editor) -> Element<'a, Message> {
    let mut rows = column![assignment_header(format!(
        "Assigned tasks · {}",
        editor.tasks.len()
    ))]
    .spacing(10);
    for task in app
        .items
        .iter()
        .filter(|task| editor.tasks.contains(&task.id))
    {
        rows = rows.push(assigned_row(
            app,
            &task.name,
            &task.tags,
            Intent::Select(task.id),
            Action::ToggleTask(task.id, false),
        ));
    }
    if editor.tasks.is_empty() {
        rows = rows.push(text("No tasks assigned.").size(13));
    }
    rows.into()
}

/// Only assigned Notes appear in the drawer; adding uses the shared selector.
pub fn task_section(app: &App) -> Element<'_, Message> {
    let editor = app.editor.as_ref().expect("task notes require an editor");
    let mut rows = column![assignment_header(format!(
        "Notes · {} assigned",
        editor.notes.len()
    ))]
    .spacing(10);
    for note in app
        .notes
        .iter()
        .filter(|note| editor.notes.contains(&note.id))
    {
        rows = rows.push(assigned_row(
            app,
            &note.name,
            &note.tags,
            Intent::SelectNote(note.id),
            Action::ToggleTaskNote(note.id, false),
        ));
    }
    if editor.notes.is_empty() {
        rows = rows.push(text("No notes assigned.").size(13));
    }
    rows.into()
}

pub fn assignment_dialog(app: &App) -> Element<'_, Message> {
    use super::views::choices;
    let picker = app
        .assignment_picker
        .as_ref()
        .expect("assignment dialog requires picker");
    let mut content = column![
        row![
            text(if !picker.context_tasks.is_empty() {
                "Link a Note"
            } else if picker.tasks {
                "Add task"
            } else {
                "Add note"
            })
            .size(22)
            .width(Fill),
            button("× Close").on_press(Message::Note(Action::CloseAssignments))
        ],
        text_input(
            if picker.tasks {
                "Search tasks"
            } else {
                "Search notes"
            },
            &picker.filter.search
        )
        .on_input(|value| Message::Note(Action::AssignmentSearch(value)))
        .padding(10),
        choices(
            "Any tag",
            app.tags
                .iter()
                .map(|tag| (tag.id, tag.name.clone()))
                .collect(),
            picker.filter.tag,
            |value| Message::Note(Action::AssignmentTag(value))
        ),
    ]
    .spacing(12);
    if picker.tasks {
        content = content.push(
            row![
                choices(
                    "Any status",
                    core::Status::ALL
                        .into_iter()
                        .map(|status| (status, status.to_string()))
                        .collect(),
                    picker.filter.status,
                    |value| Message::Note(Action::AssignmentStatus(value))
                ),
                text_input("Priority", &picker.priority)
                    .on_input(|value| Message::Note(Action::AssignmentPriority(value))),
            ]
            .spacing(8),
        );
    }
    let mut results = Column::new().spacing(4);
    let mut count = 0;
    if picker.tasks {
        if let Some(editor) = &app.note_editor {
            for task in core::query::filtered(&app.items, &picker.filter)
                .into_iter()
                .filter(|task| !editor.tasks.contains(&task.id))
            {
                count += 1;
                results = results.push(
                    button(text(&task.name))
                        .width(Fill)
                        .on_press(Message::Note(Action::Assign(task.id))),
                );
            }
        }
    } else if let Some(document) = &app.document {
        match document.search_notes_filtered(&picker.filter.search, picker.filter.tag, None) {
            Ok(notes) => {
                for note in notes.into_iter().filter(|note| {
                    if picker.context_tasks.is_empty() {
                        app.editor
                            .as_ref()
                            .is_some_and(|editor| !editor.notes.contains(&note.id))
                    } else {
                        picker
                            .context_tasks
                            .iter()
                            .any(|task| !note.tasks.contains(task))
                    }
                }) {
                    count += 1;
                    results = results.push(
                        button(
                            row![
                                tag_stripes(app, &note.tags, 4.into(), 26.into()).spacing(2),
                                text(note.name)
                            ]
                            .spacing(8),
                        )
                        .width(Fill)
                        .on_press(Message::Note(Action::Assign(note.id))),
                    );
                }
            }
            Err(error) => {
                count += 1;
                results = results.push(text(error.to_string()).style(text::danger));
            }
        }
    }
    if !picker.context_tasks.is_empty()
        && let Some(error) = &app.error
    {
        content = content.push(text(error).style(text::danger));
    }
    if count == 0 {
        results = results.push(text("No unassigned matches.").size(13));
    }
    content
        .push(scrollable(results.padding(iced::Padding::new(0.0).right(12))).height(340))
        .into()
}

pub fn drawer(app: &App, _width: f32) -> Element<'_, Message> {
    let editor = app
        .note_editor
        .as_ref()
        .expect("note drawer requires editor");
    let exists = app.notes.iter().any(|note| note.id == editor.original.id);
    let header = row![
        button("Close")
            .style(soft_button)
            .on_press(Message::Intent(Intent::CloseDrawer)),
        tag_stripes(app, &editor.tags, 4.into(), 32.into()).spacing(2),
        text("Edit note").size(24).width(Fill),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    let mut actions = row![].spacing(10);
    if app.llm_config.is_some() && exists {
        actions =
            actions.push(button("Start chat").on_press(Message::NoteChatOpen(editor.original.id)));
    }
    actions = actions
        .push(button("Apply changes").on_press(Message::Apply))
        .push(
            button("Delete")
                .style(button::danger)
                .on_press_maybe(exists.then_some(Message::DeleteRequest)),
        );
    let mut body = column![].spacing(18);
    if app.delete_confirm {
        body = body.push(
            column![
                text("Delete this note?"),
                row![
                    button("Delete note")
                        .style(button::danger)
                        .on_press(Message::Note(Action::Delete)),
                    button("Keep note")
                        .style(soft_button)
                        .on_press(Message::DeleteCancel),
                ]
                .spacing(10)
            ]
            .spacing(10),
        );
    }
    body = body
        .push(text("Name"))
        .push(
            text_input("Note name", &editor.name)
                .on_input(|value| Message::Note(Action::Name(value)))
                .padding(12),
        )
        .push(text("Content · Markdown"))
        .push(
            text_editor(&editor.content)
                .on_action(|action| Message::Note(Action::Content(action)))
                .highlight_with::<super::markdown_editor::MarkdownHighlighter>(
                    app.preferences.dark_mode,
                    super::markdown_editor::format,
                )
                .height(app.description_height)
                .padding(12),
        )
        .push(
            row![
                text("Editor height").size(13),
                slider(
                    100.0..=650.0,
                    app.description_height,
                    Message::DescriptionHeight
                )
                .step(10.0_f32),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .push(tag_options(app, editor))
        .push(task_options(app, editor));
    column![
        header,
        actions.wrap(),
        scrollable(body.padding(iced::Padding::new(0.0).right(12))).height(Fill)
    ]
    .spacing(16)
    .height(Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::super::app::Page;
    use super::*;
    use crate::core::preferences::Preferences;

    fn send(app: &mut App, message: Message) {
        drop(app.update(message));
    }

    fn app() -> App {
        let mut app = App::empty(Preferences::default());
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Intent(Intent::Switch(Page::Notes)));
        app
    }

    #[test]
    fn notes_create_edit_delete_and_search_keep_markdown_and_hide_ids() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::AddNote));
        let id = app.note_editor.as_ref().unwrap().original.id;
        send(
            &mut app,
            Message::Note(Action::Name("Release ideas".into())),
        );
        let markdown = "# Ideas\n\n- Keep **Markdown**\n- Árvíz";
        send(
            &mut app,
            Message::Note(Action::Content(text_editor::Action::Edit(
                text_editor::Edit::Paste(std::sync::Arc::new(markdown.into())),
            ))),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(app.note_editor.is_none());
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes[0].content.trim_end(), markdown);
        send(&mut app, Message::Note(Action::Search("markdown".into())));
        assert_eq!(
            app.document
                .as_ref()
                .unwrap()
                .search_notes(&app.note_search)
                .unwrap()
                .len(),
            1
        );
        send(&mut app, Message::Intent(Intent::SelectNote(id)));
        send(
            &mut app,
            Message::Note(Action::Name("Updated ideas".into())),
        );
        send(&mut app, Message::Apply);
        assert_eq!(app.notes[0].name, "Updated ideas");
        send(&mut app, Message::Intent(Intent::SelectNote(id)));
        send(&mut app, Message::Note(Action::Delete));
        assert_eq!(
            app.notes.len(),
            1,
            "deletion requires the confirmation action"
        );
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::DeleteCancel);
        assert_eq!(app.notes.len(), 1);
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::Note(Action::Delete));
        assert!(app.notes.is_empty());
        assert!(app.note_editor.is_none());
    }

    #[test]
    fn notes_guard_unsaved_changes_and_preserve_invalid_drafts() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::AddNote));
        send(&mut app, Message::Note(Action::Name("  ".into())));
        send(&mut app, Message::Apply);
        assert!(app.error.is_some());
        assert!(app.note_editor.as_ref().unwrap().dirty);
        send(&mut app, Message::Intent(Intent::Switch(Page::Calendar)));
        assert!(app.pending.is_some());
        send(&mut app, Message::Continue(true));
        assert!(app.pending.is_some());
        assert_eq!(app.page, Page::Notes);
        send(&mut app, Message::CancelIntent);
        send(&mut app, Message::Note(Action::Name("Keep this".into())));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert_eq!(app.notes[0].name, "Keep this");
        assert!(app.note_editor.is_none());
        let id = app.notes[0].id;
        send(&mut app, Message::Intent(Intent::SelectNote(id)));
        send(&mut app, Message::Note(Action::Name("Discard this".into())));
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        send(&mut app, Message::Continue(false));
        assert_eq!(app.notes[0].name, "Keep this");
        assert!(app.note_editor.is_none());
        assert_eq!(app.page, Page::Tasks);
    }

    #[test]
    fn notes_page_and_drawer_render_named_controls_in_both_themes() {
        let mut app = app();
        let note = app
            .document
            .as_mut()
            .unwrap()
            .save_note(Note::new("Named note"))
            .unwrap();
        app.notes = vec![note.clone()];
        for dark in [false, true] {
            app.preferences.dark_mode = dark;
            for width in [900.0, 1400.0] {
                let snapshot =
                    super::super::views::render_tests::render(app.view(), &app.theme(), width);
                assert!(snapshot.contains("+ New note"));
                assert!(snapshot.contains("Named note"));
                assert!(!snapshot.contains(&note.id.to_string()));
                assert!(!snapshot.contains("+ New task"));
                send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
                let snapshot =
                    super::super::views::render_tests::render(app.view(), &app.theme(), width);
                assert!(snapshot.contains("Edit note"));
                assert!(snapshot.contains("Content · Markdown"));
                assert!(snapshot.contains("Apply changes"));
                assert!(snapshot.contains("Delete"));
                send(&mut app, Message::Intent(Intent::CloseDrawer));
            }
        }
    }
    fn linked_fixture() -> (
        App,
        core::Item,
        core::Item,
        Note,
        Note,
        core::Tag,
        core::Tag,
    ) {
        let mut app = app();
        let document = app.document.as_mut().unwrap();
        let first = document.save_item(core::Item::new("First task")).unwrap();
        let second = document.save_item(core::Item::new("Second task")).unwrap();
        let red = document
            .save_tag(core::Tag::new("Research", "#EF4567"))
            .unwrap();
        let blue = document
            .save_tag(core::Tag::new("Reference", "#4567EF"))
            .unwrap();
        let mut note = Note::new("Shared note");
        note.content = "# Shared reference".into();
        note.tags = vec![red.id, blue.id];
        note.tasks = vec![first.id, second.id];
        let note = document.save_note(note).unwrap();
        let other = document.save_note(Note::new("Other note")).unwrap();
        app.changed(Ok(()));
        (app, first, second, note, other, red, blue)
    }

    #[test]
    fn note_and_task_editors_stage_many_to_many_assignments_and_preserve_other_links() {
        let (mut app, first, second, note, other, red, blue) = linked_fixture();
        send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
        send(&mut app, Message::Note(Action::OpenAssignments));
        send(
            &mut app,
            Message::Note(Action::AssignmentSearch("Second".into())),
        );
        send(&mut app, Message::Note(Action::CloseAssignments));
        assert!(!app.note_editor.as_ref().unwrap().dirty);
        send(&mut app, Message::Note(Action::ToggleTask(first.id, false)));
        send(&mut app, Message::Note(Action::ToggleTag(red.id, false)));
        assert_eq!(app.document.as_ref().unwrap().note(note.id).unwrap(), note);
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        assert!(app.pending.is_some());
        send(&mut app, Message::Continue(false));
        assert_eq!(app.document.as_ref().unwrap().note(note.id).unwrap(), note);
        send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
        send(&mut app, Message::Note(Action::ToggleTag(red.id, false)));
        send(&mut app, Message::Apply);
        assert_eq!(
            app.document.as_ref().unwrap().note(note.id).unwrap().tags,
            [blue.id]
        );
        send(&mut app, Message::Intent(Intent::Select(first.id)));
        assert_eq!(app.editor.as_ref().unwrap().notes, [note.id]);
        send(&mut app, Message::Note(Action::OpenAssignments));
        send(
            &mut app,
            Message::Note(Action::AssignmentSearch("Other".into())),
        );
        send(&mut app, Message::Note(Action::CloseAssignments));
        assert!(!app.editor.as_ref().unwrap().dirty);
        send(
            &mut app,
            Message::Note(Action::ToggleTaskNote(other.id, true)),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_none(), "{:?}", app.error);
        let document = app.document.as_ref().unwrap();
        assert_eq!(document.note(note.id).unwrap().tasks, [first.id, second.id]);
        assert_eq!(document.note(other.id).unwrap().tasks, [first.id]);
        send(&mut app, Message::Intent(Intent::Select(first.id)));
        send(
            &mut app,
            Message::Note(Action::ToggleTaskNote(note.id, false)),
        );
        send(&mut app, Message::Intent(Intent::SelectNote(other.id)));
        assert!(
            app.pending.is_some(),
            "opening a Note protects a task relationship draft"
        );
        send(&mut app, Message::Continue(true));
        assert_eq!(
            app.document.as_ref().unwrap().note(note.id).unwrap().tasks,
            [second.id]
        );
        assert_eq!(app.note_editor.as_ref().unwrap().original.id, other.id);
    }

    #[test]
    fn invalid_note_links_and_invalid_task_edits_preserve_relationship_drafts() {
        let (mut app, first, _, note, other, _, _) = linked_fixture();
        send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
        send(
            &mut app,
            Message::Note(Action::ToggleTask(Uuid::now_v7(), true)),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_some());
        assert!(app.note_editor.as_ref().unwrap().dirty);
        assert_eq!(app.document.as_ref().unwrap().note(note.id).unwrap(), note);
        send(&mut app, Message::Intent(Intent::Select(first.id)));
        send(&mut app, Message::Continue(false));
        send(
            &mut app,
            Message::Note(Action::ToggleTaskNote(other.id, true)),
        );
        send(
            &mut app,
            Message::EditField(super::super::editor::Field::Name, "".into()),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_some());
        assert!(app.editor.as_ref().unwrap().dirty);
        assert!(
            app.document
                .as_ref()
                .unwrap()
                .note(other.id)
                .unwrap()
                .tasks
                .is_empty()
        );
    }

    #[test]
    fn note_tag_colors_are_drawn_on_cards_drawers_and_task_note_choices() {
        use super::super::views::render_tests::Screen;
        let (mut app, first, _, mut note, _, red, blue) = linked_fixture();
        note.name =
            "A shared reference with a long name that wraps on a narrow Notes card".repeat(2);
        note = app.document.as_mut().unwrap().save_note(note).unwrap();
        app.changed(Ok(()));
        for dark in [false, true] {
            app.preferences.dark_mode = dark;
            let theme = app.theme();
            // Exercise cards inside the real scrollable Notes list: a Fill-height
            // stripe in an unbounded list can draw outside the visible viewport.
            for width in [360.0, 1000.0] {
                let size = iced::Size::new(width, 900.0);
                let mut screen = Screen::new(view(&app), size);
                screen.draw(&theme);
                for tag in [&red, &blue] {
                    assert!(
                        screen
                            .quads_with_color(super::super::views::color(&tag.color))
                            .iter()
                            .any(|bounds| bounds.width == 4.0
                                && bounds.height >= 20.0
                                && bounds.height < size.height
                                && bounds.y >= 0.0
                                && bounds.y + bounds.height <= size.height
                                && bounds.x >= 0.0
                                && bounds.x + bounds.width <= size.width)
                    );
                }
            }
            send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
            let mut screen = Screen::new(drawer(&app, 600.0), iced::Size::new(600.0, 900.0));
            screen.draw(&theme);
            for tag in [&red, &blue] {
                assert!(
                    screen
                        .quads_with_color(super::super::views::color(&tag.color))
                        .iter()
                        .any(|bounds| bounds.width == 4.0 && bounds.height == 32.0)
                );
            }
            let snapshot = screen.snapshot();
            assert!(snapshot.contains("Assigned tasks · 2"));
            assert!(snapshot.contains("Research"));
            assert!(snapshot.contains("Reference"));
            assert!(!snapshot.inputs.contains(&note.id.to_string()));
            drop(screen);
            send(&mut app, Message::Intent(Intent::Select(first.id)));
            let mut screen = Screen::new(task_section(&app), iced::Size::new(600.0, 400.0));
            screen.draw(&theme);
            for tag in [&red, &blue] {
                assert!(
                    screen
                        .quads_with_color(super::super::views::color(&tag.color))
                        .iter()
                        .any(|bounds| bounds.width == 4.0 && bounds.height == 30.0)
                );
            }
            assert!(screen.snapshot().contains("Notes · 1 assigned"));
            drop(screen);
            send(&mut app, Message::Intent(Intent::CloseDrawer));
        }
    }
    fn click_assignment(app: &mut App, label: &str) {
        use super::super::views::render_tests::Screen;
        let messages = {
            let mut screen = Screen::new(assignment_dialog(app), iced::Size::new(700.0, 600.0));
            screen.click(label)
        };
        for message in messages {
            send(app, message);
        }
    }

    #[test]
    fn task_note_assignment_dialog_filters_adds_and_detaches_staged_links() {
        use super::super::views::render_tests::{Screen, render};
        let (mut app, first, _, note, other, red, _) = linked_fixture();
        send(&mut app, Message::Intent(Intent::Select(first.id)));
        let mut screen = Screen::new(task_section(&app), iced::Size::new(600.0, 400.0));
        assert!(screen.snapshot().contains("Shared note"));
        assert!(!screen.snapshot().contains("Other note"));
        let messages = screen.click("Add…");
        drop(screen);
        for message in messages {
            send(&mut app, message);
        }
        assert!(app.assignment_picker.is_some());
        assert!(!app.editor.as_ref().unwrap().dirty);
        let snapshot = render(assignment_dialog(&app), &app.theme(), 700.0);
        assert!(snapshot.contains("Add note"));
        assert!(snapshot.contains("Other note"));
        assert!(!snapshot.contains("Shared note"));
        send(&mut app, Message::Note(Action::AssignmentTag(Some(red.id))));
        assert!(
            render(assignment_dialog(&app), &app.theme(), 700.0).contains("No unassigned matches.")
        );
        send(&mut app, Message::Note(Action::AssignmentTag(None)));
        send(
            &mut app,
            Message::Note(Action::AssignmentSearch("Other".into())),
        );
        send(
            &mut app,
            Message::EditField(super::super::editor::Field::Name, "Blocked".into()),
        );
        send(&mut app, Message::Apply);
        assert_eq!(app.editor.as_ref().unwrap().name, first.name);
        assert!(app.assignment_picker.is_some());
        click_assignment(&mut app, "Other note");
        assert!(app.editor.as_ref().unwrap().notes.contains(&other.id));
        assert!(app.editor.as_ref().unwrap().dirty);
        assert!(
            app.document
                .as_ref()
                .unwrap()
                .note(other.id)
                .unwrap()
                .tasks
                .is_empty()
        );
        assert!(
            render(assignment_dialog(&app), &app.theme(), 700.0).contains("No unassigned matches.")
        );
        click_assignment(&mut app, "× Close");
        assert!(app.assignment_picker.is_none());
        let mut screen = Screen::new(task_section(&app), iced::Size::new(600.0, 400.0));
        let messages = screen.click("Detach");
        drop(screen);
        for message in messages {
            send(&mut app, message);
        }
        assert_eq!(app.editor.as_ref().unwrap().notes, [note.id]);
        send(&mut app, Message::Apply);
        assert!(
            app.document
                .as_ref()
                .unwrap()
                .note(other.id)
                .unwrap()
                .tasks
                .is_empty()
        );
    }

    #[test]
    fn note_task_assignment_dialog_filters_and_keeps_unsaved_changes_protected() {
        use super::super::views::render_tests::render;
        let (mut app, mut first, _, _, other, red, _) = linked_fixture();
        first.tags = vec![red.id];
        first.status = core::Status::Wip;
        first.priority = 2;
        app.document
            .as_mut()
            .unwrap()
            .save_item(first.clone())
            .unwrap();
        app.changed(Ok(()));
        send(&mut app, Message::Intent(Intent::SelectNote(other.id)));
        assert!(!render(drawer(&app, 600.0), &app.theme(), 600.0).contains("First task"));
        send(&mut app, Message::Note(Action::OpenAssignments));
        send(
            &mut app,
            Message::Note(Action::AssignmentSearch("First".into())),
        );
        send(
            &mut app,
            Message::Note(Action::AssignmentStatus(Some(core::Status::Done))),
        );
        assert!(
            render(assignment_dialog(&app), &app.theme(), 700.0).contains("No unassigned matches.")
        );
        send(
            &mut app,
            Message::Note(Action::AssignmentStatus(Some(core::Status::Wip))),
        );
        send(&mut app, Message::Note(Action::AssignmentTag(Some(red.id))));
        send(
            &mut app,
            Message::Note(Action::AssignmentPriority("2".into())),
        );
        let snapshot = render(assignment_dialog(&app), &app.theme(), 700.0);
        assert!(snapshot.contains("Add task"));
        assert!(snapshot.contains("First task"));
        assert!(!snapshot.contains("Second task"));
        click_assignment(&mut app, "First task");
        assert_eq!(app.note_editor.as_ref().unwrap().tasks, [first.id]);
        send(
            &mut app,
            Message::Key(
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                iced::keyboard::Modifiers::default(),
            ),
        );
        assert!(app.assignment_picker.is_none());
        assert!(app.note_editor.as_ref().unwrap().dirty);
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        assert!(app.pending.is_some());
        send(&mut app, Message::CancelIntent);
        send(&mut app, Message::Apply);
        assert_eq!(
            app.document.as_ref().unwrap().note(other.id).unwrap().tasks,
            [first.id]
        );
        send(&mut app, Message::Intent(Intent::SelectNote(other.id)));
        send(&mut app, Message::Note(Action::ToggleTask(first.id, false)));
        send(&mut app, Message::Note(Action::OpenAssignments));
        assert!(render(assignment_dialog(&app), &app.theme(), 700.0).contains("First task"));
        send(&mut app, Message::Note(Action::CloseAssignments));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert_eq!(
            app.document.as_ref().unwrap().note(other.id).unwrap().tasks,
            [first.id]
        );
    }
}
