//! Native render smoke test. Run with `cargo run --example ui_preview -- /tmp/antodo-preview`.
//! Captures actual Iced windows and exits; never touches the user's preferences or task files.
use super::app::{App, Message, Page};
use super::datetime::{DatePicker, DateTarget};
use super::editor::{DateSlot, Editor};
use crate::core::{Document, Item, Moment, Status, Tag};
use chrono::{Duration, Local};
use iced::{Element, Task, Theme, window};
use std::{io::Write, path::PathBuf};

#[derive(Debug, Clone)]
enum PreviewMessage {
    App(Message),
    Redraw,
    Captured(window::Screenshot),
}
struct Preview {
    app: App,
    directory: PathBuf,
    step: usize,
}
impl Preview {
    fn boot() -> (Self, Task<PreviewMessage>) {
        let state = Self::new();
        // Capture without an application message between layout and capture:
        // editor layout replaces an internal Arc, invalidating the previous
        // frame's Weak text primitives even when its contents are unchanged.
        let capture = Task::future(async {
            std::thread::sleep(std::time::Duration::from_millis(
                std::env::var("ANTODO_PREVIEW_DELAY_MS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(1200),
            ));
        })
        .then(|()| window::latest())
        .then(|id| match id {
            Some(id) => window::screenshot(id).map(PreviewMessage::Captured),
            None => iced::exit(),
        });
        let capture = if matches!(state.step, 12 | 20 | 30) {
            let scroll_id = if state.step == 20 {
                "task-table"
            } else if state.step == 30 {
                "task-editor"
            } else {
                "overview"
            };
            Task::future(async {
                std::thread::sleep(std::time::Duration::from_millis(200));
            })
            .then(move |()| iced::widget::operation::snap_to_end(scroll_id))
            .chain(Task::done(PreviewMessage::Redraw))
            .chain(capture)
        } else {
            capture
        };
        (state, capture)
    }
    fn new() -> Self {
        let mut app = App::empty(Default::default());
        app.document = Some(sample().expect("sample document"));
        app.items = app.document.as_ref().unwrap().items().unwrap();
        app.tags = app.document.as_ref().unwrap().tags().unwrap();
        let step: usize = std::env::args()
            .nth(2)
            .and_then(|v| v.parse().ok())
            .filter(|v| *v < 34)
            .unwrap_or(0);
        if step == 14 {
            app.preferences.dark_mode = false;
        }
        if (25..=27).contains(&step) || step >= 30 {
            let mut note = crate::core::Note::new("Release ideas");
            note.content = "# A calmer release\n\nKeep useful ideas near the work.\n\n- Review **priorities** together\n- Capture *small improvements*\n- Share a clear checklist\n\n## Reference\n`antodo` keeps notes in the workspace.".into();
            note.tags = app.tags.iter().take(2).map(|tag| tag.id).collect();
            note.tasks = app.items.iter().take(2).map(|item| item.id).collect();
            app.document.as_mut().unwrap().save_note(note).unwrap();
            app.notes = app.document.as_ref().unwrap().notes().unwrap();
        }
        match step {
            28 | 29 => {
                app.page = Page::Outline;
                let item = app.items.iter().find(|i| !i.parents.is_empty()).unwrap();
                let target = crate::core::actions::TaskOccurrence {
                    item_id: item.id,
                    parent_id: item.parents.first().copied(),
                };
                drop(
                    app.update(Message::Tasks(super::task_interactions::Message::Context(
                        target,
                        iced::Point::new(710.0, 310.0),
                    ))),
                );
                drop(
                    app.update(Message::Tasks(super::task_interactions::Message::Submenu(
                        super::task_interactions::Submenu::Status,
                    ))),
                );
                app.preferences.dark_mode = step != 29;
            }
            30 | 32 => {
                app.page = Page::Tasks;
                app.editor = Some(Editor::with_notes(app.items[0].clone(), &app.notes));
            }
            31 => {
                app.page = Page::Notes;
                app.note_editor = Some(super::notes::Editor::new(app.notes[0].clone()));
                app.llm_config = Some(Default::default());
                app.chat_open = true;
                app.chat_context = Some(crate::core::llm::Context::Note(app.notes[0].id));
                app.chat.log = vec![
                    ("You".into(), "Summarize this note and its linked tasks.".into()),
                    ("Tool".into(), "get_note: read completed".into()),
                    ("Assistant".into(), "The note describes a calmer release: review priorities together, capture small improvements, and share a checklist. It is tagged for focus and personal planning and linked to two tasks.".into()),
                ];
            }
            25 => app.page = Page::Notes,
            26 | 27 => {
                app.page = Page::Notes;
                app.preferences.dark_mode = step != 27;
                app.note_editor = Some(super::notes::Editor::new(app.notes[0].clone()));
            }
            1 => app.preferences.dark_mode = false,
            2 => app.page = Page::Tasks,
            3 => app.page = Page::Outline,
            4 | 15 => app.page = Page::Calendar,
            5 | 10 => {
                let mut editor = Editor::new(
                    app.items
                        .iter()
                        .find(|i| i.name == "Launch a little more calmly")
                        .unwrap()
                        .clone(),
                );
                editor.deadline.value.push_str(" 15:30");
                app.picker = Some(DatePicker::new(
                    DateTarget::Task(DateSlot::Deadline),
                    &editor.deadline,
                ));
                app.editor = Some(editor);
            }
            6 | 7 | 11 | 13 | 14 | 18 => {
                app.page = Page::Tasks;
                app.editor = Some(super::editor::Editor::new(
                    app.items
                        .iter()
                        .find(|i| i.name == "Launch a little more calmly")
                        .unwrap()
                        .clone(),
                ))
            }
            8 => app.page = Page::Tags,
            23 => app.page = Page::Llm,
            24 => {
                app.llm_config = Some(Default::default());
                app.editor = Some(Editor::new(app.items[0].clone()));
                app.chat_open = true;
                app.chat_context = Some(crate::core::llm::Context::Task(app.items[0].id));
                app.chat.log = vec![
                    ("You".into(), "What should I focus on today?".into()),
                    ("Tool".into(), "list_tasks: read completed".into()),
                    ("Assistant".into(), "Bring the story into focus is overdue. Finish the outline first, then keep your protected hour for A little time to think.".into()),
                ];
            }
            21 => app.page = Page::Mcp,
            22 => {
                app.page = Page::Mcp;
                app.mcp_settings.port = "0".into();
                drop(app.update(Message::McpStart));
            }
            16 => {
                app.page = Page::Tags;
                app.color_draft = Some(app.tag_color.clone());
            }
            17 => {
                app.page = Page::Tasks;
                app.show_columns = true;
            }
            9 => app.preferences.font_size = 24,
            _ => {}
        }
        if step == 32 {
            let mut note = crate::core::Note::new("Meeting notes");
            note.tags = app.tags.iter().take(1).map(|tag| tag.id).collect();
            app.document.as_mut().unwrap().save_note(note).unwrap();
            app.notes = app.document.as_ref().unwrap().notes().unwrap();
            drop(app.update(Message::Note(super::notes::Action::OpenAssignments)));
        }
        if step == 33 {
            app.page = Page::Notes;
            app.note_editor = Some(super::notes::Editor::new(app.notes[0].clone()));
            drop(app.update(Message::Note(super::notes::Action::OpenAssignments)));
        }
        if matches!(step, 19 | 20) {
            app.page = Page::Tasks;
            app.items = (0..10_000)
                .map(|i| {
                    let mut item = Item::new(format!("Task {i:05}"));
                    item.description = "Large-list scrolling sample".into();
                    item
                })
                .collect();
        }
        if step == 18 {
            app.editor.as_mut().unwrap().status = Status::Event;
        }
        Self {
            app,
            directory: std::env::args_os()
                .nth(1)
                .map(PathBuf::from)
                .expect("output directory required"),
            step,
        }
    }

    fn view(&self) -> Element<'_, PreviewMessage> {
        self.app.view().map(PreviewMessage::App)
    }
    fn update(&mut self, message: PreviewMessage) -> Task<PreviewMessage> {
        match message {
            PreviewMessage::Redraw => Task::none(),
            PreviewMessage::App(message) => {
                if matches!(message, Message::TableScrolled(_)) {
                    return self.app.update(message).map(PreviewMessage::App);
                }
                Task::none()
            }
            PreviewMessage::Captured(screenshot) => {
                let name = [
                    "overview-dark",
                    "overview-light",
                    "tasks",
                    "hierarchy",
                    "calendar",
                    "date-time-picker",
                    "details",
                    "editor",
                    "tags",
                    "large-font",
                    "date-time-picker-narrow",
                    "details-narrow",
                    "overview-progress",
                    "editor-narrow",
                    "editor-light",
                    "calendar-narrow",
                    "color-picker",
                    "table-columns",
                    "event-editor",
                    "large-table-top",
                    "large-table-bottom",
                    "mcp-stopped",
                    "mcp-running",
                    "llm-setup",
                    "llm-chat",
                    "notes",
                    "note-editor",
                    "note-editor-light",
                    "task-menu",
                    "task-menu-light",
                    "task-notes",
                    "note-chat",
                    "add-note",
                    "add-task",
                ][self.step];
                std::fs::create_dir_all(&self.directory).unwrap();
                let mut out =
                    std::fs::File::create(self.directory.join(format!("{name}.ppm"))).unwrap();
                write!(
                    out,
                    "P6\n{} {}\n255\n",
                    screenshot.size.width, screenshot.size.height
                )
                .unwrap();
                let rgb: Vec<_> = screenshot
                    .rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| p[..3].iter().copied())
                    .collect();
                out.write_all(&rgb).unwrap();
                iced::exit()
            }
        }
    }
}
/// Launch a deterministic screen sequence for visual review on an available desktop.
pub fn run() -> iced::Result {
    iced::application(Preview::boot, Preview::update, Preview::view)
        .title("antodo · visual verification")
        .theme(|state: &Preview| -> Theme { state.app.theme() })
        .scale_factor(|state: &Preview| state.app.scale())
        .window_size(
            if matches!(
                std::env::args().nth(2).as_deref(),
                Some("9" | "10" | "11" | "13" | "15")
            ) {
                (900.0, 680.0)
            } else {
                (1400.0, 1000.0)
            },
        )
        .run()
}
fn sample() -> crate::core::Result<Document> {
    let mut doc = Document::new()?;
    let today = Local::now().date_naive();
    let tz = crate::core::local_timezone();
    let date = |offset: i64, time: &str| {
        Moment::parse(&format!("{}{}", today + Duration::days(offset), time), &tz)
    };
    let work = doc.save_tag(Tag::new("Studio", "#A395FF"))?;
    let personal = doc.save_tag(Tag::new("Personal", "#48CFA9"))?;
    let focus = doc.save_tag(Tag::new("Deep work", "#E9BA75"))?;
    let mut parent = Item::new("Launch a little more calmly");
    parent.description="# Make room for the important work\n\nA small, thoughtful release of our next idea.\n\n## What good looks like\n- A clear story\n- The details feel considered\n- Everyone has room to breathe\n\n**One step at a time.** Keep decisions here, close to the work.\n\n[Project inspiration](https://iced.rs)".into();
    parent.status = Status::Wip;
    parent.tags = vec![work.id, focus.id];
    parent.deadline = Some(date(3, "")?);
    parent.effort = 8;
    parent.participants = vec!["Alex".into(), "sam@example.com".into()];
    parent = doc.save_item(parent)?;
    let mut task = Item::new("Bring the story into focus");
    task.description="Turn the loose notes into a clear outline for the team. Keep it human, specific, and simple.".into();
    task.status = Status::Wip;
    task.parents = vec![parent.id];
    task.tags = vec![work.id];
    task.planned = Some(date(0, " 10:00")?);
    task.deadline = Some(date(-1, "")?);
    task.effort = 3;
    doc.save_item(task)?;
    let mut task = Item::new("A little time to think");
    task.description = "A protected hour for the ideas that need a bit more space.".into();
    task.start = Some(date(0, " 14:00")?);
    task.end = Some(date(0, " 15:00")?);
    task.tags = vec![focus.id, personal.id];
    task.is_recurring = true;
    task.rrule = "FREQ=DAILY;COUNT=5".into();
    doc.save_item(task)?;
    let mut task = Item::new("Share the first sketches");
    task.description = "Collect early reactions and decide what to explore next.".into();
    task.parents = vec![parent.id];
    task.tags = vec![work.id, personal.id];
    task.planned = Some(date(1, " 11:00")?);
    task.effort = 2;
    doc.save_item(task)?;
    let mut task = Item::new("Clear a little space");
    task.description = "Archive old drafts and keep the useful parts.".into();
    task.status = Status::Done;
    task.tags = vec![personal.id];
    task.effort = 2;
    doc.save_item(task)?;
    let mut task = Item::new("A weekend away from screens");
    task.status = Status::Someday;
    task.tags = vec![personal.id];
    doc.save_item(task)?;
    Ok(doc)
}

#[cfg(test)]
pub(super) fn test_app() -> App {
    let mut app = App::empty(Default::default());
    let document = sample().expect("valid preview fixture");
    app.items = document.items().unwrap();
    app.tags = document.tags().unwrap();
    app.document = Some(document);
    app
}
