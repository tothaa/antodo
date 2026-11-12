//! Shared task selection, context menus, and hierarchy pointer gestures.
//! Every mutation is delegated to a transactional core command.
use super::app::{App, Intent, Message as AppMessage, Page};
use crate::core::actions::{TaskAction, TaskOccurrence};
use crate::core::{Status, query};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer, widget};
use iced::widget::{
    Space, button, column, container, mouse_area, opaque, responsive, row, scrollable, stack, text,
};
use iced::{
    Border, Element, Event, Fill, Length, Point, Rectangle, Size, Subscription, Task, Theme,
    Vector, keyboard,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Submenu {
    Status,
    Priority,
    Severity,
    Tags,
    Delete,
}
#[derive(Debug, Clone)]
pub enum Command {
    Edit,
    AddChild,
    LinkParent,
    LinkNote,
    RemoveParent,
    Apply(TaskAction),
}
#[derive(Debug, Clone)]
pub enum Message {
    Press(TaskOccurrence, Point),
    Release(Option<TaskOccurrence>),
    Move(Point),
    Modifiers(keyboard::Modifiers),
    Context(TaskOccurrence, Point),
    Submenu(Submenu),
    Choose(Command),
    LinkParent(Uuid),
    Close,
    Cancel,
}
#[derive(Debug, Clone)]
pub struct Menu {
    position: Point,
    submenu: Option<Submenu>,
}
#[derive(Debug, Clone)]
struct Press {
    target: TaskOccurrence,
    origin: Point,
    dragging: bool,
    selecting: bool,
    hierarchy: bool,
}
#[derive(Debug, Default)]
pub struct State {
    pub selected: Vec<TaskOccurrence>,
    pub modifiers: keyboard::Modifiers,
    pub menu: Option<Menu>,
    pub linking_parent: bool,
    press: Option<Press>,
    pointer: Option<Point>,
}
impl State {
    pub fn track_pointer(&mut self, point: Point) {
        self.pointer = Some(point);
    }
    pub fn cancel_gesture(&mut self) {
        self.press = None;
        self.menu = None;
    }
    pub fn busy(&self) -> bool {
        self.menu.is_some() || self.press.is_some()
    }
    pub fn dragging(&self) -> bool {
        self.press.as_ref().is_some_and(|p| p.dragging)
    }
    fn additive(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.alt()
        } else {
            self.modifiers.control()
        }
    }
    fn select(&mut self, target: TaskOccurrence, toggle: bool) {
        if toggle {
            if let Some(index) = self.selected.iter().position(|t| *t == target) {
                self.selected.remove(index);
            } else {
                self.selected.push(target);
            }
        } else if !self.selected.contains(&target) {
            self.selected = vec![target];
        }
    }
    fn ids(&self) -> Vec<Uuid> {
        let mut ids = Vec::new();
        for target in &self.selected {
            if !ids.contains(&target.item_id) {
                ids.push(target.item_id);
            }
        }
        ids
    }
}

pub fn subscription() -> Subscription<AppMessage> {
    iced::event::listen_with(|event, status, _| subscription_message(event, status))
}
fn subscription_message(event: Event, status: iced::event::Status) -> Option<AppMessage> {
    let message = match event {
        Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) => {
            Message::Modifiers(modifiers)
        }
        Event::Mouse(mouse::Event::CursorMoved { position }) => Message::Move(position),
        // Task rows capture targeted releases. Only empty-space releases need
        // the subscription fallback, so a drop cannot also unlink its target.
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
            if status == iced::event::Status::Ignored =>
        {
            Message::Release(None)
        }
        Event::Window(iced::window::Event::Unfocused) => Message::Cancel,
        _ => return None,
    };
    Some(AppMessage::Tasks(message))
}

pub fn handle(app: &mut App, message: Message) -> Task<AppMessage> {
    if (app.editor.is_some() || app.occurrence.is_some() || app.note_editor.is_some())
        && !matches!(
            message,
            Message::Modifiers(_) | Message::Cancel | Message::Close
        )
    {
        app.interactions.press = None;
        return Task::none();
    }
    match message {
        Message::Modifiers(modifiers) => app.interactions.modifiers = modifiers,
        Message::Move(position) => {
            app.interactions.pointer = Some(position);
            if let Some(press) = &mut app.interactions.press
                && press.hierarchy
                && !press.selecting
                && press.origin.distance(position) >= 5.0
            {
                press.dragging = true;
            }
        }
        Message::Cancel => {
            app.interactions.press = None;
            app.interactions.menu = None;
            app.interactions.modifiers = keyboard::Modifiers::default();
        }
        Message::Close => app.interactions.menu = None,
        Message::Press(target, position) => {
            // Scrollable translates widget cursors into content coordinates;
            // drag origins and menus must use the raw window pointer instead.
            let position = app.interactions.pointer.unwrap_or(position);
            app.interactions.menu = None;
            let selecting = app.interactions.modifiers.shift();
            app.interactions.select(target, selecting);
            app.interactions.pointer = Some(position);
            app.interactions.press = Some(Press {
                target,
                origin: position,
                dragging: false,
                selecting,
                hierarchy: app.page == Page::Outline,
            });
        }
        Message::Release(target) => {
            let Some(press) = app.interactions.press.take() else {
                return Task::none();
            };
            if press.dragging {
                let destination = target.map(|t| t.item_id);
                // A drop back onto the source is a gesture cancellation. The
                // core still rejects actual cycles involving other targets.
                if target == Some(press.target) {
                    return Task::none();
                }
                let additive = app.interactions.additive();
                if let Some(document) = &mut app.document {
                    let result =
                        document.relink_items(&app.interactions.selected, destination, additive);
                    finish(app, result);
                }
            } else if !press.selecting && target == Some(press.target) {
                app.interactions.selected = vec![press.target];
                return app.request(Intent::Select(press.target.item_id));
            }
        }
        Message::Context(target, position) => {
            let position = app.interactions.pointer.unwrap_or(position);
            app.interactions.press = None;
            app.interactions.select(target, false);
            app.interactions.menu = Some(Menu {
                position,
                submenu: None,
            });
        }
        Message::Submenu(submenu) => {
            if let Some(menu) = &mut app.interactions.menu {
                menu.submenu = if menu.submenu == Some(submenu) {
                    None
                } else {
                    Some(submenu)
                };
            }
        }
        Message::Choose(command) => {
            let ids = app.interactions.ids();
            if ids.is_empty() {
                return Task::none();
            }
            app.interactions.menu = None;
            match command {
                Command::Edit if ids.len() == 1 => return app.request(Intent::Select(ids[0])),
                Command::AddChild if ids.len() == 1 => {
                    return app.request(Intent::AddChild(ids[0]));
                }
                Command::LinkNote => {
                    app.error = None;
                    app.assignment_picker = Some(super::notes::AssignmentPicker {
                        tasks: false,
                        filter: Default::default(),
                        priority: String::new(),
                        context_tasks: ids,
                    });
                }
                Command::LinkParent => {
                    app.error = None;
                    app.parent_filter = Default::default();
                    app.parent_priority.clear();
                    app.interactions.linking_parent = true;
                    app.parent_picker = true;
                }
                Command::RemoveParent => {
                    if let Some(document) = &mut app.document {
                        let result = document.relink_items(&app.interactions.selected, None, false);
                        finish(app, result);
                    }
                }
                Command::Apply(action) => {
                    if let Some(document) = &mut app.document {
                        let result = document.apply_task_action(&ids, action);
                        finish(app, result);
                    }
                }
                Command::Edit | Command::AddChild => {}
            }
        }
        Message::LinkParent(parent) => {
            if !app.interactions.linking_parent || !app.parent_picker {
                return Task::none();
            }
            let ids = app.interactions.ids();
            if let Some(document) = &mut app.document {
                let result = document.apply_task_action(&ids, TaskAction::LinkParent(parent));
                if result.is_ok() {
                    app.parent_picker = false;
                    app.interactions.linking_parent = false;
                }
                finish(app, result);
            }
        }
    }
    Task::none()
}
fn finish(app: &mut App, result: crate::core::Result<()>) {
    if result.is_ok() {
        app.interactions.selected.clear();
    }
    app.invalidate_views();
    app.changed(result);
}

pub fn parent_options(app: &App) -> Element<'_, AppMessage> {
    let ids = app.interactions.ids();
    let options = query::filtered(&app.items, &app.parent_filter)
        .into_iter()
        .filter(|item| !ids.contains(&item.id))
        .filter(|item| {
            ids.iter().any(|id| {
                app.items
                    .iter()
                    .find(|i| i.id == *id)
                    .is_some_and(|child| !child.parents.contains(&item.id))
            })
        });
    column(options.map(|item| {
        button(text(&item.name))
            .width(Fill)
            .on_press(AppMessage::Tasks(Message::LinkParent(item.id)))
            .into()
    }))
    .spacing(4)
    .into()
}

fn menu_button(label: impl Into<String>, action: Message) -> Element<'static, AppMessage> {
    button(text(label.into()).size(14))
        .width(Fill)
        .padding([7, 12])
        .style(button::text)
        .on_press(AppMessage::Tasks(action))
        .into()
}
fn command(label: impl Into<String>, command: Command) -> Element<'static, AppMessage> {
    menu_button(label, Message::Choose(command))
}
fn submenu(label: &str, kind: Submenu) -> Element<'static, AppMessage> {
    menu_button(format!("{label}  ›"), Message::Submenu(kind))
}
pub fn context_menu(app: &App, max_height: f32) -> Element<'_, AppMessage> {
    let count = app.interactions.ids().len();
    let mut menu = column![
        text(if count == 1 {
            "Task actions".into()
        } else {
            format!("{count} selected tasks")
        })
        .size(12)
    ]
    .spacing(2);
    if count == 1 {
        menu = menu.push(command("Edit", Command::Edit));
    }
    menu = menu.push(submenu("Status", Submenu::Status));
    if app.interactions.ids().iter().all(|id| {
        app.items
            .iter()
            .find(|item| item.id == *id)
            .is_some_and(|item| item.status != Status::Event)
    }) {
        menu = menu.push(command(
            "Plan for Today",
            Command::Apply(TaskAction::PlanToday),
        ));
    }
    menu = menu
        .push(submenu("Priority", Submenu::Priority))
        .push(submenu("Severity", Submenu::Severity))
        .push(submenu("Toggle Tag", Submenu::Tags));
    if count == 1 {
        menu = menu.push(command("Add Child", Command::AddChild));
    }
    menu = menu
        .push(command("Link a Parent", Command::LinkParent))
        .push(command("Link a Note", Command::LinkNote));
    if app
        .interactions
        .selected
        .iter()
        .any(|t| t.parent_id.is_some())
    {
        menu = menu.push(command("Remove from Parent", Command::RemoveParent));
    }
    menu = menu.push(submenu("Delete", Submenu::Delete));
    let mut content = row![menu_panel(menu.into(), max_height)].spacing(4);
    if let Some(kind) = app.interactions.menu.as_ref().and_then(|m| m.submenu) {
        let choices: Element<'_, AppMessage> = match kind {
            Submenu::Status => column(Status::ALL.into_iter().map(|status| {
                command(
                    status.to_string(),
                    Command::Apply(TaskAction::Status(status)),
                )
            }))
            .into(),
            Submenu::Priority | Submenu::Severity => column((1..=5).map(|value| {
                command(
                    value.to_string(),
                    Command::Apply(if kind == Submenu::Priority {
                        TaskAction::Priority(value)
                    } else {
                        TaskAction::Severity(value)
                    }),
                )
            }))
            .into(),
            Submenu::Tags => {
                if app.tags.is_empty() {
                    container(text("Create tags on the Tags screen.").size(13))
                        .padding(8)
                        .into()
                } else {
                    column(app.tags.iter().map(|tag| {
                        command(
                            tag.name.clone(),
                            Command::Apply(TaskAction::ToggleTag(tag.id)),
                        )
                    }))
                    .into()
                }
            }
            Submenu::Delete => column![
                text(format!(
                    "Delete {count} task{}?",
                    if count == 1 { "" } else { "s" }
                ))
                .size(14),
                command("Confirm delete", Command::Apply(TaskAction::Delete))
            ]
            .spacing(8)
            .into(),
        };
        content = content.push(menu_panel(choices, max_height));
    }
    content.into()
}
fn menu_panel(content: Element<'_, AppMessage>, max_height: f32) -> Element<'_, AppMessage> {
    container(scrollable(content).height(Length::Shrink))
        .max_height(max_height)
        .width(220)
        .padding(8)
        .style(container::bordered_box)
        .into()
}
pub fn overlay(app: &App) -> Element<'_, AppMessage> {
    responsive(move |size| {
        if let Some(menu) = &app.interactions.menu {
            let menu_width = if menu.submenu.is_some() { 444.0 } else { 220.0 };
            let x = menu
                .position
                .x
                .min((size.width - menu_width - 12.0).max(0.0));
            let max_height = (size.height - 16.0).clamp(1.0, 435.0);
            let y = menu
                .position
                .y
                .clamp(8.0, (size.height - max_height - 8.0).max(8.0));
            stack![
                mouse_area(Space::new().width(Fill).height(Fill))
                    .on_press(AppMessage::Tasks(Message::Close))
                    .on_right_press(AppMessage::Tasks(Message::Close)),
                container(opaque(context_menu(app, max_height))).padding(iced::Padding {
                    top: y,
                    left: x,
                    right: 0.0,
                    bottom: 0.0
                })
            ]
            .width(Fill)
            .height(Fill)
            .into()
        } else if app.interactions.dragging() {
            let label = format!(
                "{}{} task{}",
                if app.interactions.additive() {
                    "+ "
                } else {
                    "Move "
                },
                app.interactions.ids().len(),
                if app.interactions.ids().len() == 1 {
                    ""
                } else {
                    "s"
                }
            );
            container(
                container(text(label).size(14))
                    .padding([6, 10])
                    .style(container::bordered_box),
            )
            .padding(iced::Padding {
                left: (app.interactions.pointer.unwrap_or_default().x + 16.0)
                    .min((size.width - 160.0).max(0.0)),
                top: (app.interactions.pointer.unwrap_or_default().y + 16.0)
                    .min((size.height - 40.0).max(0.0)),
                right: 0.0,
                bottom: 0.0,
            })
            .into()
        } else {
            Space::new().into()
        }
    })
    .into()
}

/// Intercepts pointer gestures before the row's visual button. Child tooltips,
/// layout and drawing are delegated unchanged; all screens share these inputs.
pub fn task<'a>(
    app: &App,
    item_id: Uuid,
    parent_id: Option<Uuid>,
    content: impl Into<Element<'a, AppMessage>>,
) -> Element<'a, AppMessage> {
    let target = TaskOccurrence { item_id, parent_id };
    let selected = app.interactions.selected.contains(&target);
    let content = container(content).style(move |theme: &Theme| container::Style {
        border: Border {
            color: theme.palette().primary,
            width: if selected { 2.0 } else { 0.0 },
            radius: 8.0.into(),
        },
        ..Default::default()
    });
    Element::new(TaskSurface {
        content: content.into(),
        target,
        dragging: app.interactions.dragging(),
        additive: app.interactions.additive(),
        hierarchy: app.page == Page::Outline,
    })
}
struct TaskSurface<'a> {
    content: Element<'a, AppMessage>,
    target: TaskOccurrence,
    dragging: bool,
    additive: bool,
    hierarchy: bool,
}
impl Widget<AppMessage, Theme, iced::Renderer> for TaskSurface<'_> {
    fn children(&self) -> Vec<widget::Tree> {
        vec![widget::Tree::new(&self.content)]
    }
    fn diff(&self, tree: &mut widget::Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }
    fn layout(
        &mut self,
        tree: &mut widget::Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }
    fn operate(
        &mut self,
        tree: &mut widget::Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, AppMessage>,
        viewport: &Rectangle,
    ) {
        if let Some(position) = cursor
            .position()
            .filter(|p| layout.bounds().contains(*p) && viewport.contains(*p))
        {
            let message = match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    Some(Message::Press(self.target, position))
                }
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                    Some(Message::Context(self.target, position))
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    Some(Message::Release(Some(self.target)))
                }
                _ => None,
            };
            if let Some(message) = message {
                shell.publish(AppMessage::Tasks(message));
                shell.capture_event();
                return;
            }
        }
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }
    fn mouse_interaction(
        &self,
        _tree: &widget::Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.dragging {
            if self.additive {
                mouse::Interaction::Copy
            } else {
                mouse::Interaction::Grabbing
            }
        } else if cursor.is_over(layout.bounds()) {
            if self.hierarchy {
                mouse::Interaction::Grab
            } else {
                mouse::Interaction::Pointer
            }
        } else {
            mouse::Interaction::None
        }
    }
    fn draw(
        &self,
        tree: &widget::Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }
    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut widget::Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, AppMessage, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Document, Item, Moment};
    use crate::ui::views::render_tests::{Screen, render};
    use chrono::Utc;

    fn fixture(page: Page) -> (App, Uuid, Uuid, Uuid) {
        let mut app = App::empty(Default::default());
        let mut document = Document::new().unwrap();
        let parent = document.save_item(Item::new("Parent")).unwrap().id;
        let mut alpha = Item::new("Alpha");
        let mut beta = Item::new("Beta");
        for item in [&mut alpha, &mut beta] {
            item.planned = Some(Moment::from_utc(Utc::now(), "UTC", true).unwrap());
            item.parents = vec![parent];
        }
        let alpha = document.save_item(alpha).unwrap().id;
        let beta = document.save_item(beta).unwrap().id;
        app.items = document.items().unwrap();
        app.document = Some(document);
        app.page = page;
        app.calendar_timezone = "UTC".into();
        (app, alpha, beta, parent)
    }
    fn position(app: &App, label: &str) -> Point {
        Screen::new(super::super::views::view(app), Size::new(1400.0, 1800.0))
            .snapshot()
            .text
            .iter()
            .rev()
            .find(|(text, _)| text == label)
            .unwrap_or_else(|| panic!("missing text {label}"))
            .1
            .center()
    }
    fn input(app: &mut App, event: Event, point: Point) {
        let (messages, status) = {
            let mut screen = Screen::new(super::super::views::view(app), Size::new(1400.0, 1800.0));
            screen.event_with_status(event.clone(), mouse::Cursor::Available(point))
        };
        for message in messages {
            drop(app.update(message));
        }
        if let Some(message) = subscription_message(event, status) {
            drop(app.update(message));
        }
    }
    fn modifiers(app: &mut App, value: keyboard::Modifiers) {
        input(
            app,
            Event::Keyboard(keyboard::Event::ModifiersChanged(value)),
            Point::ORIGIN,
        );
    }
    fn click(app: &mut App, label: &str, button: mouse::Button) {
        let point = position(app, label);
        input(
            app,
            Event::Mouse(mouse::Event::CursorMoved { position: point }),
            point,
        );
        let events = [
            Event::Mouse(mouse::Event::ButtonPressed(button)),
            Event::Mouse(mouse::Event::ButtonReleased(button)),
        ];
        let outputs = {
            let mut screen = Screen::new(super::super::views::view(app), Size::new(1400.0, 1800.0));
            events
                .iter()
                .map(|event| {
                    screen.event_with_status(event.clone(), mouse::Cursor::Available(point))
                })
                .collect::<Vec<_>>()
        };
        for (event, (messages, status)) in events.into_iter().zip(outputs) {
            for message in messages {
                drop(app.update(message));
            }
            if let Some(message) = subscription_message(event, status) {
                drop(app.update(message));
            }
        }
    }
    fn drag(app: &mut App, source: &str, destination: Option<&str>, additive: bool) {
        let origin = position(app, source);
        let target = destination
            .map(|name| position(app, name))
            .unwrap_or(Point::new(1000.0, 800.0));
        modifiers(
            app,
            if additive {
                if cfg!(target_os = "macos") {
                    keyboard::Modifiers::ALT
                } else {
                    keyboard::Modifiers::CTRL
                }
            } else {
                keyboard::Modifiers::default()
            },
        );
        input(
            app,
            Event::Mouse(mouse::Event::CursorMoved { position: origin }),
            origin,
        );
        input(
            app,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            origin,
        );
        input(
            app,
            Event::Mouse(mouse::Event::CursorMoved { position: target }),
            target,
        );
        assert!(app.interactions.dragging());
        if additive {
            assert!(
                render(overlay(app), &app.theme(), 1400.0)
                    .contains(&format!("+ {} tasks", app.interactions.ids().len()))
            );
        }
        input(
            app,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            target,
        );
        assert!(!app.interactions.dragging());
    }
    fn choose(app: &mut App, command: Command) {
        drop(app.update(AppMessage::Tasks(Message::Choose(command))));
    }

    #[test]
    fn shift_click_and_context_menus_work_on_every_task_screen() {
        for page in [Page::Overview, Page::Tasks, Page::Outline, Page::Calendar] {
            let (mut app, alpha, beta, _) = fixture(page);
            modifiers(&mut app, keyboard::Modifiers::SHIFT);
            click(&mut app, "Alpha", mouse::Button::Left);
            click(&mut app, "Beta", mouse::Button::Left);
            assert_eq!(app.interactions.ids(), [alpha, beta], "{page:?}");
            assert!(app.editor.is_none());
            click(&mut app, "Beta", mouse::Button::Right);
            assert_eq!(app.interactions.ids().len(), 2);
            let menu = render(context_menu(&app, 435.0), &app.theme(), 1400.0);
            assert!(menu.contains("2 selected tasks"));
            assert!(!menu.contains("Edit"));
            assert!(!menu.contains("Add Child"));
            assert!(menu.contains("Plan for Today"));
            assert!(menu.contains("Link a Note"));
            choose(&mut app, Command::Apply(TaskAction::Status(Status::Done)));
            for id in [alpha, beta] {
                let item = app.items.iter().find(|item| item.id == id).unwrap();
                assert_eq!(item.status, Status::Done);
                assert!(item.completion.is_some());
            }
            assert!(app.interactions.selected.is_empty());
        }
    }

    #[test]
    fn normal_click_opens_editor_and_shift_click_can_deselect() {
        let (mut app, alpha, _, _) = fixture(Page::Tasks);
        modifiers(&mut app, keyboard::Modifiers::SHIFT);
        click(&mut app, "Alpha", mouse::Button::Left);
        click(&mut app, "Alpha", mouse::Button::Left);
        assert!(app.interactions.selected.is_empty());
        modifiers(&mut app, keyboard::Modifiers::default());
        click(&mut app, "Alpha", mouse::Button::Left);
        assert_eq!(app.editor.as_ref().unwrap().original.id, alpha);
        drop(app.update(AppMessage::DeleteRequest));
        drop(app.update(AppMessage::DeleteConfirm));
        assert!(
            app.interactions.selected.is_empty(),
            "deleting through the drawer must remove stale selection targets"
        );
    }

    #[test]
    fn hierarchy_drag_moves_multiple_links_and_additive_drag_preserves_them() {
        for additive in [false, true] {
            let (mut app, alpha, beta, parent) = fixture(Page::Outline);
            let destination = app
                .document
                .as_mut()
                .unwrap()
                .save_item(Item::new("Destination"))
                .unwrap()
                .id;
            app.changed(Ok(()));
            modifiers(&mut app, keyboard::Modifiers::SHIFT);
            click(&mut app, "Alpha", mouse::Button::Left);
            click(&mut app, "Beta", mouse::Button::Left);
            drag(&mut app, "Alpha", Some("Destination"), additive);
            assert!(app.error.is_none(), "{:?}", app.error);
            for id in [alpha, beta] {
                let item = app.items.iter().find(|item| item.id == id).unwrap();
                assert!(item.parents.contains(&destination));
                assert_eq!(item.parents.contains(&parent), additive);
            }
            assert!(app.editor.is_none());
        }
    }

    #[test]
    fn hierarchy_empty_drop_unlinks_and_cycles_roll_back_the_whole_selection() {
        let (mut app, alpha, beta, parent) = fixture(Page::Outline);
        modifiers(&mut app, keyboard::Modifiers::SHIFT);
        click(&mut app, "Alpha", mouse::Button::Left);
        click(&mut app, "Beta", mouse::Button::Left);
        let mut descendant = Item::new("Descendant");
        descendant.parents.push(beta);
        app.document
            .as_mut()
            .unwrap()
            .save_item(descendant)
            .unwrap();
        app.changed(Ok(()));
        drag(&mut app, "Alpha", Some("Descendant"), false);
        assert!(app.error.is_some());
        for id in [alpha, beta] {
            assert_eq!(
                app.items.iter().find(|item| item.id == id).unwrap().parents,
                [parent]
            );
        }
        drag(&mut app, "Alpha", None, false);
        assert!(app.error.is_none());
        for id in [alpha, beta] {
            assert!(
                app.items
                    .iter()
                    .find(|item| item.id == id)
                    .unwrap()
                    .parents
                    .is_empty()
            );
        }
    }

    #[test]
    fn context_commands_prefill_children_link_parents_and_hide_event_planning() {
        let (mut app, alpha, beta, parent) = fixture(Page::Outline);
        click(&mut app, "Alpha", mouse::Button::Right);
        choose(&mut app, Command::AddChild);
        assert_eq!(app.editor.as_ref().unwrap().parents, [alpha]);
        assert!(app.editor.as_ref().unwrap().name.is_empty());
        app.editor = None;
        click(&mut app, "Alpha", mouse::Button::Right);
        choose(&mut app, Command::LinkParent);
        assert!(app.parent_picker && app.interactions.linking_parent);
        click(&mut app, "Beta", mouse::Button::Left);
        assert!(!app.parent_picker);
        assert_eq!(
            app.items
                .iter()
                .find(|item| item.id == alpha)
                .unwrap()
                .parents,
            [parent, beta]
        );
        app.interactions.selected = vec![TaskOccurrence {
            item_id: alpha,
            parent_id: Some(parent),
        }];
        choose(&mut app, Command::RemoveParent);
        assert_eq!(
            app.items
                .iter()
                .find(|item| item.id == alpha)
                .unwrap()
                .parents,
            [beta]
        );
        app.items
            .iter_mut()
            .find(|item| item.id == alpha)
            .unwrap()
            .status = Status::Event;
        app.interactions.selected = vec![TaskOccurrence {
            item_id: alpha,
            parent_id: None,
        }];
        assert!(
            !render(context_menu(&app, 435.0), &app.theme(), 1400.0).contains("Plan for Today")
        );
    }
    #[test]
    fn scrolled_task_gestures_and_context_menus_use_window_coordinates() {
        let (mut app, alpha, _, parent) = fixture(Page::Outline);
        let row = task(
            &app,
            alpha,
            Some(parent),
            button("Scrolled task").height(40).width(Fill),
        );
        let content = scrollable(iced::widget::column![
            Space::new().height(600),
            row,
            Space::new().height(400)
        ])
        .height(400)
        .width(500);
        let mut screen = Screen::new(content.into(), Size::new(500.0, 400.0));
        let point = Point::new(100.0, 120.0);
        let cursor = mouse::Cursor::Available(point);
        screen.event(
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -500.0 },
            }),
            cursor,
        );
        let events = [
            Event::Mouse(mouse::Event::CursorMoved { position: point }),
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            Event::Mouse(mouse::Event::CursorMoved {
                position: Point::new(101.0, 120.0),
            }),
        ];
        for event in events {
            let (messages, status) = screen.event_with_status(event.clone(), cursor);
            for message in messages {
                drop(app.update(message));
            }
            if let Some(message) = subscription_message(event, status) {
                drop(app.update(message));
            }
        }
        let press = app.interactions.press.as_ref().unwrap();
        assert_eq!(press.origin, point);
        assert!(
            !press.dragging,
            "one pixel of motion after scrolling is not a drag"
        );
        let right_click = screen.event(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)),
            cursor,
        );
        for message in right_click {
            drop(app.update(message));
        }
        assert_eq!(
            app.interactions.menu.as_ref().unwrap().position,
            Point::new(101.0, 120.0)
        );
    }
    #[test]
    fn navigation_cancels_menus_and_dragging_before_opening_drawers_or_confirmations() {
        let (mut app, alpha, _, parent) = fixture(Page::Outline);
        click(&mut app, "Alpha", mouse::Button::Right);
        assert!(app.interactions.menu.is_some());
        drop(app.update(AppMessage::Key(
            keyboard::Key::Character("n".into()),
            keyboard::Modifiers::COMMAND,
        )));
        assert!(app.editor.is_some());
        assert!(!app.interactions.busy());
        assert!(
            !render(super::super::views::view(&app), &app.theme(), 1400.0).contains("Task actions")
        );
        app.editor = None;
        let origin = position(&app, "Alpha");
        input(
            &mut app,
            Event::Mouse(mouse::Event::CursorMoved { position: origin }),
            origin,
        );
        input(
            &mut app,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            origin,
        );
        let outside = Point::new(1000.0, 800.0);
        input(
            &mut app,
            Event::Mouse(mouse::Event::CursorMoved { position: outside }),
            outside,
        );
        assert!(app.interactions.dragging());
        drop(app.update(AppMessage::Intent(Intent::Open)));
        assert!(app.pending.is_some());
        assert!(!app.interactions.busy());
        drop(app.update(AppMessage::CancelIntent));
        input(
            &mut app,
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            outside,
        );
        assert_eq!(
            app.items
                .iter()
                .find(|item| item.id == alpha)
                .unwrap()
                .parents,
            [parent]
        );
        assert!(app.error.is_none());
    }
    #[test]
    fn changing_filters_clears_hidden_bulk_targets() {
        let (mut app, alpha, beta, _) = fixture(Page::Tasks);
        modifiers(&mut app, keyboard::Modifiers::SHIFT);
        click(&mut app, "Alpha", mouse::Button::Left);
        click(&mut app, "Beta", mouse::Button::Left);
        assert_eq!(app.interactions.ids(), [alpha, beta]);
        drop(app.update(AppMessage::Search("Alpha".into())));
        assert!(app.interactions.selected.is_empty());
        modifiers(&mut app, keyboard::Modifiers::default());
        click(&mut app, "Alpha", mouse::Button::Right);
        choose(&mut app, Command::Apply(TaskAction::Priority(5)));
        assert_eq!(
            app.items
                .iter()
                .find(|item| item.id == alpha)
                .unwrap()
                .priority,
            5
        );
        assert_eq!(
            app.items
                .iter()
                .find(|item| item.id == beta)
                .unwrap()
                .priority,
            3
        );
    }
    #[test]
    fn compact_context_menu_scrolls_lower_actions_and_long_submenus_into_reach() {
        let (mut app, alpha, _, parent) = fixture(Page::Outline);
        app.preferences.font_size = crate::core::preferences::MAX_FONT_SIZE;
        let size = Size::new(880.0 / app.scale(), 620.0 / app.scale());
        app.interactions.selected = vec![TaskOccurrence {
            item_id: alpha,
            parent_id: Some(parent),
        }];
        app.interactions.menu = Some(Menu {
            position: Point::new(size.width - 1.0, size.height - 1.0),
            submenu: None,
        });
        {
            let mut screen = Screen::new(overlay(&app), size);
            screen.draw(&app.theme());
            let snapshot = screen.snapshot();
            let (bounds, content, _) = snapshot.scrolls[0];
            assert!(
                content.height > bounds.height,
                "compact root menu must be scrollable"
            );
            assert!(bounds.x >= 0.0 && bounds.y >= 0.0);
            assert!(bounds.x + bounds.width <= size.width);
            assert!(bounds.y + bounds.height <= size.height);
            screen.event(
                Event::Mouse(mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -2000.0 },
                }),
                mouse::Cursor::Available(bounds.center()),
            );
            let snapshot = screen.snapshot();
            let (bounds, _, translation) = snapshot.scrolls[0];
            let delete = snapshot.bounds("Delete  ›").center() - translation;
            assert!(
                bounds.contains(delete),
                "Delete must be reachable after scrolling"
            );
            let cursor = mouse::Cursor::Available(delete);
            screen.event(
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                cursor,
            );
            assert!(matches!(
                screen
                    .event(
                        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                        cursor
                    )
                    .as_slice(),
                [AppMessage::Tasks(Message::Submenu(Submenu::Delete))]
            ));
        }
        app.tags = (0..40)
            .map(|index| crate::core::Tag::new(format!("Tag {index:02}"), "#123456"))
            .collect();
        app.interactions.menu.as_mut().unwrap().submenu = Some(Submenu::Tags);
        let mut screen = Screen::new(overlay(&app), size);
        screen.draw(&app.theme());
        let snapshot = screen.snapshot();
        assert_eq!(snapshot.scrolls.len(), 2);
        for (bounds, _, _) in &snapshot.scrolls {
            assert!(bounds.x >= 0.0 && bounds.y >= 0.0);
            assert!(bounds.x + bounds.width <= size.width);
            assert!(bounds.y + bounds.height <= size.height);
        }
        let bounds = snapshot.scrolls[1].0;
        screen.event(
            Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -5000.0 },
            }),
            mouse::Cursor::Available(bounds.center()),
        );
        let snapshot = screen.snapshot();
        let (bounds, _, translation) = snapshot.scrolls[1];
        let last_tag = snapshot.bounds("Tag 39").center() - translation;
        assert!(bounds.contains(last_tag));
        let cursor = mouse::Cursor::Available(last_tag);
        screen.event(
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            cursor,
        );
        assert!(
            matches!(screen.event(Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)), cursor).as_slice(), [AppMessage::Tasks(Message::Choose(Command::Apply(TaskAction::ToggleTag(id))))] if *id == app.tags[39].id)
        );
    }
    #[test]
    fn context_menu_links_notes_from_every_task_screen_and_bulk_selection() {
        for page in [Page::Overview, Page::Tasks, Page::Outline, Page::Calendar] {
            let (mut app, alpha, beta, parent) = fixture(page);
            let tag = app
                .document
                .as_mut()
                .unwrap()
                .save_tag(crate::core::Tag::new("Reference", "#123456"))
                .unwrap();
            let mut note = crate::core::Note::new("Shared reference");
            note.tasks = vec![parent];
            note.tags = vec![tag.id];
            let note = app.document.as_mut().unwrap().save_note(note).unwrap();
            let mut linked = crate::core::Note::new("Already linked");
            linked.tasks = vec![alpha, beta];
            app.document.as_mut().unwrap().save_note(linked).unwrap();
            app.changed(Ok(()));
            modifiers(&mut app, keyboard::Modifiers::SHIFT);
            click(&mut app, "Alpha", mouse::Button::Left);
            click(&mut app, "Beta", mouse::Button::Left);
            click(&mut app, "Beta", mouse::Button::Right);
            let messages = {
                let mut screen = Screen::new(context_menu(&app, 600.0), Size::new(700.0, 800.0));
                screen.click("Link a Note")
            };
            for message in messages {
                drop(app.update(message));
            }
            assert!(app.assignment_picker.is_some());
            assert!(app.editor.is_none());
            let snapshot = render(
                super::super::notes::assignment_dialog(&app),
                &app.theme(),
                700.0,
            );
            assert!(snapshot.contains("Shared reference"));
            assert!(!snapshot.contains("Already linked"));
            drop(app.update(AppMessage::Note(
                super::super::notes::Action::AssignmentSearch("missing".into()),
            )));
            assert!(
                render(
                    super::super::notes::assignment_dialog(&app),
                    &app.theme(),
                    700.0
                )
                .contains("No unassigned matches.")
            );
            drop(app.update(AppMessage::Note(
                super::super::notes::Action::AssignmentSearch("Shared".into()),
            )));
            drop(app.update(AppMessage::Note(
                super::super::notes::Action::AssignmentTag(Some(tag.id)),
            )));
            let messages = {
                let mut screen = Screen::new(
                    super::super::notes::assignment_dialog(&app),
                    Size::new(700.0, 700.0),
                );
                screen.click("Shared reference")
            };
            for message in messages {
                drop(app.update(message));
            }
            assert!(app.assignment_picker.is_none());
            assert!(app.interactions.selected.is_empty());
            let tasks = &app
                .notes
                .iter()
                .find(|value| value.id == note.id)
                .unwrap()
                .tasks;
            assert_eq!(tasks, &[parent, alpha, beta]);
        }
    }

    #[test]
    fn context_note_picker_cancellation_and_failure_preserve_tasks() {
        let (mut app, alpha, _, _) = fixture(Page::Tasks);
        let note = app
            .document
            .as_mut()
            .unwrap()
            .save_note(crate::core::Note::new("Link me"))
            .unwrap();
        app.changed(Ok(()));
        click(&mut app, "Alpha", mouse::Button::Right);
        choose(&mut app, Command::LinkNote);
        drop(app.update(AppMessage::Key(
            keyboard::Key::Named(keyboard::key::Named::Escape),
            keyboard::Modifiers::default(),
        )));
        assert!(app.assignment_picker.is_none());
        assert!(app.notes[0].tasks.is_empty());
        choose(&mut app, Command::LinkNote);
        app.document.as_mut().unwrap().delete_item(alpha).unwrap();
        drop(
            app.update(AppMessage::Note(super::super::notes::Action::Assign(
                note.id,
            ))),
        );
        assert!(app.error.is_some());
        assert!(app.assignment_picker.is_some());
        assert!(
            app.document
                .as_ref()
                .unwrap()
                .note(note.id)
                .unwrap()
                .tasks
                .is_empty()
        );
        assert!(
            render(
                super::super::notes::assignment_dialog(&app),
                &app.theme(),
                700.0
            )
            .contains(app.error.as_ref().unwrap())
        );
        drop(app.update(AppMessage::Note(
            super::super::notes::Action::CloseAssignments,
        )));
        assert!(app.assignment_picker.is_none());
    }
}
