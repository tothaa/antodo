use super::datetime::{DatePicker, DateTarget, PickerAction};
use super::editor::{DateSlot, Editor, Field, OccurrenceEditor};
use crate::core::preferences::Preferences;
use crate::core::query::{DateField, Filter};
use crate::core::{self, Document, EventOverride, Item, Occurrence, Reminder, Status, Tag};
use chrono::{Datelike, Local, NaiveDate, Utc};
use iced::widget::text_editor;
use iced::{Element, Subscription, Task, Theme, keyboard, window};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Overview,
    Tasks,
    Outline,
    Calendar,
    Notes,
    Tags,
    Mcp,
    Llm,
    About,
}
impl Page {
    pub const ALL: [Self; 9] = [
        Self::Overview,
        Self::Tasks,
        Self::Outline,
        Self::Calendar,
        Self::Notes,
        Self::Tags,
        Self::Mcp,
        Self::Llm,
        Self::About,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Tasks => "All tasks",
            Self::Outline => "Hierarchy",
            Self::Calendar => "Calendar",
            Self::Notes => "Notes",
            Self::Tags => "Tags",
            Self::Mcp => "MCP",
            Self::Llm => "LLM",
            Self::About => "About",
        }
    }
    pub fn subtitle(self) -> &'static str {
        match self {
            Self::Overview => "Make space for what matters today.",
            Self::Tasks => "Every idea, commitment, and next step.",
            Self::Outline => "See how the pieces fit together.",
            Self::Calendar => "Give your intentions a place in time.",
            Self::Notes => "Keep your ideas and reference material close.",
            Self::Tags => "Give your work a little color.",
            Self::Mcp | Self::Llm => "",
            Self::About => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawerTarget {
    Task(Uuid),
    Note(Uuid),
}

#[derive(Debug, Clone)]
pub enum Intent {
    New,
    Open,
    Switch(Page),
    Select(Uuid),
    AddNote,
    AddChild(Uuid),
    SelectNote(Uuid),
    Navigate {
        target: DrawerTarget,
        origin: Option<DrawerTarget>,
    },
    EditTag(Uuid),
    Occurrence(Box<Occurrence>),
    Add,
    CloseDrawer,
    CloseFile,
    Quit,
    AddOn(NaiveDate),
    Close(window::Id),
}
impl Intent {
    pub(super) fn leaves_document(&self) -> bool {
        matches!(
            self,
            Self::New | Self::Open | Self::Close(_) | Self::CloseFile | Self::Quit
        )
    }
}
#[derive(Debug, Clone, Copy)]
pub enum FilterText {
    Participant,
    Priority,
    Severity,
    Effort,
    From,
    To,
    Timezone,
    Rule,
    ReminderMinutes,
}
#[derive(Debug, Clone)]
pub enum Message {
    Noop,
    OpenUrl(&'static str),
    Note(super::notes::Action),
    Tasks(super::task_interactions::Message),
    LlmProvider(core::llm::Provider),
    LlmField(super::llm::Field, String),
    LlmEdits(bool),
    LlmConfigure,
    LlmDisconnect,
    ChatOpen(Option<Uuid>),
    NoteChatOpen(Uuid),
    ChatClose,
    ChatInput(String),
    ChatSend,
    ChatPoll,
    ChatCancel,
    ChatClear,
    McpField(super::mcp::Field, String),
    McpEdits(bool),
    McpStart,
    McpStop,
    McpPoll,
    McpCopyToken,
    McpCopyConfig,
    McpDocs(bool),
    TableScrolled(iced::widget::scrollable::Viewport),
    MoveColumn(super::table::TaskColumn, i8),
    OpenColor,
    ColorDraft(String),
    SelectColor,
    CancelColor,
    PlanToday,
    CalendarTimezone(String),
    ParentPicker(bool),
    ParentSearch(String),
    ParentStatus(Option<Status>),
    ParentTag(Option<Uuid>),
    ParentPriority(String),
    DescriptionHeight(f32),
    Columns,
    ToggleColumn(super::table::TaskColumn, bool),
    SortColumn(super::table::TaskColumn),

    Intent(Intent),
    Continue(bool),
    CancelIntent,
    Opened(Option<PathBuf>),
    Save,
    SaveAs,
    SavePath(Option<PathBuf>),
    Search(String),
    StatusFilter(Option<Status>),
    TagFilter(Option<Uuid>),
    ParentFilter(Option<Uuid>),
    Advanced,
    FilterText(FilterText, String),
    FilterDate(Option<DateField>),
    RecurringFilter(Option<bool>),
    ReminderFilter(Option<bool>),
    RemindedFilter(Option<bool>),
    ClearFilters,
    Theme,
    Font(i16),
    Dismiss,
    Tick,
    Key(keyboard::Key, keyboard::Modifiers),
    EditField(Field, String),
    StepRating(Field, i8),
    EditDate(DateSlot, bool, String),
    Description(text_editor::Action),
    EditStatus(Status),
    OpenPicker(DateTarget),
    DatePicker(PickerAction),
    EditRecurring(bool),
    ToggleTag(Uuid, bool),
    ToggleParent(Uuid, bool),
    Apply,
    DeleteRequest,
    DeleteConfirm,
    DeleteCancel,
    CalendarMove(i32),
    CalendarToday,
    EditOccurrence(Box<Occurrence>),
    OccurrenceDate(bool, bool, String),
    OccurrenceStatus(Status),
    OccurrenceCancel(bool),
    ApplyOccurrence,
    ResetOccurrence,
    TagName(String),
    TagColor(String),
    EditTag(Uuid),
    SaveTag,
    DeleteTag(Uuid),
    DismissReminder(usize),
}

pub struct App {
    drawer_history: Vec<DrawerTarget>,
    pub interactions: super::task_interactions::State,
    pub notes: Vec<core::Note>,
    pub note_search: String,
    pub note_editor: Option<super::notes::Editor>,
    pub calendar_timezone: String,
    pub calendar_timezones: iced::widget::combo_box::State<String>,
    pub color_draft: Option<String>,
    pub assignment_picker: Option<super::notes::AssignmentPicker>,
    pub parent_picker: bool,
    pub parent_filter: Filter,
    pub parent_priority: String,
    pub description_height: f32,
    pub show_columns: bool,
    pub columns: Vec<super::table::TaskColumn>,
    pub sort_column: super::table::TaskColumn,
    pub sort_descending: bool,
    pub mcp_settings: super::mcp::Settings,
    pub mcp_server: Option<core::mcp::Server>,
    pub llm_settings: core::llm::Config,
    pub llm_config: Option<core::llm::Config>,
    pub chat: core::llm::Chat,
    pub chat_open: bool,
    pub chat_context: Option<core::llm::Context>,
    pub chat_input: String,
    pub document: Option<Document>,
    pub dialog_open: bool,
    pub dashboard_cache: std::cell::OnceCell<std::result::Result<core::query::Dashboard, String>>,
    pub table_order: std::cell::OnceCell<Vec<usize>>,
    pub table_offset: f32,
    pub calendar_cache:
        std::cell::OnceCell<std::result::Result<Vec<core::query::CalendarEntry>, String>>,
    pub items: Vec<Item>,
    pub tags: Vec<Tag>,
    pub overrides: Vec<EventOverride>,
    pub preferences: Preferences,
    pub page: Page,
    pub filter: Filter,
    pub advanced: bool,
    pub filter_values: [String; 9],
    pub editor: Option<Editor>,
    pub picker: Option<DatePicker>,
    pub filter_timezones: iced::widget::combo_box::State<String>,
    pub occurrence: Option<OccurrenceEditor>,
    pub pending: Option<Intent>,
    pub continuation: Option<Intent>,
    pub delete_confirm: bool,
    pub error: Option<String>,
    pub notice: Option<String>,
    pub reminders: Vec<Reminder>,
    pub month: NaiveDate,
    pub tag_name: String,
    pub tag_color: String,
    pub tag_id: Option<Uuid>,
    pub tag_dirty: bool,
    #[cfg(test)]
    pub preferences_path: Option<PathBuf>,
}

pub fn run() -> iced::Result {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .scale_factor(App::scale)
        .subscription(App::subscription)
        .window(super::icon::settings())
        .run()
}

impl App {
    pub fn boot() -> Self {
        let (preferences, preference_error) = match Preferences::load() {
            Ok(p) => (p, None),
            Err(e) => (Preferences::default(), Some(e.to_string())),
        };
        let mut app = Self::empty(preferences);
        app.error = preference_error;
        let requested = std::env::args_os()
            .nth(1)
            .map(PathBuf::from)
            .or_else(|| app.preferences.last_file.clone());
        if let Some(path) = requested {
            app.open(path);
        }
        app
    }
    pub fn empty(preferences: Preferences) -> Self {
        Self {
            drawer_history: Vec::new(),
            interactions: Default::default(),
            notes: vec![],
            note_search: String::new(),
            note_editor: None,
            calendar_timezone: core::local_timezone(),
            calendar_timezones: super::datetime::timezones(),
            color_draft: None,
            assignment_picker: None,
            parent_picker: false,
            parent_filter: Filter::default(),
            parent_priority: String::new(),
            description_height: 210.0,
            show_columns: false,
            columns: super::table::TaskColumn::DEFAULT.to_vec(),
            sort_column: super::table::TaskColumn::Name,
            sort_descending: false,
            mcp_settings: Default::default(),
            mcp_server: None,
            llm_settings: Default::default(),
            llm_config: None,
            chat: Default::default(),
            chat_open: false,
            chat_context: None,
            chat_input: String::new(),
            document: None,
            dialog_open: false,
            dashboard_cache: Default::default(),
            table_order: Default::default(),
            table_offset: 0.0,
            calendar_cache: Default::default(),
            items: vec![],
            tags: vec![],
            overrides: vec![],
            preferences,
            page: Page::Overview,
            filter: Filter::default(),
            advanced: false,
            filter_values: Default::default(),
            editor: None,
            picker: None,
            filter_timezones: super::datetime::timezones(),
            occurrence: None,
            pending: None,
            continuation: None,
            delete_confirm: false,
            error: None,
            notice: None,
            reminders: vec![],
            month: Local::now().date_naive().with_day(1).unwrap(),
            tag_name: String::new(),
            tag_color: "#8B7CF8".into(),
            tag_id: None,
            tag_dirty: false,
            #[cfg(test)]
            preferences_path: None,
        }
    }
    pub fn title(&self) -> String {
        format!(
            "{}{} · antodo",
            self.filename(),
            if self.dirty() { " •" } else { "" }
        )
    }
    pub fn filename(&self) -> String {
        self.document
            .as_ref()
            .and_then(|d| d.path())
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                if self.document.is_some() {
                    "Untitled.antodo"
                } else {
                    "Welcome"
                }
                .into()
            })
    }
    pub fn theme(&self) -> Theme {
        let palette = if self.preferences.dark_mode {
            iced::theme::Palette {
                background: iced::Color::from_rgb8(36, 39, 46),
                text: iced::Color::from_rgb8(234, 236, 245),
                primary: iced::Color::from_rgb8(163, 149, 255),
                success: iced::Color::from_rgb8(72, 207, 169),
                warning: iced::Color::from_rgb8(245, 187, 102),
                danger: iced::Color::from_rgb8(255, 119, 137),
            }
        } else {
            iced::theme::Palette {
                background: iced::Color::from_rgb8(210, 212, 216),
                text: iced::Color::from_rgb8(33, 38, 55),
                primary: iced::Color::from_rgb8(103, 79, 214),
                success: iced::Color::from_rgb8(22, 134, 108),
                warning: iced::Color::from_rgb8(165, 104, 20),
                danger: iced::Color::from_rgb8(196, 49, 77),
            }
        };
        Theme::custom("Antodo", palette)
    }
    pub fn scale(&self) -> f32 {
        self.preferences.font_size as f32 / 16.0
    }
    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            super::task_interactions::subscription(),
            if self.chat.busy() {
                iced::time::every(std::time::Duration::from_millis(80)).map(|_| Message::ChatPoll)
            } else {
                Subscription::none()
            },
            if self.mcp_server.is_some() {
                iced::time::every(std::time::Duration::from_millis(100)).map(|_| Message::McpPoll)
            } else {
                Subscription::none()
            },
            iced::time::every(std::time::Duration::from_secs(20)).map(|_| Message::Tick),
            window::close_requests().map(|id| Message::Intent(Intent::Close(id))),
            iced::event::listen_with(|event, status, _| match event {
                iced::Event::Keyboard(keyboard::Event::KeyPressed {
                    key: keyboard::Key::Named(keyboard::key::Named::Escape),
                    ..
                }) if status == iced::event::Status::Captured => None,
                iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) => {
                    Some(Message::Key(key, modifiers))
                }
                _ => None,
            }),
        ])
    }
    fn edit_dirty(&self) -> bool {
        self.editor.as_ref().is_some_and(|e| e.dirty)
            || self.occurrence.as_ref().is_some_and(|e| e.dirty)
            || self.tag_dirty
            || self.note_editor.as_ref().is_some_and(|e| e.dirty)
    }
    pub fn dirty(&self) -> bool {
        self.edit_dirty() || self.document.as_ref().is_some_and(Document::is_dirty)
    }
    fn persist_preferences(&mut self) {
        #[cfg(test)]
        let result = self
            .preferences_path
            .as_ref()
            .map(|p| self.preferences.save_to(p))
            .unwrap_or(Ok(()));
        #[cfg(not(test))]
        let result = self.preferences.save();
        if let Err(e) = result {
            self.error = Some(e.to_string());
        }
    }
    fn refresh(&mut self) -> core::Result<()> {
        if let Some(doc) = &self.document {
            self.items = doc.items()?;
            self.notes = doc.notes()?;
            self.tags = doc.tags()?;
            self.overrides = doc.overrides()?;
            self.interactions.selected.retain(|selected| {
                self.items.iter().any(|item| {
                    item.id == selected.item_id
                        && selected
                            .parent_id
                            .is_none_or(|parent| item.parents.contains(&parent))
                })
            });
        }
        Ok(())
    }
    pub(super) fn changed(&mut self, result: core::Result<()>) {
        self.error = result
            .and_then(|()| self.refresh())
            .err()
            .map(|e| e.to_string());
    }
    pub(super) fn invalidate_views(&mut self) {
        self.dashboard_cache.take();
        self.calendar_cache.take();
        self.table_order.take();
    }
    fn remember_document_path(&mut self) {
        self.preferences.last_file = self
            .document
            .as_ref()
            .and_then(|d| d.path())
            .map(std::path::Path::to_path_buf);
        self.persist_preferences();
    }
    fn clear_tag_editor(&mut self) {
        self.tag_id = None;
        self.tag_name.clear();
        self.tag_dirty = false;
    }
    fn close_drawer(&mut self) {
        self.assignment_picker = None;
        self.note_editor = None;
        self.editor = None;
        self.occurrence = None;
        self.delete_confirm = false;
    }
    fn drawer_target(&self) -> Option<DrawerTarget> {
        self.editor
            .as_ref()
            .map(|editor| DrawerTarget::Task(editor.original.id))
            .or_else(|| {
                self.note_editor
                    .as_ref()
                    .map(|editor| DrawerTarget::Note(editor.original.id))
            })
    }
    fn show_drawer(&mut self, target: DrawerTarget) -> bool {
        match target {
            DrawerTarget::Task(id) => {
                let Some(item) = self.items.iter().find(|item| item.id == id).cloned() else {
                    return false;
                };
                self.close_drawer();
                self.editor = Some(Editor::with_notes(item, &self.notes));
            }
            DrawerTarget::Note(id) => {
                let Some(note) = self.notes.iter().find(|note| note.id == id).cloned() else {
                    return false;
                };
                self.close_drawer();
                self.note_editor = Some(super::notes::Editor::new(note));
            }
        }
        true
    }
    pub(super) fn return_to_drawer(&mut self) {
        self.close_drawer();
        while let Some(target) = self.drawer_history.pop() {
            if self.show_drawer(target) {
                break;
            }
        }
    }
    fn open(&mut self, path: PathBuf) {
        match Document::open(&path) {
            Ok(doc) => {
                self.document = Some(doc);
                self.reset_workspace();
                self.changed(Ok(()));
                self.remember_document_path();
                self.check_reminders();
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn reset_workspace(&mut self) {
        self.drawer_history.clear();
        self.interactions = Default::default();
        self.note_search.clear();
        self.chat = Default::default();
        self.chat_open = false;
        self.chat_context = None;
        self.chat_input.clear();
        self.mcp_server = None;
        self.table_offset = 0.0;
        self.picker = None;
        self.parent_picker = false;
        self.color_draft = None;
        self.close_drawer();
        self.reminders.clear();
        self.page = Page::Overview;
        self.filter = Filter::default();
        self.filter_values = Default::default();
        self.clear_tag_editor();
        self.notice = None;
    }
    fn execute(&mut self, intent: Intent) -> Task<Message> {
        self.interactions.cancel_gesture();
        self.picker = None;
        self.clear_tag_editor();
        if !matches!(
            intent,
            Intent::Navigate { .. }
                | Intent::Open
                | Intent::Select(_)
                | Intent::SelectNote(_)
                | Intent::CloseDrawer
        ) {
            self.drawer_history.clear();
        }
        match intent {
            Intent::New => match Document::new() {
                Ok(doc) => {
                    self.document = Some(doc);
                    self.reset_workspace();
                    self.changed(Ok(()));
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            Intent::Open => {
                self.dialog_open = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .add_filter("Antodo task files", &["antodo"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_owned())
                    },
                    Message::Opened,
                );
            }
            Intent::Switch(page) => {
                if matches!(page, Page::Mcp | Page::Llm) && self.document.is_none() {
                    return Task::none();
                }
                if page != self.page {
                    self.interactions = Default::default();
                    self.table_offset = 0.0;
                }
                self.page = page;
                self.close_drawer();
            }
            Intent::AddNote => {
                self.close_drawer();
                if self.document.is_some() {
                    self.note_editor = Some(super::notes::Editor::new(core::Note::new("")));
                }
            }
            Intent::Navigate { target, origin } => {
                if self.show_drawer(target)
                    && let Some(origin) = origin
                {
                    self.drawer_history.push(origin);
                }
            }
            Intent::SelectNote(id) => {
                self.show_drawer(DrawerTarget::Note(id));
            }
            Intent::Select(id) => {
                self.show_drawer(DrawerTarget::Task(id));
            }
            Intent::EditTag(id) => {
                if let Some(tag) = self.tags.iter().find(|t| t.id == id) {
                    self.tag_id = Some(id);
                    self.tag_name = tag.name.clone();
                    self.tag_color = tag.color.clone();
                    self.close_drawer();
                }
            }
            Intent::Occurrence(occurrence) => {
                self.close_drawer();
                if let Some(item) = self.items.iter().find(|i| i.id == occurrence.item_id) {
                    let mut editor = OccurrenceEditor::new(item.name.clone(), *occurrence);
                    editor.cancelled = self.overrides.iter().any(|o| {
                        o.item_id == editor.occurrence.item_id
                            && o.original_start.utc().ok()
                                == editor.occurrence.original_start.utc().ok()
                            && o.cancelled
                    });
                    self.occurrence = Some(editor);
                    self.editor = None;
                }
            }
            Intent::AddChild(parent) => {
                self.close_drawer();
                if self.document.is_some() {
                    let mut editor = Editor::new(Item::new(""));
                    editor.parents.push(parent);
                    editor.dirty = true;
                    self.editor = Some(editor);
                }
            }
            Intent::Add => {
                if self.document.is_none() {
                    return Task::none();
                }
                self.close_drawer();
                self.editor = Some(Editor::new(Item::new("")));
                self.occurrence = None;
                self.delete_confirm = false;
            }
            Intent::CloseDrawer => self.return_to_drawer(),
            Intent::AddOn(date) => {
                self.close_drawer();
                if self.document.is_some() {
                    let mut editor = Editor::new(Item::new(""));
                    for field in [&mut editor.start, &mut editor.end] {
                        field.value = date.to_string();
                        field.timezone = self.calendar_timezone.clone();
                    }
                    editor.dirty = true;
                    self.editor = Some(editor);
                    self.occurrence = None;
                }
            }
            Intent::CloseFile => {
                self.document = None;
                self.items.clear();
                self.notes.clear();
                self.tags.clear();
                self.overrides.clear();
                self.reset_workspace();
                self.preferences.last_file = None;
                self.persist_preferences();
            }
            Intent::Quit => return iced::exit(),
            Intent::Close(id) => return window::close(id),
        }
        Task::none()
    }
    pub(super) fn request(&mut self, intent: Intent) -> Task<Message> {
        self.interactions.cancel_gesture();
        let target = match intent {
            Intent::Select(id) => Some(DrawerTarget::Task(id)),
            Intent::SelectNote(id) => Some(DrawerTarget::Note(id)),
            _ => None,
        };
        let intent = if let Some(target) = target {
            Intent::Navigate {
                target,
                origin: self.drawer_target().filter(|origin| *origin != target),
            }
        } else {
            intent
        };
        let needs_guard = if intent.leaves_document() {
            self.dirty()
        } else {
            self.edit_dirty()
        };
        if needs_guard {
            self.pending = Some(intent);
            Task::none()
        } else {
            self.execute(intent)
        }
    }
    fn save_tag(&mut self) -> core::Result<()> {
        let tag = Tag {
            id: self.tag_id.unwrap_or_else(Uuid::now_v7),
            name: self.tag_name.clone(),
            color: self.tag_color.clone(),
        };
        self.document
            .as_mut()
            .ok_or_else(|| core::invalid("Open a file first"))?
            .save_tag(tag)?;
        self.clear_tag_editor();
        Ok(())
    }
    fn apply(&mut self) -> core::Result<()> {
        self.apply_note()?;
        if self.tag_dirty {
            self.save_tag()?;
        }

        if let Some(editor) = &self.editor {
            let item = editor.item()?;
            self.document
                .as_mut()
                .ok_or_else(|| core::invalid("Create or open a file first"))?
                .save_item_with_notes(item, editor.notes.clone())?;
            self.editor = None;
        }
        if let Some(editor) = &self.occurrence {
            let value = editor.value(&self.overrides)?;
            self.document
                .as_mut()
                .ok_or_else(|| core::invalid("Open a file first"))?
                .save_override(value)?;
            self.occurrence = None;
        }
        self.refresh()
    }
    fn save(&mut self, save_as: bool) -> Task<Message> {
        self.interactions.cancel_gesture();
        if self.edit_dirty() {
            if let Err(e) = self.apply() {
                self.error = Some(e.to_string());
                self.continuation = None;
                return Task::none();
            }
            self.return_to_drawer();
        }
        let Some(document) = self.document.as_mut() else {
            return Task::none();
        };
        if save_as || document.path().is_none() {
            let filename = self.filename();
            self.dialog_open = true;
            return Task::perform(
                async move {
                    rfd::AsyncFileDialog::new()
                        .set_file_name(filename)
                        .add_filter("Antodo task files", &["antodo"])
                        .save_file()
                        .await
                        .map(|f| f.path().to_owned())
                },
                Message::SavePath,
            );
        }
        let result = document.save();
        self.finish_save(result, false)
    }
    fn finish_save(&mut self, result: core::Result<()>, remember_path: bool) -> Task<Message> {
        match result {
            Ok(()) => {
                self.error = None;
                if remember_path {
                    self.remember_document_path();
                }
                self.notice = Some("All changes saved.".into());
                if let Some(next) = self.continuation.take() {
                    return self.execute(next);
                }
            }
            Err(e) => {
                self.error = Some(e.to_string());
                self.continuation = None;
            }
        }
        Task::none()
    }
    fn check_reminders(&mut self) {
        if self.document.is_none() {
            return;
        }
        match core::query::due_reminders(&self.items, &self.overrides, Utc::now()) {
            Ok(reminders) => {
                for reminder in reminders {
                    let result = self.document.as_mut().unwrap().mark_reminded(&reminder);
                    match result {
                        Ok(()) => self.reminders.push(reminder),
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if let Err(e) = self.refresh() {
                    self.error = Some(e.to_string());
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    fn start_chat(&mut self, context: Option<core::llm::Context>) -> Task<Message> {
        let exists = context.is_none_or(|context| match context {
            core::llm::Context::Task(id) => self.items.iter().any(|item| item.id == id),
            core::llm::Context::Note(id) => self.notes.iter().any(|note| note.id == id),
        });
        if self.llm_config.is_some() && self.document.is_some() && exists {
            self.interactions.cancel_gesture();
            self.chat_open = true;
            self.chat_context = context;
            return iced::widget::operation::snap_to_end("llm-chat-history");
        }
        Task::none()
    }
    pub fn update(&mut self, message: Message) -> Task<Message> {
        if let Message::Tasks(super::task_interactions::Message::Move(point)) = &message {
            self.interactions.track_pointer(*point);
        }
        if matches!(
            message,
            Message::Tasks(super::task_interactions::Message::Cancel)
        ) {
            return super::task_interactions::handle(
                self,
                super::task_interactions::Message::Cancel,
            );
        }
        if let Message::Tasks(super::task_interactions::Message::Modifiers(modifiers)) = message {
            self.interactions.modifiers = modifiers;
            return Task::none();
        }
        if matches!(message, Message::ChatPoll) {
            let previous_log_len = self.chat.log.len();
            let safe = !self.edit_dirty()
                && !self.interactions.busy()
                && self.assignment_picker.is_none()
                && self.pending.is_none()
                && !self.dialog_open
                && self.occurrence.is_none();
            if let Some(doc) = &mut self.document
                && self.chat.poll(doc, safe)
            {
                self.invalidate_views();
                self.changed(Ok(()));
                if let Some(editor) = &self.editor
                    && !editor.dirty
                {
                    self.editor = self
                        .items
                        .iter()
                        .find(|i| i.id == editor.original.id)
                        .cloned()
                        .map(|item| Editor::with_notes(item, &self.notes));
                }
                if let Some(editor) = &self.note_editor
                    && !editor.dirty
                {
                    self.note_editor = self
                        .notes
                        .iter()
                        .find(|note| note.id == editor.original.id)
                        .cloned()
                        .map(super::notes::Editor::new);
                }
                if self.editor.is_none() && self.note_editor.is_none() && self.occurrence.is_none()
                {
                    self.return_to_drawer();
                }
                self.notice = Some("Chat updated the workspace. Save to keep changes.".into());
            }
            return if self.chat.log.len() != previous_log_len || self.chat.error.is_some() {
                iced::widget::operation::snap_to_end("llm-chat-history")
            } else {
                Task::none()
            };
        }
        if self.chat_open {
            match &message {
                Message::Intent(Intent::Close(_) | Intent::Quit) => {
                    self.chat.cancel();
                    self.chat_open = false;
                }
                Message::ChatClose
                | Message::ChatInput(_)
                | Message::ChatSend
                | Message::ChatCancel
                | Message::ChatClear
                | Message::Tick
                | Message::McpPoll => {}
                Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _) => {
                    self.chat.cancel();
                    self.chat_open = false;
                    return Task::none();
                }
                _ => return Task::none(),
            }
        }
        if matches!(message, Message::McpPoll) {
            let edits_clear = !self.interactions.busy()
                && self.note_editor.is_none()
                && self.editor.is_none()
                && self.occurrence.is_none()
                && !self.tag_dirty
                && self.pending.is_none()
                && !self.dialog_open
                && self.picker.is_none()
                && !self.parent_picker
                && self.assignment_picker.is_none()
                && self.color_draft.is_none()
                && !self.chat_open;
            if let (Some(server), Some(document)) = (&self.mcp_server, &mut self.document)
                && server.poll(document, edits_clear)
            {
                self.invalidate_views();
                self.notice = Some("MCP updated the workspace. Save to keep these changes.".into());
                self.changed(Ok(()));
            }
            return Task::none();
        }
        if let Message::TableScrolled(viewport) = &message {
            self.table_offset = viewport.absolute_offset().y;
            return Task::none();
        }
        if matches!(message, Message::Noop) {
            return Task::none();
        }
        if self.color_draft.is_some() {
            match &message {
                Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _)
                | Message::Intent(Intent::Close(_)) => {
                    self.color_draft = None;
                    return Task::none();
                }
                Message::ColorDraft(_)
                | Message::SelectColor
                | Message::CancelColor
                | Message::Tick => {}
                _ => return Task::none(),
            }
        }
        if self.assignment_picker.is_some() {
            use super::notes::Action;
            match &message {
                Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _)
                | Message::Intent(Intent::Close(_)) => {
                    self.assignment_picker = None;
                    return Task::none();
                }
                Message::Note(
                    Action::CloseAssignments
                    | Action::AssignmentSearch(_)
                    | Action::AssignmentTag(_)
                    | Action::AssignmentStatus(_)
                    | Action::AssignmentPriority(_)
                    | Action::Assign(_),
                )
                | Message::Tick => {}
                _ => return Task::none(),
            }
        }
        if self.parent_picker {
            match &message {
                Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _)
                | Message::Intent(Intent::Close(_)) => {
                    self.parent_picker = false;
                    self.interactions.linking_parent = false;
                    return Task::none();
                }
                Message::Tasks(super::task_interactions::Message::LinkParent(_))
                | Message::ParentPicker(_)
                | Message::ParentSearch(_)
                | Message::ParentStatus(_)
                | Message::ParentTag(_)
                | Message::ParentPriority(_)
                | Message::ToggleParent(_, _)
                | Message::Tick => {}
                _ => return Task::none(),
            }
        }
        if self.picker.is_some() {
            match &message {
                Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _)
                | Message::Intent(Intent::Close(_)) => {
                    self.picker = None;
                    return Task::none();
                }
                Message::DatePicker(_) | Message::Tick => {}
                _ => return Task::none(),
            }
        }
        if self.dialog_open
            && !matches!(
                message,
                Message::Opened(_) | Message::SavePath(_) | Message::Tick
            )
        {
            return Task::none();
        }
        if matches!(message, Message::Opened(_) | Message::SavePath(_)) {
            self.dialog_open = false;
        }
        if self.pending.is_some()
            && !matches!(
                &message,
                Message::Continue(_)
                    | Message::CancelIntent
                    | Message::Tick
                    | Message::Key(keyboard::Key::Named(keyboard::key::Named::Escape), _)
            )
        {
            return Task::none();
        }
        if let Message::Tasks(message) = message {
            return super::task_interactions::handle(self, message);
        }
        if matches!(
            message,
            Message::Search(_)
                | Message::StatusFilter(_)
                | Message::TagFilter(_)
                | Message::ParentFilter(_)
                | Message::FilterText(_, _)
                | Message::FilterDate(_)
                | Message::RecurringFilter(_)
                | Message::ReminderFilter(_)
                | Message::RemindedFilter(_)
                | Message::ClearFilters
                | Message::CalendarMove(_)
                | Message::CalendarToday
                | Message::CalendarTimezone(_)
        ) {
            self.interactions.cancel_gesture();
            self.interactions.selected.clear();
        }
        self.invalidate_views();
        match message {
            Message::Note(action) => self.update_note(action),
            Message::Tasks(_) => unreachable!("task interaction handled above"),
            Message::Noop | Message::TableScrolled(_) | Message::McpPoll | Message::ChatPoll => {}
            Message::OpenUrl(url) => {
                let _ = open::that(url);
            }
            Message::LlmProvider(provider) => {
                self.llm_settings.provider = provider;
                self.llm_settings.key.clear();
                use core::llm::Provider;
                let (url, model) = match provider {
                    Provider::Ollama => ("http://127.0.0.1:11434/v1/chat/completions", "qwen3:8b"),
                    Provider::OpenAi => ("https://api.openai.com/v1/chat/completions", "gpt-4.1"),
                    Provider::Gemini => (
                        "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions",
                        "",
                    ),
                    Provider::Codex => ("", ""),
                };
                self.llm_settings.endpoint = url.into();
                self.llm_settings.model = model.into();
            }
            Message::LlmField(field, value) => match field {
                super::llm::Field::Endpoint => self.llm_settings.endpoint = value,
                super::llm::Field::Model => self.llm_settings.model = value,
                super::llm::Field::Key => self.llm_settings.key = value,
            },
            Message::LlmEdits(value) => self.llm_settings.edits = value,
            Message::LlmConfigure => match self.llm_settings.validate() {
                Ok(()) => {
                    self.llm_config = Some(self.llm_settings.clone());
                    self.chat = Default::default();
                    self.error = None;
                    self.notice =
                        Some("LLM configured for this session. Start chat to connect.".into());
                }
                Err(e) => self.error = Some(e.to_string()),
            },
            Message::LlmDisconnect => {
                self.llm_config = None;
                self.llm_settings.key.clear();
                self.chat = Default::default();
            }
            Message::ChatOpen(context) => {
                return self.start_chat(context.map(core::llm::Context::Task));
            }
            Message::NoteChatOpen(id) => {
                return self.start_chat(Some(core::llm::Context::Note(id)));
            }
            Message::ChatClose => {
                self.chat.cancel();
                self.chat_open = false;
            }
            Message::ChatCancel => self.chat.cancel(),
            Message::ChatClear => {
                self.chat = Default::default();
                self.chat_input.clear();
            }
            Message::ChatInput(value) => self.chat_input = value,
            Message::ChatSend => {
                if let Some(config) = self.llm_config.clone() {
                    match self.chat.send_with_context(
                        config,
                        self.chat_input.clone(),
                        self.chat_context,
                    ) {
                        Ok(()) => {
                            self.chat_input.clear();
                            return iced::widget::operation::snap_to_end("llm-chat-history");
                        }
                        Err(e) => self.chat.error = Some(e.to_string()),
                    }
                }
            }
            Message::McpField(field, value) => {
                if self.mcp_server.is_none() {
                    match field {
                        super::mcp::Field::Address => self.mcp_settings.address = value,
                        super::mcp::Field::Port => self.mcp_settings.port = value,
                        super::mcp::Field::Token => self.mcp_settings.token = value,
                    }
                }
            }
            Message::McpEdits(value) => {
                if self.mcp_server.is_none() {
                    self.mcp_settings.allow_edits = value;
                }
            }
            Message::McpStart => {
                if self.document.is_some() && self.mcp_server.is_none() {
                    match self
                        .mcp_settings
                        .config()
                        .and_then(core::mcp::Server::start)
                    {
                        Ok(server) => {
                            self.mcp_server = Some(server);
                            self.error = None;
                        }
                        Err(error) => self.error = Some(error.to_string()),
                    }
                }
            }
            Message::McpStop => self.mcp_server = None,
            Message::McpCopyToken => {
                return iced::clipboard::write(self.mcp_settings.token.clone());
            }
            Message::McpCopyConfig => {
                let url = self.mcp_server.as_ref().map(|s| s.url()).or_else(|| {
                    self.mcp_settings.config().ok().map(|c| {
                        format!(
                            "http://{}/mcp",
                            std::net::SocketAddr::new(c.address, c.port)
                        )
                    })
                });
                if let Some(url) = url {
                    return iced::clipboard::write(super::mcp::snippet(&url));
                }
            }
            Message::McpDocs(tunnel) => {
                let url = if tunnel {
                    "https://developers.openai.com/api/docs/guides/secure-mcp-tunnels"
                } else {
                    "https://learn.chatgpt.com/docs/extend/mcp"
                };
                if let Err(error) = open::that_detached(url) {
                    self.error = Some(error.to_string());
                }
            }
            Message::MoveColumn(column, direction) => {
                if let Some(index) = self.columns.iter().position(|c| *c == column) {
                    let target = index as isize + isize::from(direction);
                    if target >= 0 && (target as usize) < self.columns.len() {
                        self.columns.swap(index, target as usize);
                    }
                }
            }
            Message::OpenColor => self.color_draft = Some(self.tag_color.clone()),
            Message::ColorDraft(value) => {
                if self.color_draft.is_some() {
                    self.color_draft = Some(value);
                }
            }
            Message::CancelColor => self.color_draft = None,
            Message::SelectColor => {
                if let Some(value) = self.color_draft.take() {
                    return self.update(Message::TagColor(value));
                }
            }
            Message::PlanToday => {
                if let Some(editor) = &mut self.editor {
                    if editor.status == Status::Event {
                        return Task::none();
                    }
                    match core::Moment::from_utc(Utc::now(), &editor.planned.timezone, true) {
                        Ok(today) => {
                            editor.planned = super::editor::DateInput::from(&Some(today));
                            editor.dirty = true;
                        }
                        Err(error) => {
                            self.error = Some(error.to_string());
                            return Task::none();
                        }
                    }
                    let result = self.apply();
                    let success = result.is_ok();
                    self.changed(result);
                    if success {
                        self.return_to_drawer();
                    }
                }
            }

            Message::CalendarTimezone(zone) => {
                self.calendar_timezone = zone;
            }
            Message::ParentPicker(open) => {
                self.parent_picker = open;
                if !open {
                    self.interactions.linking_parent = false;
                }
            }
            Message::ParentSearch(value) => self.parent_filter.search = value,
            Message::ParentStatus(value) => self.parent_filter.status = value,
            Message::ParentTag(value) => self.parent_filter.tag = value,
            Message::ParentPriority(value) => {
                self.parent_filter.priority = value.parse().ok();
                self.parent_priority = value;
            }
            Message::DescriptionHeight(height) => self.description_height = height,
            Message::Columns => self.show_columns = !self.show_columns,
            Message::ToggleColumn(column, enabled) => {
                if enabled && !self.columns.contains(&column) {
                    self.columns.push(column);
                } else if !enabled && self.columns.len() > 1 {
                    self.columns.retain(|c| *c != column);
                }
            }
            Message::SortColumn(column) => {
                self.sort_descending = self.sort_column == column && !self.sort_descending;
                self.sort_column = column;
            }

            Message::Intent(intent) => return self.request(intent),
            Message::Continue(save) => {
                if let Some(intent) = self.pending.take() {
                    if save {
                        if let Err(e) = self.apply() {
                            self.error = Some(e.to_string());
                            self.pending = Some(intent);
                            return Task::none();
                        }
                        if intent.leaves_document() {
                            self.continuation = Some(intent);
                            return self.save(false);
                        }
                    }
                    return self.execute(intent);
                }
            }
            Message::CancelIntent => self.pending = None,
            Message::Opened(path) => {
                if let Some(path) = path {
                    self.open(path);
                }
            }
            Message::Save => return self.save(false),
            Message::SaveAs => return self.save(true),
            Message::SavePath(path) => {
                if let Some(path) = path {
                    let result = self
                        .document
                        .as_mut()
                        .ok_or_else(|| core::invalid("No file open"))
                        .and_then(|d| d.save_as(&path));
                    return self.finish_save(result, true);
                } else {
                    self.continuation = None;
                }
            }
            Message::Search(value) => self.filter.search = value,
            Message::StatusFilter(value) => self.filter.status = value,
            Message::TagFilter(value) => self.filter.tag = value,
            Message::ParentFilter(value) => self.filter.parent = value,
            Message::Advanced => self.advanced = !self.advanced,
            Message::FilterDate(value) => {
                self.filter.date_field = value;
                if value.is_none() {
                    self.filter.date_from = None;
                    self.filter.date_to = None;
                    self.filter_values[4].clear();
                    self.filter_values[5].clear();
                }
            }
            Message::RecurringFilter(value) => self.filter.recurring = value,
            Message::ReminderFilter(value) => self.filter.has_reminder = value,
            Message::RemindedFilter(value) => self.filter.reminded = value,
            Message::FilterText(field, value) => {
                let index = field as usize;
                self.filter_values[index] = value.clone();
                let mut candidate = self.filter.clone();
                if matches!(field, FilterText::From | FilterText::To)
                    && !value.trim().is_empty()
                    && candidate.date_field.is_none()
                {
                    candidate.date_field = Some(DateField::Deadline);
                }
                let result: core::Result<()> = (|| {
                    match field {
                        FilterText::Participant => candidate.participant = value,
                        FilterText::Priority => {
                            candidate.priority = parse_optional(&value, "Priority")?
                        }
                        FilterText::Severity => {
                            candidate.severity = parse_optional(&value, "Severity")?
                        }
                        FilterText::Effort => candidate.effort = parse_optional(&value, "Effort")?,
                        FilterText::From => candidate.date_from = parse_date(&value)?,
                        FilterText::To => candidate.date_to = parse_date(&value)?,
                        FilterText::Timezone => candidate.timezone = value,
                        FilterText::Rule => candidate.rule = value,
                        FilterText::ReminderMinutes => {
                            candidate.reminder_minutes = parse_optional(&value, "Reminder minutes")?
                        }
                    };
                    candidate.validate()
                })();
                if let Err(e) = result {
                    self.error = Some(e.to_string());
                } else {
                    self.filter = candidate;
                    self.error = None;
                }
            }
            Message::ClearFilters => {
                self.filter = Filter::default();
                self.filter_values = Default::default();
                self.error = None;
            }
            Message::Theme => {
                self.preferences.dark_mode = !self.preferences.dark_mode;
                self.persist_preferences();
            }
            Message::Font(delta) => {
                self.preferences.font_size =
                    (i32::from(self.preferences.font_size) + i32::from(delta)).clamp(
                        i32::from(core::preferences::MIN_FONT_SIZE),
                        i32::from(core::preferences::MAX_FONT_SIZE),
                    ) as u16;
                self.persist_preferences();
            }
            Message::Dismiss => {
                self.error = None;
                self.notice = None;
            }
            Message::Tick => self.check_reminders(),
            Message::Key(key, modifiers) => {
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    if self.pending.is_some() {
                        self.pending = None;
                    } else if self.interactions.busy() {
                        return super::task_interactions::handle(
                            self,
                            super::task_interactions::Message::Cancel,
                        );
                    } else if self.delete_confirm {
                        self.delete_confirm = false;
                    } else if self.editor.is_some()
                        || self.occurrence.is_some()
                        || self.note_editor.is_some()
                    {
                        return self.request(Intent::CloseDrawer);
                    }
                    return Task::none();
                }
                if modifiers.command() {
                    match key.as_ref() {
                        keyboard::Key::Character(value) if value.eq_ignore_ascii_case("s") => {
                            return self.save(modifiers.shift());
                        }
                        keyboard::Key::Character(value) if value.eq_ignore_ascii_case("o") => {
                            return self.request(Intent::Open);
                        }
                        keyboard::Key::Character(value) if value.eq_ignore_ascii_case("n") => {
                            return self.request(if self.page == Page::Notes {
                                Intent::AddNote
                            } else {
                                Intent::Add
                            });
                        }
                        _ => {}
                    }
                }
            }
            Message::EditField(field, value) => {
                if let Some(e) = self.editor.as_mut() {
                    *e.field_mut(field) = value;
                    e.dirty = true;
                }
            }
            Message::StepRating(field, delta) => {
                if matches!(field, Field::Priority | Field::Severity)
                    && let Some(e) = self.editor.as_mut()
                {
                    let value = e.field_mut(field);
                    if let Ok(current) = value.parse::<u8>() {
                        *value = core::step_rating(current, delta).to_string();
                        e.dirty = true;
                    } else {
                        self.error = Some("Enter a numeric rating before using the arrows".into());
                    }
                }
            }
            Message::EditDate(field, zone, value) => {
                if let Some(e) = self.editor.as_mut() {
                    e.date_mut(field).set(zone, value);
                    e.dirty = true;
                }
            }
            Message::Description(action) => {
                if let Some(e) = self.editor.as_mut() {
                    let edited = action.is_edit();
                    e.dirty |= edited;
                    e.description.perform(action);
                }
            }
            Message::EditStatus(status) => {
                if let Some(e) = self.editor.as_mut() {
                    if status == Status::Done
                        && e.status != Status::Done
                        && e.completion.value.trim().is_empty()
                    {
                        match core::Moment::from_utc(Utc::now(), &e.completion.timezone, false) {
                            Ok(moment) => {
                                e.completion = super::editor::DateInput::from(&Some(moment))
                            }
                            Err(error) => self.error = Some(error.to_string()),
                        }
                    } else if status != Status::Done {
                        e.completion.value.clear();
                    }
                    e.status = status;
                    e.dirty = true;
                }
            }
            Message::OpenPicker(target) => {
                use super::editor::DateInput;
                let date = match target {
                    DateTarget::Task(slot) => {
                        self.editor.as_mut().map(|e| e.date_mut(slot).clone())
                    }
                    DateTarget::Occurrence(end) => self
                        .occurrence
                        .as_ref()
                        .map(|e| if end { e.end.clone() } else { e.start.clone() }),
                    DateTarget::Filter(end) => {
                        let mut date = DateInput::from(&None);
                        date.value = self.filter_values[if end { 5 } else { 4 }].clone();
                        Some(date)
                    }
                };
                if let Some(date) = date {
                    self.picker = Some(DatePicker::new(target, &date));
                }
            }
            Message::DatePicker(action) => match action {
                PickerAction::Cancel => self.picker = None,
                PickerAction::Apply | PickerAction::Clear => {
                    if let Some(picker) = self.picker.as_mut() {
                        if matches!(action, PickerAction::Apply)
                            && let Err(error) = picker.validate()
                        {
                            picker.error = Some(error.to_string());
                            return Task::none();
                        }
                        let target = picker.target;
                        let value = if matches!(action, PickerAction::Clear) {
                            String::new()
                        } else {
                            picker.value()
                        };
                        self.picker = None;
                        return self.update(target.message(false, value));
                    }
                }
                action => {
                    if let Some(picker) = &mut self.picker {
                        picker.update(action);
                    }
                }
            },
            Message::EditRecurring(value) => {
                if let Some(e) = self.editor.as_mut() {
                    e.recurring = value;
                    e.dirty = true;
                }
            }
            Message::ToggleTag(id, value) => {
                if let Some(e) = self.editor.as_mut() {
                    toggle(&mut e.tags, id, value);
                    e.dirty = true;
                }
            }
            Message::ToggleParent(id, value) => {
                if let Some(e) = self.editor.as_mut() {
                    toggle(&mut e.parents, id, value);
                    e.dirty = true;
                }
            }
            Message::Apply | Message::ApplyOccurrence => {
                let result = self.apply();
                let success = result.is_ok();
                self.changed(result);
                if success {
                    self.return_to_drawer();
                }
            }
            Message::DeleteRequest => self.delete_confirm = true,
            Message::DeleteCancel => self.delete_confirm = false,
            Message::DeleteConfirm => {
                if let (Some(doc), Some(editor)) = (&mut self.document, &self.editor) {
                    let result = doc.delete_item(editor.original.id);
                    if result.is_ok() {
                        self.editor = None;
                        self.delete_confirm = false;
                    }
                    let success = result.is_ok();
                    self.changed(result);
                    if success {
                        self.return_to_drawer();
                    }
                }
            }
            Message::CalendarMove(delta) => {
                let total = self.month.year() * 12 + self.month.month0() as i32 + delta;
                if let Some(date) = NaiveDate::from_ymd_opt(
                    total.div_euclid(12),
                    total.rem_euclid(12) as u32 + 1,
                    1,
                ) {
                    self.month = date;
                }
            }
            Message::CalendarToday => {
                if let Ok(today) =
                    core::Moment::from_utc(Utc::now(), &self.calendar_timezone, false)
                        .and_then(|m| m.date())
                {
                    self.month = today.with_day(1).unwrap();
                }
            }
            Message::EditOccurrence(occurrence) => {
                return self.request(Intent::Occurrence(occurrence));
            }
            Message::OccurrenceDate(end, zone, value) => {
                if let Some(e) = self.occurrence.as_mut() {
                    e.date_mut(end).set(zone, value);
                    e.dirty = true;
                }
            }
            Message::OccurrenceStatus(value) => {
                if let Some(e) = self.occurrence.as_mut() {
                    e.status = value;
                    e.dirty = true;
                }
            }
            Message::OccurrenceCancel(value) => {
                if let Some(e) = self.occurrence.as_mut() {
                    e.cancelled = value;
                    e.dirty = true;
                }
            }
            Message::ResetOccurrence => {
                if let Some(editor) = &self.occurrence {
                    let previous = self
                        .overrides
                        .iter()
                        .find(|o| {
                            o.item_id == editor.occurrence.item_id
                                && o.original_start.utc().ok()
                                    == editor.occurrence.original_start.utc().ok()
                        })
                        .map(|o| o.id);
                    if let Some(id) = previous {
                        let result = self.document.as_mut().unwrap().delete_override(id);
                        self.changed(result);
                    }
                    self.occurrence = None;
                }
            }
            Message::TagName(value) => {
                self.tag_name = value;
                self.tag_dirty = true;
            }
            Message::TagColor(value) => {
                self.tag_color = value;
                self.tag_dirty = true;
            }
            Message::EditTag(id) => return self.request(Intent::EditTag(id)),
            Message::SaveTag => {
                if self.document.is_some() {
                    let result = self.save_tag();
                    self.changed(result);
                }
            }
            Message::DeleteTag(id) => {
                if let Some(doc) = &mut self.document {
                    let result = doc.delete_tag(id);
                    if result.is_ok() {
                        self.clear_tag_editor();
                    }
                    self.changed(result);
                }
            }
            Message::DismissReminder(index) => {
                if index < self.reminders.len() {
                    self.reminders.remove(index);
                }
            }
        }
        Task::none()
    }
    pub fn view(&self) -> Element<'_, Message> {
        super::views::view(self)
    }
}

fn toggle(values: &mut Vec<Uuid>, id: Uuid, enabled: bool) {
    values.retain(|v| *v != id);
    if enabled {
        values.push(id);
    }
}
fn parse_optional<T: std::str::FromStr>(value: &str, name: &str) -> core::Result<Option<T>> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        value
            .parse()
            .map(Some)
            .map_err(|_| core::invalid(format!("{name} filter must be a whole number")))
    }
}
fn parse_date(value: &str) -> core::Result<Option<NaiveDate>> {
    if value.trim().is_empty() {
        Ok(None)
    } else {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map(Some)
            .map_err(|_| core::invalid("Filter dates must be YYYY-MM-DD"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Moment, recurrence};
    use chrono::Duration;
    use serde_json::{Value, json};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;
    use std::time::{Duration as Wait, Instant};

    fn send(app: &mut App, message: Message) {
        drop(app.update(message));
    }
    fn app() -> App {
        let mut app = App::empty(Preferences::default());
        send(&mut app, Message::Intent(Intent::New));
        app
    }
    fn add(app: &mut App, name: &str) -> Uuid {
        send(app, Message::Intent(Intent::Add));
        send(app, Message::EditField(Field::Name, name.into()));
        let id = app.editor.as_ref().unwrap().original.id;
        send(app, Message::Apply);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(app.editor.is_none());
        // Tests that edit a saved task explicitly reopen its drawer.
        send(app, Message::Intent(Intent::Select(id)));
        id
    }
    fn moment(value: &str) -> Moment {
        Moment::parse(value, "UTC").unwrap()
    }
    fn read_http_body(stream: &mut TcpStream) -> Value {
        stream.set_read_timeout(Some(Wait::from_secs(5))).unwrap();
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            stream.read_exact(&mut byte).unwrap();
            header.push(byte[0]);
            assert!(header.len() < 16_384);
        }
        let header = String::from_utf8(header).unwrap();
        let length: usize = header
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse().unwrap())
            })
            .unwrap();
        let mut body = vec![0; length];
        stream.read_exact(&mut body).unwrap();
        serde_json::from_slice(&body).unwrap()
    }
    fn poll_until(app: &mut App, message: Message, finished: impl Fn(&App) -> bool) {
        let start = Instant::now();
        while !finished(app) {
            assert!(start.elapsed() < Wait::from_secs(10), "polling timed out");
            send(app, message.clone());
            thread::sleep(Wait::from_millis(5));
        }
    }
    fn mcp_call(app: &mut App, name: &str, arguments: Value) -> Value {
        let address = app.mcp_server.as_ref().unwrap().address;
        let token = app.mcp_settings.token.clone();
        let body = json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":name,"arguments":arguments}}).to_string();
        let client = thread::spawn(move || {
            let mut stream = TcpStream::connect_timeout(&address, Wait::from_secs(2)).unwrap();
            write!(stream, "POST /mcp HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
            read_http_body(&mut stream)
        });
        poll_until(app, Message::McpPoll, |_| client.is_finished());
        client.join().unwrap()
    }
    fn chat_provider(replies: Vec<Value>) -> (core::llm::Config, thread::JoinHandle<Vec<Value>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let config = core::llm::Config {
            endpoint: format!(
                "http://{}/v1/chat/completions",
                listener.local_addr().unwrap()
            ),
            edits: true,
            ..Default::default()
        };
        let worker = thread::spawn(move || {
            replies.into_iter().map(|reply| {
                let start = Instant::now();
                let mut stream = loop {
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            assert!(start.elapsed() < Wait::from_secs(10), "provider not contacted");
                            thread::sleep(Wait::from_millis(5));
                        }
                        Err(error) => panic!("{error}"),
                    }
                };
                let request = read_http_body(&mut stream);
                let body = json!({"choices":[{"message":reply}]}).to_string();
                write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
                request
            }).collect()
        });
        (config, worker)
    }
    fn chat_tool(name: &str, arguments: Value) -> Value {
        json!({"role":"assistant","content":null,"tool_calls":[{"id":"change","type":"function","function":{"name":name,"arguments":arguments.to_string()}}]})
    }
    fn cache_views(app: &App) {
        app.dashboard_cache.set(Ok(Default::default())).unwrap();
        app.calendar_cache.set(Ok(vec![])).unwrap();
        app.table_order.set(vec![0]).unwrap();
    }
    fn recurring(app: &mut App) -> Occurrence {
        let mut item = Item::new("Weekly planning");
        item.start = Some(moment("2030-01-07 09:00"));
        item.end = Some(moment("2030-01-07 10:00"));
        item.is_recurring = true;
        item.rrule = "FREQ=WEEKLY;COUNT=3".into();
        let item = app.document.as_mut().unwrap().save_item(item).unwrap();
        app.refresh().unwrap();
        recurrence::expand(
            &item,
            &[],
            moment("2030-01-01").utc().unwrap(),
            moment("2030-02-01").utc().unwrap(),
        )
        .unwrap()
        .remove(0)
    }

    #[test]
    fn new_add_apply_cancel_and_delete_follow_document_lifecycle() {
        let mut app = app();
        assert_eq!(app.filename(), "Untitled.antodo");
        assert_eq!(app.title(), "Untitled.antodo · antodo");
        assert!(!app.dirty());
        let id = add(&mut app, "First task");
        assert_eq!(app.items.len(), 1);
        assert!(app.dirty());
        assert_eq!(app.title(), "Untitled.antodo • · antodo");
        assert!(app.editor.is_some());
        send(
            &mut app,
            Message::EditField(Field::Name, "Discard this edit".into()),
        );
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        send(&mut app, Message::Intent(Intent::Select(id)));
        assert_eq!(app.editor.as_ref().unwrap().name, "First task");
        assert_eq!(app.items[0].name, "First task");
        send(&mut app, Message::DeleteRequest);
        assert!(app.delete_confirm);
        send(&mut app, Message::DeleteCancel);
        assert!(!app.delete_confirm);
        assert_eq!(app.items[0].id, id);
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::DeleteConfirm);
        assert!(app.items.is_empty());
        assert!(app.editor.is_none());
        send(&mut app, Message::Intent(Intent::Add));
        send(
            &mut app,
            Message::EditField(Field::Name, "Unsaved new task".into()),
        );
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        assert!(app.items.is_empty());
    }

    #[test]
    fn form_validation_keeps_input_and_database_unchanged() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::Add));
        send(&mut app, Message::Apply);
        assert!(app.error.as_deref().unwrap().contains("name"));
        assert!(app.items.is_empty());
        send(
            &mut app,
            Message::EditField(Field::Name, "Validated task".into()),
        );
        send(
            &mut app,
            Message::EditDate(DateSlot::Start, false, "2030-01-01".into()),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.as_deref().unwrap().contains("both"));
        assert_eq!(app.editor.as_ref().unwrap().name, "Validated task");
        send(
            &mut app,
            Message::EditDate(DateSlot::End, false, "2030-01-02".into()),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_none());
        assert_eq!(app.items.len(), 1);
    }

    #[test]
    fn dirty_navigation_can_cancel_apply_or_discard() {
        let mut app = app();
        add(&mut app, "Original");
        send(&mut app, Message::EditField(Field::Name, "Changed".into()));
        send(&mut app, Message::Intent(Intent::Switch(Page::Calendar)));
        assert!(matches!(app.pending, Some(Intent::Switch(Page::Calendar))));
        assert_eq!(app.page, Page::Overview);
        send(&mut app, Message::CancelIntent);
        assert!(app.pending.is_none());
        assert_eq!(app.editor.as_ref().unwrap().name, "Changed");
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        send(&mut app, Message::Continue(true));
        assert_eq!(app.page, Page::Tasks);
        assert_eq!(app.items[0].name, "Changed");
        let id = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Name, "Discard".into()));
        send(&mut app, Message::Intent(Intent::Switch(Page::Outline)));
        send(&mut app, Message::Continue(false));
        assert_eq!(app.page, Page::Outline);
        assert_eq!(app.items[0].name, "Changed");
    }

    #[test]
    fn validation_failure_keeps_navigation_confirmation_pending() {
        let mut app = app();
        add(&mut app, "Keep this");
        send(&mut app, Message::EditField(Field::Name, "".into()));
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Continue(true));
        assert!(app.error.is_some());
        assert!(matches!(app.pending, Some(Intent::New)));
        assert_eq!(app.items[0].name, "Keep this");
        assert!(app.continuation.is_none());
    }

    #[test]
    fn save_as_and_open_use_temporary_files_and_remember_canonical_path() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tasks.antodo");
        let preferences = folder.path().join("settings/preferences.json");
        let mut app = app();
        app.preferences_path = Some(preferences.clone());
        add(&mut app, "Saved task");
        send(&mut app, Message::SavePath(Some(path.clone())));
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(!app.dirty());
        assert_eq!(
            app.preferences.last_file,
            Some(path.canonicalize().unwrap())
        );
        assert_eq!(
            Preferences::load_from(&preferences).unwrap().last_file,
            app.preferences.last_file
        );
        send(&mut app, Message::Intent(Intent::New));
        assert!(app.items.is_empty());
        send(&mut app, Message::Opened(Some(path.clone())));
        assert_eq!(app.items[0].name, "Saved task");
        let id = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Name, "Updated".into()));
        send(&mut app, Message::Save);
        assert!(!app.dirty());
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Updated"
        );
    }

    #[test]
    fn save_before_new_continues_only_after_successful_save() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tasks.antodo");
        let mut app = app();
        add(&mut app, "Keep on disk");
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Continue(true));
        assert!(matches!(app.continuation, Some(Intent::New)));
        send(&mut app, Message::SavePath(None));
        assert!(app.continuation.is_none());
        assert_eq!(app.items[0].name, "Keep on disk");
        assert!(app.dirty());
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Continue(true));
        send(&mut app, Message::SavePath(Some(path.clone())));
        assert!(app.items.is_empty());
        assert!(!app.dirty());
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Keep on disk"
        );
    }

    #[test]
    fn failed_open_and_save_preserve_current_document() {
        let folder = tempfile::tempdir().unwrap();
        let mut app = app();
        let id = add(&mut app, "Keep me");
        send(
            &mut app,
            Message::Opened(Some(folder.path().join("missing.antodo"))),
        );
        assert!(app.error.is_some());
        assert_eq!(app.items[0].id, id);
        send(
            &mut app,
            Message::SavePath(Some(folder.path().join("wrong.txt"))),
        );
        assert!(app.error.is_some());
        assert!(app.dirty());
        assert!(!folder.path().join("wrong.txt").exists());
    }

    #[test]
    fn failed_save_before_new_cancels_continuation_and_retains_work() {
        for existing_file in [false, true] {
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("tasks.antodo");
            let unrelated = folder.path().join("notes.txt");
            std::fs::write(&unrelated, "Keep this file").unwrap();
            let mut app = app();
            add(&mut app, "Original");
            if existing_file {
                send(&mut app, Message::SavePath(Some(path.clone())));
                let mut other = Document::open(&path).unwrap();
                other.save_item(Item::new("External change")).unwrap();
                other.save().unwrap();
            }
            send(
                &mut app,
                Message::EditField(Field::Name, "Keep edits".into()),
            );
            send(&mut app, Message::Intent(Intent::New));
            send(&mut app, Message::Continue(true));
            if !existing_file {
                assert!(matches!(app.continuation, Some(Intent::New)));
                send(&mut app, Message::SavePath(Some(unrelated.clone())));
            }
            assert!(app.error.is_some());
            assert!(app.continuation.is_none());
            assert!(!app.dialog_open);
            assert_eq!(app.items.len(), 1);
            assert_eq!(app.items[0].name, "Keep edits");
            assert!(app.dirty());
            assert_eq!(
                std::fs::read_to_string(&unrelated).unwrap(),
                "Keep this file"
            );
            if existing_file {
                assert_eq!(Document::open(&path).unwrap().items().unwrap().len(), 2);
            }
        }
    }

    #[test]
    fn occurrence_edits_are_guarded_saved_and_reset() {
        let mut app = app();
        let occurrence = recurring(&mut app);
        add(&mut app, "Editing another task");
        send(
            &mut app,
            Message::EditField(Field::Name, "Unsaved name".into()),
        );
        send(
            &mut app,
            Message::EditOccurrence(Box::new(occurrence.clone())),
        );
        assert!(app.pending.is_some());
        assert!(app.occurrence.is_none());
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        send(
            &mut app,
            Message::OccurrenceDate(false, false, "2030-01-08 09:00".into()),
        );
        send(
            &mut app,
            Message::OccurrenceDate(true, false, "2030-01-08 10:00".into()),
        );
        send(&mut app, Message::OccurrenceStatus(Status::Wip));
        send(&mut app, Message::ApplyOccurrence);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert_eq!(app.overrides.len(), 1);
        assert_eq!(app.overrides[0].status, Status::Wip);
        assert_eq!(
            app.overrides[0].new_start.as_ref().unwrap().value,
            "2030-01-08 09:00"
        );
        send(&mut app, Message::EditOccurrence(Box::new(occurrence)));
        send(&mut app, Message::ResetOccurrence);
        assert!(app.overrides.is_empty());
        assert!(app.occurrence.is_none());
    }

    #[test]
    fn occurrence_validation_does_not_discard_editor() {
        let mut app = app();
        let occurrence = recurring(&mut app);
        send(&mut app, Message::EditOccurrence(Box::new(occurrence)));
        send(
            &mut app,
            Message::OccurrenceDate(true, false, "2029-01-01".into()),
        );
        send(&mut app, Message::ApplyOccurrence);
        assert!(app.error.is_some());
        assert!(app.occurrence.as_ref().unwrap().dirty);
        assert!(app.overrides.is_empty());
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert!(app.occurrence.is_none());
    }

    #[test]
    fn search_and_property_filters_compose_and_reset() {
        let mut app = app();
        let mut item = Item::new("Publish");
        item.description = "Release notes and documentation".into();
        item.priority = 2;
        item.severity = 4;
        item.effort = 8;
        item.participants = vec!["Alice@example.com".into()];
        item.planned = Some(moment("2030-01-10"));
        item.status = Status::Wip;
        app.document.as_mut().unwrap().save_item(item).unwrap();
        app.document
            .as_mut()
            .unwrap()
            .save_item(Item::new("Unrelated"))
            .unwrap();
        app.refresh().unwrap();
        send(&mut app, Message::Search("DOCUMENTATION".into()));
        send(&mut app, Message::StatusFilter(Some(Status::Wip)));
        send(
            &mut app,
            Message::FilterText(FilterText::Participant, "alice".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::Priority, "2".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::Severity, "4".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::Effort, "8".into()),
        );
        send(&mut app, Message::FilterDate(Some(DateField::Planned)));
        send(
            &mut app,
            Message::FilterText(FilterText::From, "2030-01-01".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::To, "2030-01-31".into()),
        );
        send(&mut app, Message::RecurringFilter(Some(false)));
        assert_eq!(core::query::filtered(&app.items, &app.filter).len(), 1);
        send(&mut app, Message::ClearFilters);
        assert_eq!(core::query::filtered(&app.items, &app.filter).len(), 2);
        assert!(app.filter_values.iter().all(String::is_empty));
    }

    #[test]
    fn invalid_filter_input_keeps_last_valid_filter() {
        let mut app = app();
        send(
            &mut app,
            Message::FilterText(FilterText::Priority, "3".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::Priority, "nine".into()),
        );
        assert!(app.error.is_some());
        assert_eq!(app.filter.priority, Some(3));
        send(
            &mut app,
            Message::FilterText(FilterText::Priority, "9".into()),
        );
        assert!(app.error.is_some());
        assert_eq!(app.filter.priority, Some(3));
        send(
            &mut app,
            Message::FilterText(FilterText::To, "2030-01-10".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::From, "2030-01-20".into()),
        );
        assert!(app.error.is_some());
        assert!(app.filter.date_from.is_none());
        send(
            &mut app,
            Message::FilterText(FilterText::From, "2030-01-01".into()),
        );
        assert!(app.error.is_none());
    }

    #[test]
    fn tag_edits_are_guarded_and_can_apply_before_navigation() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::Switch(Page::Tags)));
        send(&mut app, Message::TagName("Focus".into()));
        send(&mut app, Message::TagColor("#123456".into()));
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        assert!(app.pending.is_some());
        send(&mut app, Message::Continue(true));
        assert_eq!(app.tags.len(), 1);
        assert_eq!(app.tags[0].name, "Focus");
        assert_eq!(app.page, Page::Tasks);
        assert!(!app.tag_dirty);
        let id = app.tags[0].id;
        send(&mut app, Message::EditTag(id));
        send(&mut app, Message::TagColor("bad".into()));
        send(&mut app, Message::SaveTag);
        assert!(app.error.is_some());
        assert_eq!(app.tags[0].color, "#123456");
        send(&mut app, Message::Intent(Intent::Switch(Page::Tags)));
        send(&mut app, Message::Continue(false));
        assert!(!app.tag_dirty);
        send(&mut app, Message::DeleteTag(id));
        assert!(app.tags.is_empty());
    }

    #[test]
    fn theme_and_font_changes_persist_only_to_test_preferences() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("preferences.json");
        let mut app = app();
        app.preferences_path = Some(path.clone());
        let dark = app.preferences.dark_mode;
        send(&mut app, Message::Theme);
        send(&mut app, Message::Font(2));
        let saved = Preferences::load_from(&path).unwrap();
        assert_eq!(saved.dark_mode, !dark);
        assert_eq!(saved.font_size, 18);
        assert_eq!(app.scale(), 18.0 / 16.0);
        send(&mut app, Message::Font(-100));
        assert_eq!(app.preferences.font_size, core::preferences::MIN_FONT_SIZE);
    }

    #[test]
    fn reminder_ticks_are_acknowledged_once_and_can_be_dismissed() {
        let mut app = app();
        let mut item = Item::new("Upcoming call");
        item.planned =
            Some(Moment::from_utc(Utc::now() + Duration::minutes(5), "UTC", false).unwrap());
        item.reminder = Some(10);
        app.document.as_mut().unwrap().save_item(item).unwrap();
        app.refresh().unwrap();
        send(&mut app, Message::Tick);
        assert_eq!(app.reminders.len(), 1);
        assert!(app.items[0].reminded);
        send(&mut app, Message::Tick);
        assert_eq!(app.reminders.len(), 1);
        send(&mut app, Message::DismissReminder(0));
        assert!(app.reminders.is_empty());
        send(&mut app, Message::Tick);
        assert!(app.reminders.is_empty());
    }

    #[test]
    fn switching_tags_guards_unsaved_color_and_delete_clears_draft() {
        let mut app = app();
        let first = app
            .document
            .as_mut()
            .unwrap()
            .save_tag(Tag::new("First", "#123456"))
            .unwrap();
        let second = app
            .document
            .as_mut()
            .unwrap()
            .save_tag(Tag::new("Second", "#654321"))
            .unwrap();
        app.refresh().unwrap();
        send(&mut app, Message::EditTag(first.id));
        send(&mut app, Message::TagColor("#ABCDEF".into()));
        send(&mut app, Message::EditTag(second.id));
        assert!(matches!(app.pending, Some(Intent::EditTag(id)) if id == second.id));
        assert_eq!(app.tag_id, Some(first.id));
        send(&mut app, Message::CancelIntent);
        assert_eq!(app.tag_color, "#ABCDEF");
        send(&mut app, Message::EditTag(second.id));
        send(&mut app, Message::Continue(true));
        assert_eq!(app.tag_id, Some(second.id));
        assert_eq!(
            app.tags
                .iter()
                .find(|tag| tag.id == first.id)
                .unwrap()
                .color,
            "#ABCDEF"
        );
        send(&mut app, Message::TagName("Unapplied rename".into()));
        send(&mut app, Message::DeleteTag(second.id));
        assert!(!app.tag_dirty);
        assert!(app.tag_id.is_none());
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        assert!(app.pending.is_none());
        assert!(app.error.is_none());
    }

    #[test]
    fn keyboard_save_applies_changes_and_cancelled_save_as_keeps_them() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tasks.antodo");
        let mut app = app();
        add(&mut app, "Original");
        send(&mut app, Message::SavePath(Some(path.clone())));
        send(
            &mut app,
            Message::EditField(Field::Name, "Saved by shortcut".into()),
        );
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Character("s".into()),
                keyboard::Modifiers::COMMAND,
            ),
        );
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Saved by shortcut"
        );
        assert!(!app.dirty());
        let id = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(
            &mut app,
            Message::EditField(Field::Name, "Save As draft".into()),
        );
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Character("S".into()),
                keyboard::Modifiers::COMMAND | keyboard::Modifiers::SHIFT,
            ),
        );
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Saved by shortcut"
        );
        send(&mut app, Message::SavePath(None));
        assert!(app.dirty());
        assert_eq!(app.items[0].name, "Save As draft");
        assert_eq!(app.document.as_ref().unwrap().path(), Some(path.as_path()));
        send(&mut app, Message::SaveAs);
        send(&mut app, Message::SavePath(None));
        assert!(app.dirty());
        send(&mut app, Message::Save);
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Save As draft"
        );
    }

    #[test]
    fn complete_task_form_preserves_markdown_dates_relations_and_numeric_fields() {
        let mut app = app();
        let parent = add(&mut app, "Project");
        send(&mut app, Message::TagName("Planning".into()));
        send(&mut app, Message::SaveTag);
        let tag = app.tags[0].id;
        send(&mut app, Message::Intent(Intent::Add));
        send(
            &mut app,
            Message::EditField(Field::Name, "Detailed recurring task".into()),
        );
        let description = "# Review\n\n- Full **Markdown** description\n- Unicode: árvíz";
        send(
            &mut app,
            Message::Description(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(description.into()),
            ))),
        );
        for (field, value) in [
            (Field::Participants, "Alice, bob@example.com"),
            (Field::Priority, "1"),
            (Field::Severity, "5"),
            (Field::Effort, "13"),
            (Field::Reminder, "30"),
            (Field::Rule, "FREQ=DAILY"),
        ] {
            send(&mut app, Message::EditField(field, value.into()));
        }
        for (field, value) in [
            (DateSlot::Start, "2030-01-07 09:00"),
            (DateSlot::End, "2030-01-07 10:00"),
            (DateSlot::Deadline, "2030-01-31"),
            (DateSlot::Planned, "2030-01-06 12:00"),
            (DateSlot::Completion, "2029-12-30"),
        ] {
            send(&mut app, Message::EditDate(field, true, "UTC".into()));
            send(&mut app, Message::EditDate(field, false, value.into()));
        }
        send(&mut app, Message::EditStatus(Status::Wip));
        send(&mut app, Message::EditRecurring(true));
        send(&mut app, Message::ToggleParent(parent, true));
        send(&mut app, Message::ToggleParent(parent, true));
        send(&mut app, Message::ToggleTag(tag, true));
        send(&mut app, Message::ToggleTag(tag, false));
        send(&mut app, Message::ToggleTag(tag, true));
        send(&mut app, Message::Apply);
        assert!(app.error.as_deref().unwrap().contains("ending"));
        assert_eq!(app.items.len(), 1);
        send(
            &mut app,
            Message::EditField(Field::Rule, "FREQ=DAILY;COUNT=3".into()),
        );
        send(&mut app, Message::Apply);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(app.editor.is_none());
        let saved = app.items.iter().find(|i| i.id != parent).unwrap();
        assert_eq!(saved.description, description);
        assert_eq!(saved.parents, vec![parent]);
        assert_eq!(saved.tags, vec![tag]);
        assert_eq!(saved.participants, vec!["Alice", "bob@example.com"]);
        assert_eq!(
            (saved.priority, saved.severity, saved.effort, saved.reminder),
            (1, 5, 13, Some(30))
        );
        assert_eq!(saved.deadline.as_ref().unwrap().value, "2030-01-31");
        assert_eq!(saved.planned.as_ref().unwrap().timezone, "UTC");
        assert!(saved.completion.is_none());
        let id = saved.id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditStatus(Status::Done));
        send(
            &mut app,
            Message::EditDate(DateSlot::Completion, false, "2029-12-30".into()),
        );
        send(&mut app, Message::Apply);
        assert_eq!(
            app.items
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .completion
                .as_ref()
                .unwrap()
                .value,
            "2029-12-30"
        );
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditStatus(Status::OnHold));
        send(&mut app, Message::Apply);
        assert!(
            app.items
                .iter()
                .find(|i| i.id == id)
                .unwrap()
                .completion
                .is_none()
        );
    }

    #[test]
    fn occurrence_can_be_cancelled_restored_and_retimed_in_another_zone() {
        let mut app = app();
        let original = recurring(&mut app);
        send(
            &mut app,
            Message::EditOccurrence(Box::new(original.clone())),
        );
        send(&mut app, Message::OccurrenceCancel(true));
        send(&mut app, Message::ApplyOccurrence);
        assert!(app.overrides[0].cancelled);
        send(
            &mut app,
            Message::EditOccurrence(Box::new(original.clone())),
        );
        assert!(app.occurrence.as_ref().unwrap().cancelled);
        send(&mut app, Message::OccurrenceCancel(false));
        send(
            &mut app,
            Message::OccurrenceDate(false, true, "Europe/Budapest".into()),
        );
        send(
            &mut app,
            Message::OccurrenceDate(true, true, "Europe/Budapest".into()),
        );
        send(
            &mut app,
            Message::OccurrenceDate(false, false, "2030-01-08 15:00".into()),
        );
        send(
            &mut app,
            Message::OccurrenceDate(true, false, "2030-01-08 16:00".into()),
        );
        send(&mut app, Message::ApplyOccurrence);
        assert!(!app.overrides[0].cancelled);
        assert_eq!(
            app.overrides[0].new_start.as_ref().unwrap().timezone,
            "Europe/Budapest"
        );
        let item = app
            .items
            .iter()
            .find(|item| item.id == original.item_id)
            .unwrap();
        let expanded = recurrence::expand(
            item,
            &app.overrides,
            moment("2030-01-08").utc().unwrap(),
            moment("2030-01-09").utc().unwrap(),
        )
        .unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(
            expanded[0].start.utc().unwrap(),
            moment("2030-01-08 14:00").utc().unwrap()
        );
    }

    #[test]
    fn open_cancel_and_close_confirmation_preserve_dirty_work() {
        let mut app = app();
        let id = add(&mut app, "Unsaved");
        send(&mut app, Message::Intent(Intent::Open));
        assert!(matches!(app.pending, Some(Intent::Open)));
        send(&mut app, Message::Continue(false));
        send(&mut app, Message::Opened(None));
        assert_eq!(app.items[0].id, id);
        assert!(app.dirty());
        send(
            &mut app,
            Message::Intent(Intent::Close(window::Id::unique())),
        );
        assert!(matches!(app.pending, Some(Intent::Close(_))));
        send(&mut app, Message::CancelIntent);
        assert_eq!(app.items[0].id, id);
        send(&mut app, Message::CalendarMove(-1));
        send(&mut app, Message::CalendarMove(1));
        send(&mut app, Message::CalendarToday);
        assert_eq!(app.month, Local::now().date_naive().with_day(1).unwrap());
    }

    #[test]
    fn preference_limits_clamp_without_overflow_and_errors_are_visible() {
        let folder = tempfile::tempdir().unwrap();
        let mut app = app();
        app.preferences_path = Some(folder.path().join("preferences.json"));
        send(&mut app, Message::Font(i16::MAX));
        assert_eq!(app.preferences.font_size, core::preferences::MAX_FONT_SIZE);
        send(&mut app, Message::Font(i16::MIN));
        assert_eq!(app.preferences.font_size, core::preferences::MIN_FONT_SIZE);
        app.preferences_path = Some(folder.path().to_owned());
        send(&mut app, Message::Theme);
        assert!(app.error.is_some());
        send(&mut app, Message::Dismiss);
        assert!(app.error.is_none());
    }
    #[test]
    fn file_dialog_guards_document_until_result_and_caches_invalidate() {
        let mut app = app();
        let id = add(&mut app, "Keep while choosing");
        send(&mut app, Message::SaveAs);
        assert!(app.dialog_open);
        send(&mut app, Message::Intent(Intent::New));
        assert_eq!(app.items[0].id, id);
        send(&mut app, Message::SavePath(None));
        assert!(!app.dialog_open);
        app.dashboard_cache.set(Ok(Default::default())).unwrap();
        app.calendar_cache.set(Ok(vec![])).unwrap();
        send(&mut app, Message::Search("Keep".into()));
        assert!(app.dashboard_cache.get().is_none());
        assert!(app.calendar_cache.get().is_none());
    }
    #[test]
    fn additional_filters_apply_and_clearing_date_field_clears_range() {
        let mut app = app();
        send(
            &mut app,
            Message::FilterText(FilterText::From, "2026-11-01".into()),
        );
        assert_eq!(app.filter.date_field, Some(DateField::Deadline));
        send(&mut app, Message::FilterDate(None));
        assert!(app.filter.date_from.is_none());
        assert!(app.filter_values[4].is_empty());
        send(
            &mut app,
            Message::FilterText(FilterText::Timezone, "Europe/Budapest".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::Rule, "WEEKLY".into()),
        );
        send(
            &mut app,
            Message::FilterText(FilterText::ReminderMinutes, "15".into()),
        );
        assert_eq!(app.filter.timezone, "Europe/Budapest");
        assert_eq!(app.filter.rule, "WEEKLY");
        assert_eq!(app.filter.reminder_minutes, Some(15));
        send(&mut app, Message::ClearFilters);
        assert!(app.filter.timezone.is_empty());
        assert!(app.filter.rule.is_empty());
        assert_eq!(app.filter.reminder_minutes, None);
    }

    #[test]
    fn status_buttons_stage_and_apply_completion_in_editor() {
        let mut app = app();
        let id = add(&mut app, "Quick action");
        for status in [
            Status::Wip,
            Status::OnHold,
            Status::Someday,
            Status::WontDo,
            Status::Pending,
            Status::Done,
            Status::Pending,
        ] {
            send(&mut app, Message::EditStatus(status));
            send(&mut app, Message::Apply);
            assert!(app.error.is_none(), "{:?}", app.error);
            let saved = app.items.iter().find(|i| i.id == id).unwrap();
            assert_eq!(saved.status, status);
            assert_eq!(saved.completion.is_some(), status == Status::Done);
            assert!(app.editor.is_none());
            send(&mut app, Message::Intent(Intent::Select(id)));
            assert!(app.dirty());
        }
        send(
            &mut app,
            Message::EditField(Field::Name, "Keep draft".into()),
        );
        send(&mut app, Message::EditStatus(Status::Done));
        assert_eq!(app.editor.as_ref().unwrap().name, "Keep draft");
        assert_eq!(app.items[0].status, Status::Pending);
        assert_eq!(app.editor.as_ref().unwrap().status, Status::Done);
    }

    #[test]
    fn status_buttons_reject_invalid_relationship_transitions() {
        let mut app = app();
        let parent = add(&mut app, "Parent");
        let mut child = Item::new("Child");
        child.parents.push(parent);
        let child = app.document.as_mut().unwrap().save_item(child).unwrap();
        app.refresh().unwrap();
        let before = app.items.clone();
        send(&mut app, Message::EditStatus(Status::Done));
        send(&mut app, Message::Apply);
        assert!(app.error.is_some());
        assert_eq!(app.items, before);
        assert_eq!(app.editor.as_ref().unwrap().status, Status::Done);
        send(&mut app, Message::EditStatus(Status::Pending));
        send(&mut app, Message::Apply);
        send(&mut app, Message::Intent(Intent::Select(child.id)));
        send(&mut app, Message::EditStatus(Status::WontDo));
        send(&mut app, Message::Apply);
        send(&mut app, Message::Intent(Intent::Select(parent)));
        send(&mut app, Message::EditStatus(Status::Done));
        send(&mut app, Message::Apply);
        assert!(app.error.is_none());
        send(&mut app, Message::Intent(Intent::Select(child.id)));
        send(&mut app, Message::EditStatus(Status::Wip));
        send(&mut app, Message::Apply);
        assert!(app.error.is_some());
        assert_eq!(
            app.items.iter().find(|i| i.id == child.id).unwrap().status,
            Status::WontDo
        );
    }

    #[test]
    fn picker_cancel_shortcuts_apply_and_clear_preserve_draft() {
        let mut app = app();
        add(&mut app, "Date picker");
        send(&mut app, Message::EditField(Field::Name, "Draft".into()));
        send(
            &mut app,
            Message::EditDate(DateSlot::Deadline, true, "Asia/Tokyo".into()),
        );
        send(
            &mut app,
            Message::OpenPicker(DateTarget::Task(DateSlot::Deadline)),
        );
        send(
            &mut app,
            Message::DatePicker(PickerAction::Day(
                NaiveDate::from_ymd_opt(2030, 2, 14).unwrap(),
            )),
        );
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Character("s".into()),
                keyboard::Modifiers::CTRL,
            ),
        );
        assert!(app.picker.is_some());
        assert!(!app.dialog_open);
        assert!(app.editor.is_some());
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default(),
            ),
        );
        assert!(app.picker.is_none());
        assert!(app.editor.as_ref().unwrap().deadline.value.is_empty());
        send(
            &mut app,
            Message::OpenPicker(DateTarget::Task(DateSlot::Deadline)),
        );
        send(
            &mut app,
            Message::DatePicker(PickerAction::Day(
                NaiveDate::from_ymd_opt(2030, 2, 14).unwrap(),
            )),
        );
        send(&mut app, Message::DatePicker(PickerAction::Timed(true)));
        send(&mut app, Message::DatePicker(PickerAction::Hour(15)));
        send(&mut app, Message::DatePicker(PickerAction::Minute(20)));
        send(&mut app, Message::DatePicker(PickerAction::Apply));
        let editor = app.editor.as_ref().unwrap();
        assert_eq!(editor.deadline.value, "2030-02-14 15:20");
        assert_eq!(editor.deadline.timezone, "Asia/Tokyo");
        assert_eq!(editor.name, "Draft");
        assert!(app.items[0].deadline.is_none());
        send(
            &mut app,
            Message::OpenPicker(DateTarget::Task(DateSlot::Deadline)),
        );
        send(&mut app, Message::DatePicker(PickerAction::Clear));
        assert!(app.editor.as_ref().unwrap().deadline.value.is_empty());
        assert_eq!(app.editor.as_ref().unwrap().deadline.timezone, "Asia/Tokyo");
    }

    #[test]
    fn picker_rejects_dst_gaps_and_routes_occurrence_and_filter_dates() {
        let mut app = app();
        let occurrence = recurring(&mut app);
        send(&mut app, Message::EditOccurrence(Box::new(occurrence)));
        send(&mut app, Message::OpenPicker(DateTarget::Occurrence(true)));
        send(
            &mut app,
            Message::DatePicker(PickerAction::Day(
                NaiveDate::from_ymd_opt(2030, 1, 9).unwrap(),
            )),
        );
        send(&mut app, Message::DatePicker(PickerAction::Apply));
        assert!(
            app.occurrence
                .as_ref()
                .unwrap()
                .end
                .value
                .starts_with("2030-01-09")
        );
        assert!(app.occurrence.as_ref().unwrap().dirty);
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        send(&mut app, Message::OpenPicker(DateTarget::Filter(false)));
        send(
            &mut app,
            Message::DatePicker(PickerAction::Day(
                NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
            )),
        );
        send(&mut app, Message::DatePicker(PickerAction::Apply));
        assert_eq!(app.filter.date_field, Some(DateField::Deadline));
        assert_eq!(app.filter_values[4], "2030-01-01");
        let id = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(
            &mut app,
            Message::EditDate(DateSlot::Planned, true, "Europe/Budapest".into()),
        );
        send(
            &mut app,
            Message::OpenPicker(DateTarget::Task(DateSlot::Planned)),
        );
        send(
            &mut app,
            Message::DatePicker(PickerAction::Day(
                NaiveDate::from_ymd_opt(2026, 3, 29).unwrap(),
            )),
        );
        send(&mut app, Message::DatePicker(PickerAction::Timed(true)));
        send(&mut app, Message::DatePicker(PickerAction::Hour(2)));
        send(&mut app, Message::DatePicker(PickerAction::Minute(30)));
        send(&mut app, Message::DatePicker(PickerAction::Apply));
        assert!(app.picker.as_ref().unwrap().error.is_some());
        assert!(app.editor.as_ref().unwrap().planned.value.is_empty());
    }

    #[test]
    fn closing_task_drawer_preserves_page_filters_month_and_applied_work() {
        let mut app = app();
        let id = add(&mut app, "Keep working document");
        send(&mut app, Message::Intent(Intent::Switch(Page::Calendar)));
        send(&mut app, Message::Search("Keep".into()));
        send(&mut app, Message::StatusFilter(Some(Status::Pending)));
        app.month = NaiveDate::from_ymd_opt(2030, 6, 1).unwrap();
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        assert!(app.editor.is_none());
        assert!(app.pending.is_none());
        assert!(!app.dialog_open);
        assert_eq!(app.page, Page::Calendar);
        assert_eq!(app.filter.search, "Keep");
        assert_eq!(app.filter.status, Some(Status::Pending));
        assert_eq!(app.month, NaiveDate::from_ymd_opt(2030, 6, 1).unwrap());
        assert_eq!(app.items[0].name, "Keep working document");
        assert!(app.document.as_ref().unwrap().is_dirty());
    }

    #[test]
    fn closing_edited_drawer_can_stay_discard_or_apply_without_saving_file() {
        let mut app = app();
        let id = add(&mut app, "Original");
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        send(&mut app, Message::Search("Original".into()));
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Name, "Draft".into()));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        assert!(matches!(app.pending, Some(Intent::CloseDrawer)));
        send(&mut app, Message::CancelIntent);
        assert_eq!(app.editor.as_ref().unwrap().name, "Draft");
        assert!(app.editor.is_some());
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        assert_eq!(app.items[0].name, "Original");
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Name, "Applied".into()));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert!(app.editor.is_none());
        assert!(app.pending.is_none());
        assert!(!app.dialog_open);
        assert!(app.document.as_ref().unwrap().path().is_none());
        assert_eq!(app.items[0].name, "Applied");
        assert_eq!(app.page, Page::Tasks);
        assert_eq!(app.filter.search, "Original");
    }

    #[test]
    fn closing_new_task_drawer_validates_before_apply_and_can_discard() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::Add));
        send(&mut app, Message::EditField(Field::Name, " ".into()));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert!(app.error.is_some());
        assert!(matches!(app.pending, Some(Intent::CloseDrawer)));
        assert!(app.editor.is_some());
        assert!(app.items.is_empty());
        send(&mut app, Message::CancelIntent);
        send(&mut app, Message::EditField(Field::Name, "New task".into()));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert!(app.editor.is_none());
        assert_eq!(app.items.len(), 1);
        assert_eq!(app.items[0].name, "New task");
        assert!(!app.dialog_open);
        send(&mut app, Message::Intent(Intent::Add));
        send(
            &mut app,
            Message::EditField(Field::Name, "Discard me".into()),
        );
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        assert_eq!(app.items.len(), 1);
    }

    #[test]
    fn escape_closes_only_the_topmost_drawer_interaction() {
        let mut app = app();
        let id = add(&mut app, "Escape");
        let escape = || {
            Message::Key(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default(),
            )
        };
        send(&mut app, escape());
        assert!(app.editor.is_none());
        assert!(app.pending.is_none());
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::DeleteRequest);
        send(&mut app, escape());
        assert!(!app.delete_confirm);
        assert!(app.editor.is_some());
        send(
            &mut app,
            Message::OpenPicker(DateTarget::Task(DateSlot::Deadline)),
        );
        send(&mut app, escape());
        assert!(app.picker.is_none());
        assert!(app.editor.is_some());
        send(
            &mut app,
            Message::EditField(Field::Name, "Keep draft".into()),
        );
        send(&mut app, escape());
        assert!(matches!(app.pending, Some(Intent::CloseDrawer)));
        send(&mut app, escape());
        assert!(app.pending.is_none());
        assert_eq!(app.editor.as_ref().unwrap().name, "Keep draft");
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(false));
        send(&mut app, Message::SaveAs);
        assert!(app.dialog_open);
        send(&mut app, escape());
        assert!(app.dialog_open);
        assert!(app.editor.is_none());
        send(&mut app, Message::SavePath(None));
    }

    #[test]
    fn closing_occurrence_drawer_guards_edits_without_changing_calendar_context() {
        let mut app = app();
        send(&mut app, Message::Intent(Intent::Switch(Page::Calendar)));
        let occurrence = recurring(&mut app);
        let month = app.month;
        send(
            &mut app,
            Message::EditOccurrence(Box::new(occurrence.clone())),
        );
        send(&mut app, Message::OccurrenceStatus(Status::Done));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        assert!(matches!(app.pending, Some(Intent::CloseDrawer)));
        send(&mut app, Message::Continue(false));
        assert!(app.occurrence.is_none());
        assert!(app.overrides.is_empty());
        send(&mut app, Message::EditOccurrence(Box::new(occurrence)));
        send(&mut app, Message::OccurrenceStatus(Status::Done));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert!(app.occurrence.is_none());
        assert_eq!(app.overrides.len(), 1);
        assert_eq!(app.overrides[0].status, Status::Done);
        assert_eq!(app.page, Page::Calendar);
        assert_eq!(app.month, month);
        assert!(!app.dialog_open);
    }

    #[test]
    fn pending_drawer_confirmation_blocks_shortcuts_and_background_mutations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("drawer.antodo");
        let mut app = app();
        add(&mut app, "Saved task");
        app.document.as_mut().unwrap().save_as(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        send(
            &mut app,
            Message::EditField(Field::Name, "Unapplied draft".into()),
        );
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        for message in [
            Message::Key(
                keyboard::Key::Character("s".into()),
                keyboard::Modifiers::CTRL,
            ),
            Message::Key(
                keyboard::Key::Character("o".into()),
                keyboard::Modifiers::CTRL,
            ),
            Message::Key(
                keyboard::Key::Character("n".into()),
                keyboard::Modifiers::CTRL,
            ),
            Message::Save,
            Message::Apply,
            Message::EditField(Field::Name, "Background change".into()),
            Message::Intent(Intent::New),
            Message::Intent(Intent::Close(window::Id::unique())),
        ] {
            send(&mut app, message);
            assert!(matches!(app.pending, Some(Intent::CloseDrawer)));
            assert_eq!(app.editor.as_ref().unwrap().name, "Unapplied draft");
            assert_eq!(app.items[0].name, "Saved task");
            assert!(!app.dialog_open);
            assert!(!app.document.as_ref().unwrap().is_dirty());
        }
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        send(&mut app, Message::Continue(false));
        assert!(app.editor.is_none());
        assert!(app.pending.is_none());
        assert_eq!(app.items[0].name, "Saved task");
    }
    #[test]
    fn task_selection_opens_editable_draft_and_successful_apply_closes_it() {
        let mut app = app();
        let id = add(&mut app, "Original");
        for page in Page::ALL {
            send(&mut app, Message::Intent(Intent::Switch(page)));
            send(&mut app, Message::Intent(Intent::Select(id)));
            send(
                &mut app,
                Message::EditField(Field::Name, "Edited directly".into()),
            );
            assert_eq!(
                app.items[0].name,
                if page == Page::Overview {
                    "Original"
                } else {
                    "Edited directly"
                }
            );
            send(&mut app, Message::Apply);
            assert!(app.editor.is_none());
            assert_eq!(app.items[0].name, "Edited directly");
            assert_eq!(app.page, page);
        }
    }

    #[test]
    fn rating_arrows_preserve_bounds_and_drafts_until_apply() {
        let mut app = app();
        add(&mut app, "Ratings");
        for field in [Field::Priority, Field::Severity] {
            for _ in 0..8 {
                send(&mut app, Message::StepRating(field, 1));
            }
            assert_eq!(app.editor.as_mut().unwrap().field_mut(field), "5");
            for _ in 0..8 {
                send(&mut app, Message::StepRating(field, -1));
            }
            assert_eq!(app.editor.as_mut().unwrap().field_mut(field), "1");
        }
        assert_eq!(app.items[0].priority, 3);
        assert_eq!(app.items[0].severity, 3);
        send(&mut app, Message::Apply);
        assert_eq!(app.items[0].priority, 1);
        assert_eq!(app.items[0].severity, 1);
        let id = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Priority, "bad".into()));
        send(&mut app, Message::StepRating(Field::Priority, 1));
        assert!(app.error.is_some());
        assert_eq!(app.editor.as_ref().unwrap().priority, "bad");
    }

    #[test]
    fn description_markdown_round_trips() {
        let mut app = app();
        add(&mut app, "Markdown");
        let source = "# Heading\n\n**Bold** and *italic* [link](https://example.com)";
        send(
            &mut app,
            Message::Description(text_editor::Action::Edit(text_editor::Edit::Paste(
                std::sync::Arc::new(source.into()),
            ))),
        );
        let draft = app.editor.as_ref().unwrap();
        assert_eq!(draft.description.text().trim_end(), source);
        assert!(draft.dirty);
        assert!(app.items[0].description.is_empty());
        send(&mut app, Message::Apply);
        assert_eq!(app.items[0].description.trim_end(), source);
    }
    #[test]
    fn delete_from_an_edited_draft_requires_confirmation_and_discards_only_that_task() {
        let mut app = app();
        let id = add(&mut app, "Delete me");
        send(
            &mut app,
            Message::EditField(Field::Name, "Unapplied name".into()),
        );
        send(&mut app, Message::DeleteRequest);
        assert_eq!(app.items[0].id, id);
        send(&mut app, Message::DeleteCancel);
        assert_eq!(app.editor.as_ref().unwrap().name, "Unapplied name");
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::DeleteConfirm);
        assert!(app.items.is_empty());
        assert!(app.editor.is_none());
        assert!(!app.delete_confirm);
    }
    #[test]
    fn calendar_creation_prefills_paired_dates_and_selected_timezone() {
        let mut app = app();
        send(
            &mut app,
            Message::CalendarTimezone("Pacific/Auckland".into()),
        );
        let day = NaiveDate::from_ymd_opt(2030, 4, 12).unwrap();
        send(&mut app, Message::Intent(Intent::AddOn(day)));
        let editor = app.editor.as_ref().unwrap();
        assert_eq!(editor.start.value, "2030-04-12");
        assert_eq!(editor.end.value, editor.start.value);
        assert_eq!(editor.start.timezone, "Pacific/Auckland");
        assert!(editor.dirty);
        send(&mut app, Message::Apply);
        assert!(app.editor.is_some()); // Empty name fails without losing the dates.
        send(
            &mut app,
            Message::EditField(Field::Name, "From calendar".into()),
        );
        send(&mut app, Message::Apply);
        assert!(app.editor.is_none());
        assert_eq!(
            app.items[0].start.as_ref().unwrap().timezone,
            "Pacific/Auckland"
        );
    }

    #[test]
    fn done_fills_completion_immediately_and_preserves_user_timestamp() {
        let mut app = app();
        let id = add(&mut app, "Completion");
        send(
            &mut app,
            Message::EditDate(DateSlot::Completion, true, "UTC".into()),
        );
        send(&mut app, Message::EditStatus(Status::Done));
        let value = &app.editor.as_ref().unwrap().completion;
        assert_eq!(value.value.len(), 19);
        assert!(
            (Utc::now() - value.parse().unwrap().unwrap().utc().unwrap())
                .num_seconds()
                .abs()
                < 3
        );
        send(
            &mut app,
            Message::EditDate(DateSlot::Completion, false, "2029-12-30 12:34:56".into()),
        );
        send(&mut app, Message::Apply);
        assert_eq!(
            app.items[0].completion.as_ref().unwrap().value,
            "2029-12-30 12:34:56"
        );
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditStatus(Status::Pending));
        send(&mut app, Message::Apply);
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditStatus(Status::Done));
        send(
            &mut app,
            Message::EditDate(DateSlot::Completion, false, String::new()),
        );
        send(&mut app, Message::Apply);
        assert!(app.items[0].completion.is_some());
    }

    #[test]
    fn close_file_and_quit_protect_unsaved_work() {
        let mut app = app();
        add(&mut app, "Keep me");
        send(&mut app, Message::Intent(Intent::Quit));
        assert!(matches!(app.pending, Some(Intent::Quit)));
        send(&mut app, Message::CancelIntent);
        send(&mut app, Message::Intent(Intent::CloseFile));
        assert!(matches!(app.pending, Some(Intent::CloseFile)));
        send(&mut app, Message::CancelIntent);
        assert_eq!(app.items.len(), 1);
        send(&mut app, Message::Intent(Intent::CloseFile));
        send(&mut app, Message::Continue(false));
        assert!(app.document.is_none());
        assert!(app.items.is_empty());
        assert!(app.editor.is_none());
        send(&mut app, Message::Intent(Intent::New));
        assert!(app.document.is_some());
    }

    #[test]
    fn parent_picker_filters_attaches_detaches_and_blocks_background_messages() {
        let mut app = app();
        let parent = add(&mut app, "Parent");
        let child = add(&mut app, "Child");
        send(&mut app, Message::ParentPicker(true));
        send(&mut app, Message::ParentSearch("Parent".into()));
        send(&mut app, Message::ParentStatus(Some(Status::Pending)));
        send(&mut app, Message::ParentPriority("3".into()));
        assert_eq!(
            core::query::filtered(&app.items, &app.parent_filter).len(),
            1
        );
        send(&mut app, Message::Apply);
        assert!(app.editor.is_some());
        send(&mut app, Message::ToggleParent(parent, true));
        assert_eq!(app.editor.as_ref().unwrap().parents, vec![parent]);
        send(&mut app, Message::ParentPicker(false));
        send(&mut app, Message::Apply);
        assert_eq!(
            app.items.iter().find(|i| i.id == child).unwrap().parents,
            vec![parent]
        );
        send(&mut app, Message::Intent(Intent::Select(child)));
        send(&mut app, Message::ToggleParent(parent, false));
        send(&mut app, Message::Apply);
        assert!(
            app.items
                .iter()
                .find(|i| i.id == child)
                .unwrap()
                .parents
                .is_empty()
        );
    }
    #[test]
    fn column_reordering_preserves_visibility_sorting_and_bounds() {
        use super::super::table::TaskColumn;
        let mut app = app();
        let initial = app.columns.clone();
        send(&mut app, Message::MoveColumn(TaskColumn::Name, -1));
        assert_eq!(app.columns, initial);
        send(&mut app, Message::MoveColumn(TaskColumn::Description, -1));
        assert_eq!(app.columns[0], TaskColumn::Description);
        assert_eq!(app.sort_column, TaskColumn::Name);
        send(&mut app, Message::MoveColumn(TaskColumn::Description, 1));
        assert_eq!(app.columns, initial);
        send(&mut app, Message::ToggleColumn(TaskColumn::Deadline, false));
        send(&mut app, Message::MoveColumn(TaskColumn::Deadline, -1));
        assert!(!app.columns.contains(&TaskColumn::Deadline));
        let last = *app.columns.last().unwrap();
        let before = app.columns.clone();
        send(&mut app, Message::MoveColumn(last, 1));
        assert_eq!(app.columns, before);
    }

    #[test]
    fn color_dialog_select_commits_but_cancel_and_escape_preserve_original() {
        let mut app = app();
        let original = app.tag_color.clone();
        send(&mut app, Message::OpenColor);
        send(&mut app, Message::ColorDraft("#12ABCD".into()));
        send(&mut app, Message::SaveTag);
        assert!(app.tags.is_empty());
        assert_eq!(app.tag_color, original);
        assert!(!app.tag_dirty);
        send(&mut app, Message::CancelColor);
        assert_eq!(app.tag_color, original);
        send(&mut app, Message::OpenColor);
        send(&mut app, Message::ColorDraft("#12ABCD".into()));
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default(),
            ),
        );
        assert!(app.color_draft.is_none());
        assert_eq!(app.tag_color, original);
        send(&mut app, Message::OpenColor);
        send(&mut app, Message::ColorDraft("#12ABCD".into()));
        send(&mut app, Message::SelectColor);
        assert_eq!(app.tag_color, "#12ABCD");
        assert!(app.tag_dirty);
        assert!(app.color_draft.is_none());
    }

    #[test]
    fn plan_today_applies_all_edits_and_closes_only_on_success() {
        let mut app = app();
        let id = add(&mut app, "Plan me");
        send(
            &mut app,
            Message::EditDate(DateSlot::Planned, true, "Pacific/Kiritimati".into()),
        );
        send(
            &mut app,
            Message::EditField(Field::Name, "Planned today".into()),
        );
        let before = Moment::from_utc(Utc::now(), "Pacific/Kiritimati", true).unwrap();
        send(&mut app, Message::PlanToday);
        assert!(app.editor.is_none());
        let saved = app.items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(saved.name, "Planned today");
        assert_eq!(saved.planned.as_ref(), Some(&before));
        send(&mut app, Message::Intent(Intent::Select(id)));
        send(&mut app, Message::EditField(Field::Name, String::new()));
        send(&mut app, Message::PlanToday);
        assert!(app.editor.is_some());
        assert!(app.error.is_some());
        assert_eq!(
            app.items.iter().find(|i| i.id == id).unwrap().name,
            "Planned today"
        );
    }
    #[test]
    fn event_editor_ignores_hidden_values_and_cannot_plan_today() {
        let mut app = app();
        let id = add(&mut app, "Appointment");
        send(
            &mut app,
            Message::EditField(Field::Effort, "invalid".into()),
        );
        send(
            &mut app,
            Message::EditDate(DateSlot::Deadline, false, "invalid".into()),
        );
        send(
            &mut app,
            Message::EditDate(DateSlot::Planned, false, "invalid".into()),
        );
        send(&mut app, Message::EditStatus(Status::Event));
        send(&mut app, Message::PlanToday);
        assert!(app.editor.is_some());
        assert_eq!(app.items[0].status, Status::Pending);
        send(&mut app, Message::Apply);
        assert!(app.error.is_none(), "{:?}", app.error);
        assert!(app.editor.is_none());
        let saved = app.items.iter().find(|i| i.id == id).unwrap();
        assert_eq!(saved.status, Status::Event);
        assert_eq!(saved.effort, 0);
        assert!(saved.deadline.is_none() && saved.planned.is_none() && saved.completion.is_none());
    }
    #[test]
    fn mcp_page_start_stop_configuration_and_workspace_lifecycle() {
        let mut app = App::empty(Preferences::default());
        assert_eq!(app.title(), "Welcome · antodo");
        let idle_subscriptions = app.subscription().units();
        send(&mut app, Message::Intent(Intent::Switch(Page::Mcp)));
        assert_eq!(app.page, Page::Overview);
        send(&mut app, Message::McpStart);
        assert!(app.mcp_server.is_none());
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Intent(Intent::Switch(Page::Mcp)));
        assert_eq!(app.page, Page::Mcp);
        send(
            &mut app,
            Message::McpField(super::super::mcp::Field::Port, "bad".into()),
        );
        send(&mut app, Message::McpStart);
        assert!(app.error.is_some());
        assert!(app.mcp_server.is_none());
        send(
            &mut app,
            Message::McpField(super::super::mcp::Field::Port, "0".into()),
        );
        send(&mut app, Message::McpStart);
        let address = app.mcp_server.as_ref().unwrap().address;
        assert_eq!(app.subscription().units(), idle_subscriptions + 1);
        let token = app.mcp_settings.token.clone();
        for (field, value) in [
            (super::super::mcp::Field::Address, "127.0.0.2"),
            (super::super::mcp::Field::Port, "1"),
            (super::super::mcp::Field::Token, "replacement-token"),
        ] {
            send(&mut app, Message::McpField(field, value.into()));
        }
        assert_eq!(app.mcp_settings.address, "127.0.0.1");
        assert_eq!(app.mcp_settings.port, "0");
        assert_eq!(app.mcp_settings.token, token);
        send(&mut app, Message::McpEdits(true));
        assert!(!app.mcp_settings.allow_edits);
        send(&mut app, Message::McpStop);
        assert!(app.mcp_server.is_none());
        assert_eq!(app.subscription().units(), idle_subscriptions);
        assert!(std::net::TcpStream::connect(address).is_err());
        send(&mut app, Message::McpEdits(true));
        send(&mut app, Message::McpStart);
        assert!(app.mcp_server.is_some());
        send(&mut app, Message::Intent(Intent::New));
        assert!(app.mcp_server.is_none());
        send(&mut app, Message::McpStart);
        send(&mut app, Message::Intent(Intent::CloseFile));
        assert!(app.mcp_server.is_none());
        assert!(app.document.is_none());
    }
    #[test]
    fn mcp_poll_blocks_each_local_interaction_and_refreshes_successful_writes() {
        let mut app = app();
        let item = app
            .document
            .as_mut()
            .unwrap()
            .save_item(Item::new("Original"))
            .unwrap();
        app.refresh().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("tasks.antodo");
        send(&mut app, Message::SavePath(Some(path.clone())));
        send(
            &mut app,
            Message::McpField(super::super::mcp::Field::Port, "0".into()),
        );
        send(&mut app, Message::McpEdits(true));
        send(&mut app, Message::McpStart);
        cache_views(&app);
        for guard in 0..9 {
            app.editor = None;
            app.occurrence = None;
            app.tag_dirty = false;
            app.pending = None;
            app.dialog_open = false;
            app.picker = None;
            app.parent_picker = false;
            app.color_draft = None;
            app.chat_open = false;
            match guard {
                0 => app.editor = Some(Editor::new(item.clone())),
                1 => {
                    app.occurrence = Some(OccurrenceEditor::new(
                        "Original".into(),
                        Occurrence {
                            item_id: item.id,
                            original_start: moment("2030-01-01"),
                            original_end: moment("2030-01-01"),
                            start: moment("2030-01-01"),
                            end: moment("2030-01-01"),
                            status: Status::Pending,
                            reminded: false,
                            overridden: false,
                        },
                    ))
                }
                2 => app.tag_dirty = true,
                3 => app.pending = Some(Intent::New),
                4 => app.dialog_open = true,
                5 => {
                    app.picker = Some(DatePicker::new(
                        DateTarget::Filter(false),
                        &super::super::editor::DateInput::from(&None),
                    ))
                }
                6 => app.parent_picker = true,
                7 => app.color_draft = Some("#123456".into()),
                8 => app.chat_open = true,
                _ => unreachable!(),
            }
            let reply = mcp_call(
                &mut app,
                "update_task",
                json!({"id":item.id,"fields":{"name":"External"}}),
            );
            assert_eq!(reply["result"]["isError"], true, "guard {guard}: {reply}");
            assert_eq!(app.items, std::slice::from_ref(&item));
            assert!(!app.document.as_ref().unwrap().is_dirty());
            assert!(app.dashboard_cache.get().is_some());
            assert!(app.calendar_cache.get().is_some());
            assert!(app.table_order.get().is_some());
            let reply = mcp_call(&mut app, "get_task", json!({"id":item.id}));
            assert_eq!(reply["result"]["isError"], false);
        }
        app.chat_open = false;
        let reply = mcp_call(
            &mut app,
            "update_task",
            json!({"id":item.id,"fields":{"name":"External"}}),
        );
        assert_eq!(reply["result"]["isError"], false);
        assert_eq!(app.items[0].name, "External");
        assert!(app.document.as_ref().unwrap().is_dirty());
        assert!(app.dashboard_cache.get().is_none());
        assert!(app.calendar_cache.get().is_none());
        assert!(app.table_order.get().is_none());
        assert!(app.notice.as_deref().unwrap().contains("MCP updated"));
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "Original"
        );
        send(&mut app, Message::Save);
        assert_eq!(
            Document::open(&path).unwrap().items().unwrap()[0].name,
            "External"
        );
    }

    #[test]
    fn chat_poll_refreshes_clean_editors_but_preserves_unapplied_drafts() {
        for (delete, dirty) in [(false, false), (true, false), (false, true)] {
            let mut app = app();
            let id = add(&mut app, "Original");
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("tasks.antodo");
            send(&mut app, Message::SavePath(Some(path.clone())));
            if dirty {
                send(
                    &mut app,
                    Message::EditField(Field::Name, "Local draft".into()),
                );
            }
            let name = if delete { "delete_task" } else { "update_task" };
            let (config, worker) = chat_provider(vec![
                chat_tool(name, json!({"id":id,"fields":{"name":"From chat"}})),
                json!({"role":"assistant","content":"Tool result received."}),
            ]);
            app.llm_config = Some(config);
            send(&mut app, Message::ChatOpen(Some(id)));
            send(&mut app, Message::ChatInput("Change this task".into()));
            let idle_subscriptions = app.subscription().units();
            send(&mut app, Message::ChatSend);
            assert!(app.chat_input.is_empty());
            assert_eq!(app.subscription().units(), idle_subscriptions + 1);
            cache_views(&app);
            poll_until(&mut app, Message::ChatPoll, |app| !app.chat.busy());
            let requests = worker.join().unwrap();
            assert_eq!(app.subscription().units(), idle_subscriptions);
            assert!(app.chat.error.is_none(), "{:?}", app.chat.error);
            assert!(
                requests[0]["messages"][1]["content"]
                    .as_str()
                    .unwrap()
                    .contains(&id.to_string())
            );
            assert_eq!(requests[1]["messages"][3]["role"], "tool");
            if dirty {
                assert_eq!(app.editor.as_ref().unwrap().name, "Local draft");
                assert!(app.editor.as_ref().unwrap().dirty);
                assert_eq!(app.items[0].name, "Original");
                assert!(!app.document.as_ref().unwrap().is_dirty());
                assert!(app.dashboard_cache.get().is_some());
                assert!(
                    app.chat
                        .log
                        .iter()
                        .any(|(_, value)| value.contains("rejected"))
                );
            } else {
                assert!(app.document.as_ref().unwrap().is_dirty());
                assert!(app.dashboard_cache.get().is_none());
                assert!(app.calendar_cache.get().is_none());
                assert!(app.table_order.get().is_none());
                assert!(app.notice.as_deref().unwrap().contains("Chat updated"));
                if delete {
                    assert!(app.items.is_empty());
                    assert!(app.editor.is_none());
                } else {
                    assert_eq!(app.items[0].name, "From chat");
                    assert_eq!(app.editor.as_ref().unwrap().name, "From chat");
                    assert!(!app.editor.as_ref().unwrap().dirty);
                }
            }
            assert_eq!(
                Document::open(&path).unwrap().items().unwrap()[0].name,
                "Original"
            );
        }
    }

    #[test]
    fn closing_chat_discards_a_ready_action_without_losing_task_drafts() {
        for close in [
            Message::ChatCancel,
            Message::ChatClose,
            Message::Key(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default(),
            ),
            Message::Intent(Intent::Quit),
        ] {
            let mut app = app();
            let id = add(&mut app, "Original");
            send(
                &mut app,
                Message::EditField(Field::Name, "Keep draft".into()),
            );
            let (config, worker) = chat_provider(vec![chat_tool("delete_task", json!({"id":id}))]);
            app.llm_config = Some(config);
            send(&mut app, Message::ChatOpen(Some(id)));
            send(&mut app, Message::ChatInput("Delete".into()));
            send(&mut app, Message::ChatSend);
            // Wait for the proposed action, but never deliver it to the UI poll handler.
            let start = Instant::now();
            while !worker.is_finished() {
                assert!(start.elapsed() < Wait::from_secs(10));
                thread::sleep(Wait::from_millis(5));
            }
            worker.join().unwrap();
            send(&mut app, close.clone());
            send(&mut app, Message::ChatPoll);
            assert!(!app.chat.busy());
            assert_eq!(app.chat_open, matches!(close, Message::ChatCancel));
            assert_eq!(app.items[0].name, "Original");
            assert_eq!(app.editor.as_ref().unwrap().name, "Keep draft");
            assert_eq!(
                app.pending.is_some(),
                matches!(close, Message::Intent(Intent::Quit))
            );
        }
    }

    #[test]
    fn invalid_draft_save_and_timezone_actions_preserve_applied_work() {
        let mut app = app();
        add(&mut app, "Original");
        send(&mut app, Message::EditField(Field::Name, String::new()));
        app.continuation = Some(Intent::New);
        send(&mut app, Message::Save);
        assert!(app.error.is_some());
        assert!(app.continuation.is_none());
        assert!(!app.dialog_open);
        assert!(app.editor.is_some());
        assert_eq!(app.items[0].name, "Original");
        send(
            &mut app,
            Message::EditDate(DateSlot::Planned, true, "Invalid/Zone".into()),
        );
        send(&mut app, Message::PlanToday);
        assert!(app.error.as_deref().unwrap().contains("timezone"));
        assert!(app.editor.as_ref().unwrap().planned.value.is_empty());
        send(
            &mut app,
            Message::EditDate(DateSlot::Completion, true, "Invalid/Zone".into()),
        );
        send(&mut app, Message::EditStatus(Status::Done));
        assert!(app.error.as_deref().unwrap().contains("timezone"));
        assert!(app.editor.as_ref().unwrap().completion.value.is_empty());
        assert_eq!(app.items[0].status, Status::Pending);
    }

    #[test]
    fn table_visibility_and_sorting_keep_one_column_and_invalidate_cached_order() {
        use super::super::table::TaskColumn;
        let mut app = app();
        app.columns = vec![TaskColumn::Name];
        send(&mut app, Message::ToggleColumn(TaskColumn::Name, false));
        assert_eq!(app.columns, [TaskColumn::Name]);
        for _ in 0..2 {
            send(&mut app, Message::ToggleColumn(TaskColumn::Priority, true));
        }
        assert_eq!(app.columns, [TaskColumn::Name, TaskColumn::Priority]);
        app.table_order.set(vec![1, 0]).unwrap();
        send(&mut app, Message::SortColumn(TaskColumn::Priority));
        assert_eq!(app.sort_column, TaskColumn::Priority);
        assert!(!app.sort_descending);
        assert!(app.table_order.get().is_none());
        send(&mut app, Message::SortColumn(TaskColumn::Priority));
        assert!(app.sort_descending);
        send(&mut app, Message::SortColumn(TaskColumn::Name));
        assert!(!app.sort_descending);
        send(&mut app, Message::ToggleColumn(TaskColumn::Priority, false));
        assert_eq!(app.columns, [TaskColumn::Name]);
    }

    #[test]
    fn changing_llm_provider_clears_credentials_and_invalid_settings_keep_active_connection() {
        use super::super::llm::Field;
        let mut app = app();
        send(&mut app, Message::LlmConfigure);
        let active = app.llm_config.as_ref().unwrap().endpoint.clone();
        for provider in core::llm::Provider::ALL {
            send(
                &mut app,
                Message::LlmField(Field::Key, "session-secret".into()),
            );
            send(&mut app, Message::LlmProvider(provider));
            assert!(app.llm_settings.key.is_empty());
            assert_eq!(app.llm_config.as_ref().unwrap().endpoint, active);
        }
        send(&mut app, Message::LlmProvider(core::llm::Provider::OpenAi));
        send(
            &mut app,
            Message::LlmField(Field::Model, "test-model".into()),
        );
        send(
            &mut app,
            Message::LlmField(Field::Endpoint, "file:///invalid".into()),
        );
        send(&mut app, Message::LlmField(Field::Key, "test-key".into()));
        send(&mut app, Message::LlmEdits(true));
        send(&mut app, Message::LlmConfigure);
        assert!(app.error.is_some());
        assert_eq!(app.llm_config.as_ref().unwrap().endpoint, active);
        send(
            &mut app,
            Message::LlmField(
                Field::Endpoint,
                "https://example.test/v1/chat/completions".into(),
            ),
        );
        app.chat
            .log
            .push(("Assistant".into(), "Old connection".into()));
        send(&mut app, Message::LlmConfigure);
        assert!(app.error.is_none());
        assert!(app.chat.log.is_empty());
        assert!(app.llm_config.as_ref().unwrap().edits);
        assert_eq!(app.llm_config.as_ref().unwrap().model, "test-model");
        send(&mut app, Message::LlmDisconnect);
        assert!(app.llm_config.is_none());
        assert!(app.llm_settings.key.is_empty());
    }

    #[test]
    fn blank_filter_inputs_clear_values_without_changing_other_filters() {
        let mut app = app();
        send(&mut app, Message::StatusFilter(Some(Status::Wip)));
        for (field, value) in [
            (FilterText::Priority, "2"),
            (FilterText::From, "2030-01-01"),
            (FilterText::To, "2030-01-31"),
            (FilterText::ReminderMinutes, "10"),
        ] {
            send(&mut app, Message::FilterText(field, value.into()));
            send(&mut app, Message::FilterText(field, "  ".into()));
            assert!(app.error.is_none());
        }
        assert!(app.filter.priority.is_none());
        assert!(app.filter.date_from.is_none());
        assert!(app.filter.date_to.is_none());
        assert!(app.filter.reminder_minutes.is_none());
        assert_eq!(app.filter.status, Some(Status::Wip));
        assert_eq!(app.filter.date_field, Some(DateField::Deadline));
    }

    #[test]
    fn llm_setup_chat_context_and_workspace_lifecycle() {
        let mut state = App::empty(Preferences::default());
        send(&mut state, Message::Intent(Intent::Switch(Page::Llm)));
        assert_eq!(state.page, Page::Overview);
        send(&mut state, Message::ChatOpen(None));
        assert!(!state.chat_open);
        send(&mut state, Message::Intent(Intent::New));
        send(&mut state, Message::Intent(Intent::Switch(Page::Llm)));
        assert_eq!(state.page, Page::Llm);
        send(
            &mut state,
            Message::LlmProvider(core::llm::Provider::OpenAi),
        );
        send(&mut state, Message::LlmConfigure);
        assert!(state.llm_config.is_none());
        assert!(state.error.is_some());
        send(
            &mut state,
            Message::LlmProvider(core::llm::Provider::Ollama),
        );
        send(&mut state, Message::LlmConfigure);
        assert!(state.llm_config.is_some());
        assert!(state.error.is_none());
        send(&mut state, Message::ChatOpen(None));
        send(&mut state, Message::ChatInput("".into()));
        send(&mut state, Message::ChatSend);
        assert!(state.chat.error.is_some());
        send(&mut state, Message::ChatClose);
        let id = add(&mut state, "Context task");
        send(&mut state, Message::ChatOpen(Some(id)));
        assert!(state.chat_open);
        assert_eq!(state.chat_context, Some(core::llm::Context::Task(id)));
        send(
            &mut state,
            Message::EditField(Field::Name, "Blocked".into()),
        );
        assert_eq!(state.editor.as_ref().unwrap().name, "Context task");
        state.chat.log.push(("Assistant".into(), "Remember".into()));
        send(&mut state, Message::ChatClear);
        assert!(state.chat.log.is_empty());
        send(&mut state, Message::ChatClose);
        send(&mut state, Message::LlmDisconnect);
        assert!(state.llm_config.is_none());
        send(&mut state, Message::LlmConfigure);
        state
            .chat
            .log
            .push(("Assistant".into(), "Old document".into()));
        state.reset_workspace();
        assert!(state.chat.log.is_empty());
        assert!(!state.chat_open);
        assert!(state.chat_context.is_none());
        assert!(state.llm_config.is_some());
    }

    #[test]
    fn note_drawers_block_external_mutations_and_mcp_refreshes_notes() {
        use super::super::notes::Action;

        let mut app = app();
        let note = app
            .document
            .as_mut()
            .unwrap()
            .save_note(core::Note::new("Original note"))
            .unwrap();
        let task = app
            .document
            .as_mut()
            .unwrap()
            .save_item(Item::new("Original task"))
            .unwrap();
        app.refresh().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.antodo");
        send(&mut app, Message::SavePath(Some(path.clone())));
        send(
            &mut app,
            Message::McpField(super::super::mcp::Field::Port, "0".into()),
        );
        send(&mut app, Message::McpEdits(true));
        send(&mut app, Message::McpStart);
        send(&mut app, Message::Intent(Intent::Switch(Page::Notes)));

        for dirty in [false, true] {
            send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
            if dirty {
                send(
                    &mut app,
                    Message::Note(Action::Name("Local note draft".into())),
                );
            }
            for (tool, args) in [
                ("create_note", json!({"name":"Blocked note"})),
                (
                    "update_note",
                    json!({"id":note.id,"fields":{"name":"Blocked change"}}),
                ),
                ("delete_note", json!({"id":note.id})),
                ("create_task", json!({"name":"Blocked task"})),
                (
                    "update_task",
                    json!({"id":task.id,"fields":{"name":"Blocked change"}}),
                ),
                ("delete_task", json!({"id":task.id})),
            ] {
                let reply = mcp_call(&mut app, tool, args);
                assert_eq!(reply["result"]["isError"], true, "{tool}: {reply}");
                assert_eq!(app.notes, std::slice::from_ref(&note));
                assert_eq!(app.items, std::slice::from_ref(&task));
                assert!(!app.document.as_ref().unwrap().is_dirty());
                assert_eq!(
                    app.note_editor.as_ref().unwrap().name,
                    if dirty {
                        "Local note draft"
                    } else {
                        "Original note"
                    }
                );
            }
            let reply = mcp_call(&mut app, "get_note", json!({"id":note.id}));
            assert_eq!(reply["result"]["isError"], false);
            let read: Value =
                serde_json::from_str(reply["result"]["content"][0]["text"].as_str().unwrap())
                    .unwrap();
            assert_eq!(read["name"], "Original note");
            send(&mut app, Message::Intent(Intent::CloseDrawer));
            if dirty {
                assert!(app.pending.is_some());
                send(&mut app, Message::Continue(false));
            }
            assert!(app.note_editor.is_none());
        }

        let reply = mcp_call(
            &mut app,
            "create_note",
            json!({"name":"External note","content":"# Markdown"}),
        );
        assert_eq!(reply["result"]["isError"], false);
        assert_eq!(app.notes.len(), 2);
        assert!(app.notes.iter().any(|note| note.content == "# Markdown"));
        assert!(app.document.as_ref().unwrap().is_dirty());
        assert!(app.notice.as_deref().unwrap().contains("MCP updated"));
        let reply = mcp_call(
            &mut app,
            "update_note",
            json!({"id":note.id,"fields":{"name":"Externally updated"}}),
        );
        assert_eq!(reply["result"]["isError"], false);
        assert_eq!(
            app.notes
                .iter()
                .find(|value| value.id == note.id)
                .unwrap()
                .name,
            "Externally updated"
        );
        assert_eq!(
            Document::open(&path).unwrap().notes().unwrap(),
            std::slice::from_ref(&note)
        );
        send(&mut app, Message::Save);
        assert_eq!(Document::open(&path).unwrap().notes().unwrap(), app.notes);
        let reply = mcp_call(&mut app, "delete_note", json!({"id":note.id}));
        assert_eq!(reply["result"]["isError"], false);
        assert_eq!(app.notes.len(), 1);
        assert!(!app.notes.iter().any(|value| value.id == note.id));
    }
    #[test]
    fn note_chat_refreshes_links_and_preserves_local_drafts() {
        for (delete, dirty) in [(false, false), (true, false), (false, true)] {
            let mut app = app();
            let document = app.document.as_mut().unwrap();
            let task = document.save_item(Item::new("Linked task")).unwrap();
            let tag = document.save_tag(Tag::new("Research", "#123456")).unwrap();
            let note = document
                .save_note(core::Note::new("Original note"))
                .unwrap();
            app.refresh().unwrap();
            let folder = tempfile::tempdir().unwrap();
            let path = folder.path().join("note-chat.antodo");
            send(&mut app, Message::SavePath(Some(path.clone())));
            send(&mut app, Message::Intent(Intent::SelectNote(note.id)));
            if dirty {
                send(
                    &mut app,
                    Message::Note(super::super::notes::Action::Name("Local note draft".into())),
                );
            }
            let name = if delete { "delete_note" } else { "update_note" };
            let (config, worker) = chat_provider(vec![
                chat_tool(
                    name,
                    json!({"id": note.id, "fields": {"name": "From chat", "tags": [tag.id], "tasks": [task.id]}}),
                ),
                json!({"role":"assistant", "content":"Tool result received."}),
            ]);
            app.llm_config = Some(config);
            send(&mut app, Message::NoteChatOpen(note.id));
            assert_eq!(app.chat_context, Some(core::llm::Context::Note(note.id)));
            send(
                &mut app,
                Message::ChatInput("Update this note and its links".into()),
            );
            send(&mut app, Message::ChatSend);
            poll_until(&mut app, Message::ChatPoll, |app| !app.chat.busy());
            let requests = worker.join().unwrap();
            assert!(
                requests[0]["messages"][1]["content"]
                    .as_str()
                    .unwrap()
                    .contains(&format!("Selected note: {}", note.id))
            );
            assert!(app.chat.error.is_none(), "{:?}", app.chat.error);
            assert_eq!(
                Document::open(&path).unwrap().notes().unwrap(),
                std::slice::from_ref(&note)
            );
            if dirty {
                assert_eq!(app.note_editor.as_ref().unwrap().name, "Local note draft");
                assert!(app.note_editor.as_ref().unwrap().dirty);
                assert_eq!(app.notes, [note]);
                assert!(!app.document.as_ref().unwrap().is_dirty());
                assert!(
                    app.chat
                        .log
                        .iter()
                        .any(|(_, line)| line.contains("rejected"))
                );
            } else if delete {
                assert!(app.note_editor.is_none());
                assert!(app.notes.is_empty());
            } else {
                let editor = app.note_editor.as_ref().unwrap();
                assert_eq!(editor.name, "From chat");
                assert!(!editor.dirty);
                assert_eq!(editor.note().tags, [tag.id]);
                assert_eq!(editor.note().tasks, [task.id]);
                assert_eq!(app.notes[0], editor.note());
            }
        }
    }

    #[test]
    fn note_chat_context_requires_a_persisted_note_and_clears_on_workspace_change() {
        let mut app = app();
        send(&mut app, Message::LlmConfigure);
        send(&mut app, Message::Intent(Intent::AddNote));
        let unsaved_id = app.note_editor.as_ref().unwrap().original.id;
        send(&mut app, Message::NoteChatOpen(unsaved_id));
        assert!(!app.chat_open);
        send(
            &mut app,
            Message::Note(super::super::notes::Action::Name("Context note".into())),
        );
        send(&mut app, Message::Apply);
        send(&mut app, Message::Intent(Intent::SelectNote(unsaved_id)));
        send(&mut app, Message::NoteChatOpen(unsaved_id));
        assert!(app.chat_open);
        assert_eq!(app.chat_context, Some(core::llm::Context::Note(unsaved_id)));
        send(&mut app, Message::ChatClose);
        app.reset_workspace();
        assert!(app.chat_context.is_none());
        assert!(!app.chat_open);
    }
    fn navigation_fixture() -> (App, Uuid, Uuid, Uuid) {
        let mut app = app();
        let doc = app.document.as_mut().unwrap();
        let first = doc.save_item(Item::new("First task")).unwrap().id;
        let second = doc.save_item(Item::new("Second task")).unwrap().id;
        let mut note = core::Note::new("Shared note");
        note.tasks = vec![first, second];
        let note = doc.save_note(note).unwrap().id;
        app.refresh().unwrap();
        (app, first, second, note)
    }

    #[test]
    fn linked_drawers_return_one_level_for_both_directions_and_repeated_records() {
        let (mut app, first, second, note) = navigation_fixture();
        for path in [
            vec![
                DrawerTarget::Task(first),
                DrawerTarget::Note(note),
                DrawerTarget::Task(second),
                DrawerTarget::Note(note),
            ],
            vec![
                DrawerTarget::Note(note),
                DrawerTarget::Task(first),
                DrawerTarget::Note(note),
                DrawerTarget::Task(second),
            ],
        ] {
            for target in &path {
                let intent = match target {
                    DrawerTarget::Task(id) => Intent::Select(*id),
                    DrawerTarget::Note(id) => Intent::SelectNote(*id),
                };
                send(&mut app, Message::Intent(intent));
                assert_eq!(app.drawer_target(), Some(*target));
            }
            for target in path.iter().rev().skip(1) {
                send(&mut app, Message::Intent(Intent::CloseDrawer));
                assert_eq!(app.drawer_target(), Some(*target));
                assert!(app.editor.is_some() != app.note_editor.is_some());
            }
            send(&mut app, Message::Intent(Intent::CloseDrawer));
            assert!(app.drawer_target().is_none());
            assert!(app.drawer_history.is_empty());
        }
    }

    #[test]
    fn nested_drawers_keep_apply_discard_stay_and_refresh_parent_links() {
        let (mut app, first, _, note) = navigation_fixture();
        send(&mut app, Message::Intent(Intent::Select(first)));
        send(
            &mut app,
            Message::EditField(Field::Name, "Renamed task".into()),
        );
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        assert!(app.pending.is_some());
        send(&mut app, Message::CancelIntent);
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Task(first)));
        assert!(app.drawer_history.is_empty());
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        send(&mut app, Message::Continue(true));
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Note(note)));
        send(
            &mut app,
            Message::Note(super::super::notes::Action::ToggleTask(first, false)),
        );
        send(
            &mut app,
            Message::Note(super::super::notes::Action::Name("".into())),
        );
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Continue(true));
        assert!(app.error.is_some());
        assert!(app.pending.is_some());
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Note(note)));
        send(&mut app, Message::CancelIntent);
        send(
            &mut app,
            Message::Note(super::super::notes::Action::Name("Updated note".into())),
        );
        send(&mut app, Message::Apply);
        let editor = app.editor.as_ref().unwrap();
        assert_eq!(editor.name, "Renamed task");
        assert!(
            editor.notes.is_empty(),
            "return reloads changed shared links"
        );
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        send(
            &mut app,
            Message::Note(super::super::notes::Action::Name("Discarded".into())),
        );
        send(
            &mut app,
            Message::Key(
                keyboard::Key::Named(keyboard::key::Named::Escape),
                keyboard::Modifiers::default(),
            ),
        );
        assert!(app.pending.is_some());
        send(&mut app, Message::Continue(false));
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Task(first)));
        assert_eq!(
            app.document.as_ref().unwrap().note(note).unwrap().name,
            "Updated note"
        );
    }

    #[test]
    fn nested_drawer_deletion_returns_and_navigation_changes_clear_history() {
        let (mut app, first, _, note) = navigation_fixture();
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        send(&mut app, Message::Intent(Intent::Select(first)));
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::DeleteConfirm);
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Note(note)));
        assert!(!app.note_editor.as_ref().unwrap().tasks.contains(&first));
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        assert!(
            app.drawer_history.is_empty(),
            "reselecting current record adds no level"
        );
        send(&mut app, Message::Intent(Intent::Switch(Page::Tasks)));
        assert!(app.drawer_history.is_empty());
        assert!(app.drawer_target().is_none());
        let second = app.items[0].id;
        send(&mut app, Message::Intent(Intent::Select(second)));
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        send(&mut app, Message::DeleteRequest);
        send(&mut app, Message::Note(super::super::notes::Action::Delete));
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Task(second)));
        assert!(app.editor.as_ref().unwrap().notes.is_empty());
        send(&mut app, Message::Intent(Intent::New));
        send(&mut app, Message::Continue(false));
        assert!(app.drawer_history.is_empty());
        assert!(app.drawer_target().is_none());
    }
    #[test]
    fn linked_name_and_close_buttons_restore_the_previous_drawer() {
        use super::super::views::render_tests::Screen;
        let (mut app, first, _, note) = navigation_fixture();
        send(&mut app, Message::Intent(Intent::Select(first)));
        let messages = {
            let mut screen = Screen::new(
                super::super::notes::task_section(&app),
                iced::Size::new(700.0, 400.0),
            );
            screen.click("Shared note")
        };
        for message in messages {
            send(&mut app, message);
        }
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Note(note)));
        let messages = {
            let mut screen = Screen::new(app.view(), iced::Size::new(1400.0, 1000.0));
            screen.click("Close")
        };
        for message in messages {
            send(&mut app, message);
        }
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Task(first)));
        send(&mut app, Message::Intent(Intent::CloseDrawer));
        send(&mut app, Message::Intent(Intent::SelectNote(note)));
        let messages = {
            let mut screen = Screen::new(
                super::super::notes::drawer(&app, 900.0),
                iced::Size::new(900.0, 1800.0),
            );
            screen.click("First task")
        };
        for message in messages {
            send(&mut app, message);
        }
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Task(first)));
        let messages = {
            let mut screen = Screen::new(app.view(), iced::Size::new(1400.0, 1000.0));
            screen.click("× Close")
        };
        for message in messages {
            send(&mut app, message);
        }
        assert_eq!(app.drawer_target(), Some(DrawerTarget::Note(note)));
    }
}
