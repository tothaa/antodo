use super::app::{App, FilterText, Intent, Message, Page};
use super::datetime::DateTarget;
use super::editor::{DateInput, DateSlot, Field};
use crate::core::query::{self, CalendarKind, DateField};
use crate::core::{Item, Moment, Status, local_timezone};
use chrono::{Datelike, Duration, Local, Utc};
use iced::widget::{
    Column, Row, Space, button, checkbox, column, combo_box, container, mouse_area, opaque,
    pick_list, progress_bar, responsive, row, scrollable, slider, stack, text, text_editor,
    text_input,
};
use iced::{Alignment, Border, Color, Element, Fill, Length, Theme};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Choice<T> {
    value: Option<T>,
    label: String,
}
impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}
pub(super) fn choices<T: Clone + PartialEq + 'static>(
    all: &str,
    options: Vec<(T, String)>,
    current: Option<T>,
    on: impl Fn(Option<T>) -> Message + 'static,
) -> Element<'static, Message> {
    let mut values = vec![Choice {
        value: None,
        label: all.to_owned(),
    }];
    values.extend(options.into_iter().map(|(value, label)| Choice {
        value: Some(value),
        label,
    }));
    let selected = values.iter().find(|c| c.value == current).cloned();
    pick_list(values, selected, move |c| on(c.value))
        .text_size(14)
        .padding(9)
        .into()
}
fn boolean_filter(
    all: &str,
    yes: &str,
    no: &str,
    value: Option<bool>,
    on: impl Fn(Option<bool>) -> Message + 'static,
) -> Element<'static, Message> {
    choices(all, vec![(true, yes.into()), (false, no.into())], value, on)
}
pub(super) fn themed_color(theme: &Theme, dark: [u8; 3], light: [u8; 3]) -> Color {
    let [r, g, b] = if theme.palette().background.r < 0.5 {
        dark
    } else {
        light
    };
    Color::from_rgb8(r, g, b)
}
fn muted_text(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(themed_color(theme, [170, 179, 201], [83, 94, 122])),
    }
}
fn small<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into()).size(13).style(muted_text)
}
fn heading<'a>(value: impl Into<String>) -> iced::widget::Text<'a> {
    text(value.into()).size(22)
}
fn panel<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .padding(20)
        .width(Fill)
        .style(panel_style)
        .into()
}
fn panel_style(theme: &Theme) -> container::Style {
    let p = theme.extended_palette();
    container::Style {
        background: Some(p.background.weak.color.into()),
        border: Border {
            color: p.background.strong.color,
            width: 1.0,
            radius: 16.0.into(),
        },
        ..Default::default()
    }
}
pub(super) fn soft_button(theme: &Theme, status: button::Status) -> button::Style {
    let p = theme.extended_palette();
    button::Style {
        background: Some(
            if matches!(status, button::Status::Hovered | button::Status::Pressed) {
                p.background.strong.color
            } else {
                p.background.base.color
            }
            .into(),
        ),
        text_color: if status == button::Status::Disabled {
            p.secondary.base.color
        } else {
            p.background.base.text
        },
        border: Border {
            radius: 9.0.into(),
            width: 1.0,
            color: p.background.strong.color,
        },
        ..Default::default()
    }
}
fn swatch(color: Color) -> Element<'static, Message> {
    container(Space::new().width(10).height(24))
        .style(move |_| container::Style {
            background: Some(color.into()),
            border: Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
pub(super) fn tag_stripes<'a>(
    app: &App,
    tags: impl IntoIterator<Item = &'a Uuid>,
    width: Length,
    height: Length,
) -> Row<'static, Message> {
    Row::with_children(tags.into_iter().filter_map(|id| {
        let tint = color(&app.tags.iter().find(|t| t.id == *id)?.color);
        Some(
            container(Space::new().width(width).height(height))
                .style(move |_| container::Style {
                    background: Some(tint.into()),
                    ..Default::default()
                })
                .into(),
        )
    }))
}
pub(super) fn color(hex: &str) -> Color {
    let h = hex.trim_start_matches('#');
    if h.len() == 6
        && let Ok(v) = u32::from_str_radix(h, 16)
    {
        return Color::from_rgb8((v >> 16) as u8, (v >> 8) as u8, v as u8);
    }
    Color::from_rgb8(139, 124, 248)
}
pub(super) fn status_color(status: Status) -> Color {
    match status {
        Status::Pending => color("#94A3B8"),
        Status::Wip => color("#9C8BFA"),
        Status::Done => color("#42C9A5"),
        Status::OnHold => color("#DFAD63"),
        Status::WontDo => color("#A58B9B"),
        Status::Someday => color("#6AB7E8"),
        Status::Event => color("#F08CBA"),
    }
}
fn badge(value: impl Into<String>, tint: Color) -> Element<'static, Message> {
    container(text(value.into()).size(12).color(tint))
        .padding([4, 8])
        .style(move |_| container::Style {
            background: Some(Color { a: 0.12, ..tint }.into()),
            border: Border {
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
fn empty<'a>(title: &str, description: &str) -> Element<'a, Message> {
    panel(column![heading(title), small(description)].spacing(8))
}

fn about() -> Element<'static, Message> {
    let link = |label: &'static str, url: &'static str| -> Element<'static, Message> {
        button(text(label).size(15))
            .style(button::text)
            .on_press(Message::OpenUrl(url))
            .into()
    };
    let project = panel(
        column![
            heading("antodo - András' TODO app"),
            small("A personal, local-first task planner."),
            text("Project page").size(16),
            link(
                "github.com/tothaa/antodo",
                "https://github.com/tothaa/antodo"
            )
        ]
        .spacing(12),
    );
    let donations = panel(
        column![
            heading("Support the project"),
            text("Say thank you with a tip to the developer. Donations also motivate me to spend more time on enhancements and bug fixes.")
                .size(15),
            row![
                badge("Featured", Color::from_rgb8(230, 171, 77)),
                link("4Fund · Donate", "https://4fund.com/hu/vhynh2")
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            text("More ways to support").size(16),
            row![
                link("GitHub Sponsors", "https://github.com/sponsors/tothaa"),
                link("Librepay", "https://liberapay.com/tothaa"),
                link("ko-fi", "https://ko-fi.com/tothaa"),
                link("PayPal", "https://paypal.me/tothaaprojects")
            ]
            .spacing(10)
            .wrap()
        ]
        .spacing(14),
    );
    column![project, donations]
        .spacing(18)
        .max_width(760)
        .into()
}

pub fn view(app: &App) -> Element<'_, Message> {
    // Track the full-window background as a drawing primitive. Tiny-skia's
    // surface clear color is shared across swap buffers, so a theme change can
    // otherwise leave older buffers with the previous theme's clear color.
    container(responsive(move |size| view_width(app, size.width)))
        .width(Fill)
        .height(Fill)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.palette().background.into()),
            ..Default::default()
        })
        .into()
}
fn view_width(app: &App, width: f32) -> Element<'_, Message> {
    let narrow = width < 1120.0;
    if app.dialog_open {
        return container(
            column![
                heading("Choose a workspace file"),
                text("Use the system file picker to continue. It may be behind this window."),
                small("Cancel in the file picker to return to your work.")
            ]
            .spacing(14),
        )
        .center_x(Fill)
        .center_y(Fill)
        .into();
    }

    let drawer_open = app.editor.is_some() || app.occurrence.is_some() || app.note_editor.is_some();
    let modal_open = app.pending.is_some()
        || app.picker.is_some()
        || app.assignment_picker.is_some()
        || app.parent_picker
        || app.color_draft.is_some()
        || app.chat_open
        || app.interactions.menu.is_some();
    let mut nav = column![
        text("antodo").size(34).color(app.theme().palette().primary),
        text("Make it so!").size(18).style(muted_text),
        Space::new().height(26)
    ]
    .spacing(8);
    for page in Page::ALL {
        let active = app.page == page;
        nav = nav.push(
            button(text(page.title()).size(16))
                .width(Fill)
                .padding([13, 16])
                .style(if active {
                    button::primary
                } else {
                    button::text
                })
                .on_press_maybe(
                    (!matches!(page, Page::Mcp | Page::Llm) || app.document.is_some())
                        .then_some(Message::Intent(Intent::Switch(page))),
                ),
        );
    }
    let footer = column![small("YOUR WORKSPACE")]
        .push(text(app.filename()).size(14))
        .push(
            small(if app.dirty() {
                "* Unsaved changes"
            } else {
                "Up to date"
            })
            .font(iced::Font {
                style: if app.dirty() {
                    iced::font::Style::Italic
                } else {
                    iced::font::Style::Normal
                },
                ..iced::Font::DEFAULT
            }),
        )
        .push(
            row![
                button("New")
                    .style(soft_button)
                    .on_press(Message::Intent(Intent::New)),
                button("Open")
                    .style(soft_button)
                    .on_press(Message::Intent(Intent::Open))
            ]
            .spacing(6),
        )
        .push(
            row![
                button("Save").on_press_maybe(app.document.as_ref().map(|_| Message::Save)),
                button("Save as")
                    .style(soft_button)
                    .on_press_maybe(app.document.as_ref().map(|_| Message::SaveAs))
            ]
            .spacing(6),
        )
        .push(
            row![
                button("Close").style(soft_button).on_press_maybe(
                    app.document
                        .as_ref()
                        .map(|_| Message::Intent(Intent::CloseFile))
                ),
                button("Quit")
                    .style(soft_button)
                    .on_press(Message::Intent(Intent::Quit))
            ]
            .spacing(6),
        )
        .push(Space::new().height(18))
        .push(
            button(if app.preferences.dark_mode {
                "Switch to light"
            } else {
                "Switch to dark"
            })
            .width(Fill)
            .style(button::text)
            .on_press(Message::Theme),
        )
        .push(
            row![
                button("A−").on_press(Message::Font(-1)).style(soft_button),
                text(format!("{} px", app.preferences.font_size)).size(13),
                button("A+").on_press(Message::Font(1)).style(soft_button)
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        );
    let sidebar = container(column![scrollable(nav).height(Fill), footer].spacing(16))
        .width(if narrow { 175 } else { 218 })
        .height(Fill)
        .padding(20)
        .style(|theme: &Theme| container::Style {
            background: Some(theme.extended_palette().background.weak.color.into()),
            ..Default::default()
        });
    // Keep notifications on the workspace in a permanent slot, preserving
    // page scroll state when alerts arrive or a drawer opens.
    let mut alerts = column![].spacing(16);
    if let Some(message) = feedback(app) {
        alerts = alerts.push(message);
    }
    for (index, reminder) in app.reminders.iter().enumerate() {
        alerts = alerts.push(panel(
            row![
                column![
                    small("REMINDER"),
                    text(&reminder.name),
                    small(format!("{} · {}", reminder.at, reminder.at.timezone))
                ]
                .spacing(4)
                .width(Fill),
                button("View task").on_press(Message::Intent(Intent::Select(reminder.item_id))),
                button("Dismiss")
                    .style(soft_button)
                    .on_press(Message::DismissReminder(index))
            ]
            .spacing(10),
        ));
    }
    let mut body = column![alerts].spacing(16);
    if app.document.is_none() {
        body = body.push(
            container(
                column![
                    badge("A calmer place to plan", app.theme().palette().primary),
                    text("Small steps.\nMeaningful progress.").size(52),
                    text("Bring your tasks, dates, and ideas together in one portable workspace.")
                        .size(18),
                    small("Your tasks stay in a local .antodo file. No account needed."),
                    row![
                        button("Create a workspace")
                            .padding([14, 22])
                            .on_press(Message::Intent(Intent::New)),
                        button("Open a file")
                            .padding([14, 22])
                            .style(soft_button)
                            .on_press(Message::Intent(Intent::Open))
                    ]
                    .spacing(12)
                ]
                .spacing(24),
            )
            .padding(40)
            .center_y(Fill)
            .height(Fill),
        );
    } else {
        body = body.push(
            row![
                column![
                    small(Local::now().format("%A, %d %B %Y").to_string()),
                    text(app.page.title()).size(34),
                    small(if app.page == Page::Overview {
                        app.page.subtitle()
                    } else {
                        ""
                    })
                ]
                .spacing(5)
                .width(Fill),
                {
                    let chat: Element<'_, Message> = if app.llm_config.is_some() {
                        button("Start chat")
                            .on_press(Message::ChatOpen(None))
                            .into()
                    } else {
                        Space::new().into()
                    };
                    chat
                },
                {
                    let control: Element<'_, Message> = if app.page == Page::Notes {
                        button("+ New note")
                            .padding([12, 20])
                            .on_press(Message::Intent(Intent::AddNote))
                            .into()
                    } else if matches!(app.page, Page::Tags | Page::Mcp | Page::Llm | Page::About) {
                        Space::new().into()
                    } else {
                        button("+ New task")
                            .padding([12, 20])
                            .on_press(Message::Intent(Intent::Add))
                            .into()
                    };
                    control
                }
            ]
            .align_y(Alignment::Center),
        );
        if !matches!(
            app.page,
            Page::Notes | Page::Tags | Page::Mcp | Page::Llm | Page::About
        ) {
            body = body.push(filters(app, narrow));
        }
        let content = match app.page {
            Page::Overview => overview(app, narrow),
            Page::Tasks => super::table::view(app),
            Page::Outline => outline(app),
            Page::Calendar => calendar(app),
            Page::Notes => super::notes::view(app),
            Page::Tags => tags(app),
            Page::Mcp => super::mcp::view(app),
            Page::Llm => super::llm::page(app),
            Page::About => about(),
        };
        body = body.push(content);
    }
    let workspace: Element<'_, Message> = row![
        sidebar,
        container(body)
            .width(Fill)
            .height(Fill)
            .padding(if narrow { 16 } else { 28 })
    ]
    .height(Fill)
    .into();
    // Keep the workspace as the first layer, preserving its widget state and
    // scroll position while the drawer opens. Ignore its messages under overlays.
    let mut layers = stack![workspace.map(move |message| {
        if drawer_open || modal_open {
            Message::Noop
        } else {
            message
        }
    })]
    .width(Fill)
    .height(Fill);
    if drawer_open {
        let drawer_width = drawer_width(width);
        let content = if app.note_editor.is_some() {
            super::notes::drawer(app, drawer_width)
        } else if app.occurrence.is_some() {
            occurrence_view(app)
        } else {
            editor_view(app, drawer_width < 600.0)
        };
        let drawer = container(content)
            .width(drawer_width)
            .height(Fill)
            .padding(20)
            .style(|theme: &Theme| container::Style {
                background: Some(theme.palette().background.into()),
                border: Border {
                    color: theme.extended_palette().background.strong.color,
                    width: 1.0,
                    radius: 0.0.into(),
                },
                // Tiny-skia 0.14 paints blurred shadows without its damage
                // clip mask. Partial hover redraws would repeatedly blend the
                // shadow over unchanged pixels. The solid border separates the
                // drawer without drawing outside the refreshed rectangle.
                ..Default::default()
            });
        let layer: Element<'_, Message> = opaque(
            row![
                mouse_area(scrim()).on_press(Message::Intent(Intent::CloseDrawer)),
                drawer
            ]
            .height(Fill),
        );
        layers =
            layers.push(layer.map(move |message| if modal_open { Message::Noop } else { message }));
    }
    if app.chat_open {
        layers = layers.push(modal(super::llm::dialog(app), Message::ChatClose, 850.0));
    } else if app.pending.is_some() {
        layers = layers.push(modal(confirmation(app), Message::CancelIntent, 700.0));
    } else if let Some(value) = &app.color_draft {
        layers = layers.push(modal(color_dialog(value), Message::CancelColor, 500.0));
    } else if app.assignment_picker.is_some() {
        layers = layers.push(modal(
            super::notes::assignment_dialog(app),
            Message::Note(super::notes::Action::CloseAssignments),
            700.0,
        ));
    } else if app.parent_picker {
        layers = layers.push(modal(
            parent_picker(app),
            Message::ParentPicker(false),
            700.0,
        ));
    } else if let Some(picker) = &app.picker {
        layers = layers.push(modal(
            picker.view(),
            Message::DatePicker(super::datetime::PickerAction::Cancel),
            560.0,
        ));
    }
    if !drawer_open
        && !app.parent_picker
        && app.assignment_picker.is_none()
        && !app.chat_open
        && app.pending.is_none()
        && (app.interactions.menu.is_some() || app.interactions.dragging())
    {
        layers = layers.push(super::task_interactions::overlay(app));
    }
    layers.into()
}

fn drawer_width(viewport: f32) -> f32 {
    (viewport * 0.88).min(900.0)
}
fn scrim<'a>() -> Element<'a, Message> {
    container(Space::new())
        .width(Fill)
        .height(Fill)
        .style(|_| container::Style {
            background: Some(Color::from_rgba(0.0, 0.0, 0.0, 0.38).into()),
            ..Default::default()
        })
        .into()
}
fn modal<'a>(content: Element<'a, Message>, dismiss: Message, width: f32) -> Element<'a, Message> {
    stack![
        mouse_area(scrim()).on_press(dismiss),
        container(opaque(
            container(scrollable(
                container(content)
                    .padding(24)
                    .width(Fill)
                    .style(panel_style)
            ))
            .max_width(width)
        ))
        .center_x(Fill)
        .center_y(Fill)
        .padding(12)
    ]
    .width(Fill)
    .height(Fill)
    .into()
}
fn confirmation(app: &App) -> Element<'_, Message> {
    let saves_file = app.pending.as_ref().is_some_and(Intent::leaves_document);
    let mut content = column![
        heading("Keep your changes?"),
        text(if saves_file {
            "Apply and save before continuing, or discard the unsaved changes."
        } else {
            "Apply your edits before continuing, or discard the unapplied changes."
        }),
        row![
            button(if saves_file {
                "Save and continue"
            } else {
                "Apply and continue"
            })
            .on_press(Message::Continue(true)),
            button("Discard changes")
                .style(button::danger)
                .on_press(Message::Continue(false)),
            button("Stay here")
                .style(soft_button)
                .on_press(Message::CancelIntent)
        ]
        .spacing(10)
        .wrap()
    ]
    .spacing(18);
    if let Some(error) = &app.error {
        content = content.push(text(error).style(text::danger));
    }
    content.into()
}
fn feedback(app: &App) -> Option<Element<'_, Message>> {
    let (message, error) = if let Some(error) = &app.error {
        (error, true)
    } else {
        (app.notice.as_ref()?, false)
    };
    Some(
        container(
            row![
                text(message)
                    .style(if error { text::danger } else { text::success })
                    .width(Fill),
                button("Dismiss")
                    .style(button::text)
                    .on_press(Message::Dismiss)
            ]
            .spacing(8),
        )
        .padding(12)
        .style(panel_style)
        .into(),
    )
}

fn filter_input<'a>(app: &'a App, field: FilterText, placeholder: &str) -> Element<'a, Message> {
    text_input(placeholder, &app.filter_values[field as usize])
        .on_input(move |value| Message::FilterText(field, value))
        .padding(8)
        .into()
}
fn filters(app: &App, narrow: bool) -> Element<'_, Message> {
    let statuses = Status::ALL
        .into_iter()
        .map(|s| (s, s.to_string()))
        .collect();
    let tags = app.tags.iter().map(|t| (t.id, t.name.clone())).collect();
    let search = text_input("Search names and descriptions…", &app.filter.search)
        .on_input(Message::Search)
        .padding(10)
        .width(Fill);
    let controls = row![
        choices(
            "Every status",
            statuses,
            app.filter.status,
            Message::StatusFilter
        ),
        choices("Every tag", tags, app.filter.tag, Message::TagFilter),
        button(if app.advanced {
            "Fewer filters"
        } else {
            "More filters"
        })
        .style(soft_button)
        .padding(10)
        .on_press(Message::Advanced),
        button("Reset")
            .style(button::text)
            .on_press(Message::ClearFilters)
    ]
    .spacing(8);
    let mut result = if narrow {
        column![search, controls].spacing(8)
    } else {
        column![row![search, controls].spacing(8)]
    };
    if app.advanced {
        let parents = app.items.iter().map(|i| (i.id, i.name.clone())).collect();
        let dates = DateField::ALL
            .into_iter()
            .map(|d| (d, d.to_string()))
            .collect();
        result = result
            .push(
                row![
                    choices(
                        "Any parent",
                        parents,
                        app.filter.parent,
                        Message::ParentFilter
                    ),
                    filter_input(app, FilterText::Participant, "Participant"),
                    filter_input(app, FilterText::Priority, "Priority 1–5"),
                    filter_input(app, FilterText::Severity, "Severity 1–5"),
                    filter_input(app, FilterText::Effort, "Effort")
                ]
                .spacing(8),
            )
            .push(
                row![
                    choices(
                        "Any date",
                        dates,
                        app.filter.date_field,
                        Message::FilterDate
                    ),
                    filter_input(app, FilterText::From, "From: YYYY-MM-DD"),
                    button("Choose…")
                        .style(soft_button)
                        .on_press(Message::OpenPicker(DateTarget::Filter(false))),
                    filter_input(app, FilterText::To, "To: YYYY-MM-DD"),
                    button("Choose…")
                        .style(soft_button)
                        .on_press(Message::OpenPicker(DateTarget::Filter(true)))
                ]
                .spacing(8),
            )
            .push(
                row![
                    boolean_filter(
                        "All recurrence",
                        "Recurring",
                        "One time",
                        app.filter.recurring,
                        Message::RecurringFilter
                    ),
                    boolean_filter(
                        "All reminders",
                        "Has reminder",
                        "No reminder",
                        app.filter.has_reminder,
                        Message::ReminderFilter
                    ),
                    boolean_filter(
                        "Any notification state",
                        "Notified",
                        "Not notified",
                        app.filter.reminded,
                        Message::RemindedFilter
                    )
                ]
                .spacing(8),
            );
        result = result.push(
            row![
                combo_box(
                    &app.filter_timezones,
                    "Timezone · type to search",
                    if app.filter_values[6].is_empty() {
                        None
                    } else {
                        Some(&app.filter_values[6])
                    },
                    |v| Message::FilterText(FilterText::Timezone, v)
                )
                .on_input(|v| Message::FilterText(FilterText::Timezone, v))
                .padding(8),
                filter_input(app, FilterText::Rule, "Occurs contains (e.g. WEEKLY)"),
                filter_input(app, FilterText::ReminderMinutes, "Reminder minutes")
            ]
            .spacing(8),
        );
    }
    result.into()
}
fn filtered(app: &App) -> Vec<Item> {
    query::filtered(&app.items, &app.filter)
        .into_iter()
        .cloned()
        .collect()
}
fn metric(label: &str, value: impl ToString, foot: &str, tint: Color) -> Element<'static, Message> {
    panel(
        column![
            small(label),
            text(value.to_string()).size(38).color(tint),
            small(foot)
        ]
        .spacing(8),
    )
}
fn overview(app: &App, narrow: bool) -> Element<'_, Message> {
    let dashboard = match app.dashboard_cache.get_or_init(|| {
        query::dashboard_filtered(
            &app.items,
            &app.overrides,
            Utc::now(),
            &local_timezone(),
            7,
            &app.filter,
        )
        .map_err(|e| e.to_string())
    }) {
        Ok(d) => d,
        Err(e) => return empty("Unable to calculate the overview", e),
    };
    let palette = app.theme().palette();
    let [attention, motion, today, progress] = [
        metric(
            "NEEDS ATTENTION",
            dashboard.overdue.len(),
            "Overdue tasks",
            palette.danger,
        ),
        metric(
            "IN MOTION",
            dashboard.ongoing.len(),
            "Active or in progress",
            palette.primary,
        ),
        metric(
            "TODAY",
            dashboard.today.len(),
            "On your agenda",
            palette.warning,
        ),
        metric(
            "PROGRESS",
            format!("{:.0}%", dashboard.percent_complete),
            "Tasks completed",
            palette.success,
        ),
    ];
    let stats: Element<'_, Message> = if narrow {
        column![
            row![attention, motion].spacing(12),
            row![today, progress].spacing(12)
        ]
        .spacing(12)
        .into()
    } else {
        row![attention, motion, today, progress].spacing(12).into()
    };
    use query::DashboardSection as DS;
    let group = |kind: DS| {
        let (title, ids, empty) = match kind {
            DS::Overdue => (
                "Overdue",
                &dashboard.overdue,
                "Nothing overdue. Breathe easy.",
            ),
            DS::Today => (
                "Today's focus",
                &dashboard.today,
                "A little breathing room in your day.",
            ),
            DS::Ongoing => (
                "In progress",
                &dashboard.ongoing,
                "Choose one task to move forward.",
            ),
            DS::NearestDeadlines => (
                "Nearest deadlines",
                &dashboard.nearest_deadlines,
                "No upcoming deadlines.",
            ),
            DS::Upcoming => (
                "The next seven days",
                &dashboard.upcoming,
                "Your upcoming days are clear.",
            ),
        };
        section(app, title, ids, empty, |id| {
            dashboard.representative(kind, id)
        })
    };
    let mut content = column![stats].spacing(20);
    if app.items.is_empty() {
        content = content.push(empty(
            "Your next chapter starts here",
            "Add a task, give it a date, or capture an idea for someday.",
        ));
    } else if narrow {
        for kind in [
            DS::Overdue,
            DS::Today,
            DS::Ongoing,
            DS::NearestDeadlines,
            DS::Upcoming,
        ] {
            content = content.push(group(kind));
        }
    } else {
        content = content
            .push(row![group(DS::Overdue), group(DS::Today)].spacing(16))
            .push(row![group(DS::Ongoing), group(DS::NearestDeadlines)].spacing(16))
            .push(group(DS::Upcoming));
    }
    let report = query::report(&filtered(app), &app.tags);
    content = content.push(overview_progress(&report, narrow));
    if !report.completed.is_empty() {
        content = content.push(section(
            app,
            "Recently closed · latest 5",
            &report.completed[..report.completed.len().min(5)],
            "Completed tasks will appear here.",
            |_| None,
        ));
    }
    scrollable(container(content).padding([0, 8]))
        .id("overview")
        .height(Fill)
        .into()
}
fn overview_progress(report: &query::Report, narrow: bool) -> Element<'static, Message> {
    let mut workload = column![
        heading("Workload & progress"),
        small(format!(
            "{} of {} tasks closed · {} / {} effort points closed",
            report.done + report.wont_do,
            report.total,
            report.completed_effort,
            report.total_effort
        )),
        progress_bar(0.0..=100.0, report.percent_complete).girth(8),
    ]
    .spacing(10);
    for (status, count) in &report.by_status {
        workload = workload.push(
            row![
                text(status.to_string())
                    .size(13)
                    .color(status_color(*status))
                    .width(85),
                progress_bar(0.0..=report.total.max(1) as f32, *count as f32)
                    .girth(6)
                    .length(Fill),
                small(count.to_string()).width(30),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        );
    }
    workload = workload.push(small(
        "Counts each task once. Recurring occurrence progress appears in the agenda and Calendar.",
    ));
    let mut areas = column![heading("Progress by area")].spacing(10);
    let mut any = false;
    for tag in &report.by_tag {
        if tag.total == 0 {
            continue;
        }
        any = true;
        areas = areas.push(
            column![
                row![
                    text(tag.name.clone()).size(14).width(Fill),
                    small(format!("{} / {} closed", tag.done, tag.total)),
                ]
                .spacing(8),
                progress_bar(0.0..=100.0, tag.percent_complete).girth(6),
            ]
            .spacing(5),
        );
    }
    if !any {
        areas = areas.push(small(
            "Assign tags to tasks to see progress by project or area.",
        ));
    }
    areas = areas.push(small("A task can belong to more than one area."));
    if narrow {
        column![panel(workload), panel(areas)].spacing(12).into()
    } else {
        row![panel(workload), panel(areas)].spacing(16).into()
    }
}
fn section<'a>(
    app: &'a App,
    title: &str,
    ids: &[Uuid],
    empty_text: &str,
    entry: impl Fn(Uuid) -> Option<&'a query::CalendarEntry>,
) -> Element<'a, Message> {
    let mut content = column![row![
        heading(title).width(Fill),
        small(ids.len().to_string())
    ]]
    .spacing(10);
    if ids.is_empty() {
        content = content.push(small(empty_text));
    }
    for id in ids {
        if let Some(item) = app.items.iter().find(|i| i.id == *id) {
            content = content.push(task_card(app, item, entry(*id)));
        }
    }
    panel(content)
}
fn task_card<'a>(
    app: &'a App,
    item: &'a Item,
    entry: Option<&'a query::CalendarEntry>,
) -> Element<'a, Message> {
    task_card_at(app, item, entry, None)
}
fn task_card_at<'a>(
    app: &'a App,
    item: &'a Item,
    entry: Option<&'a query::CalendarEntry>,
    parent: Option<Uuid>,
) -> Element<'a, Message> {
    let status = entry.map(|e| e.status).unwrap_or(item.status);
    let name = format!(
        "{}{}",
        item.name,
        if item.parents.len() > 1 {
            " · multiple parents"
        } else {
            ""
        }
    );
    let title = row![
        responsive(
            move |size| text(truncate(&name, (size.width / 8.0).max(1.0) as usize))
                .size(15)
                .wrapping(text::Wrapping::None)
                .into()
        )
        .height(22)
        .width(Fill),
        {
            let deadline: Element<'_, Message> = if app.page == Page::Overview {
                match query::dashboard_deadline(item, entry) {
                    Ok(Some(deadline)) => small(format!("Due {deadline}")).into(),
                    Ok(None) => small("No deadline").into(),
                    Err(error) => text(error.to_string()).size(12).style(text::danger).into(),
                }
            } else {
                Space::new().into()
            };
            deadline
        },
        badge(status.to_string(), status_color(status))
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    let info = column![title].spacing(7);
    let content = row![
        tag_stripes(app, &item.tags, 4.into(), Fill).spacing(2),
        container(info).padding([10, 12]).width(Fill)
    ]
    .height(44);
    super::task_interactions::task(
        app,
        item.id,
        parent,
        button(content)
            .width(Fill)
            .padding(0)
            .style(soft_button)
            .on_press(Message::Intent(Intent::Select(item.id))),
    )
}
fn truncate(value: &str, max: usize) -> String {
    let mut chars = value.chars();
    let mut s: String = chars.by_ref().take(max).collect();
    if chars.next().is_some() {
        s.push('…');
    }
    s
}
fn outline(app: &App) -> Element<'_, Message> {
    let mut content = column![small(
        "Shift-click to select tasks. Drag to move; Ctrl (Option on macOS) adds a parent. Drop on empty space to unlink. Tasks with multiple parents appear under each parent."
    )]
    .spacing(10);
    let rows = query::hierarchy(&app.items);
    let mut count = 0;
    for node in rows {
        if let Some(item) = app.items.iter().find(|i| i.id == node.item_id) {
            if !app.filter.matches(item) {
                continue;
            }
            count += 1;
            content = content.push(
                row![
                    Space::new().width((node.depth.min(24) * 32) as f32),
                    text(if node.depth == 0 {
                        ""
                    } else {
                        ["−", "¤", "+", "•", "◦"][(node.depth - 1) % 5]
                    })
                    .size(18),
                    task_card_at(app, item, None, node.parent_id)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            );
        }
    }
    if count == 0 {
        content = content.push(empty(
            "Everything starts with a first step",
            "Link tasks to parents in the task editor to build a hierarchy.",
        ));
    }
    mouse_area(scrollable(content).height(Fill))
        .on_release(Message::Tasks(super::task_interactions::Message::Release(
            None,
        )))
        .into()
}

fn calendar(app: &App) -> Element<'_, Message> {
    let first = app.month - Duration::days(app.month.weekday().num_days_from_monday() as i64);
    let last = first + Duration::days(42);
    let tz = app.calendar_timezone.clone();
    let from = Moment::parse(&first.to_string(), &tz).and_then(|m| m.utc());
    let to = Moment::parse(&last.to_string(), &tz).and_then(|m| m.utc());
    let (from, to) = match (from, to) {
        (Ok(f), Ok(t)) => (f, t),
        _ => {
            return empty(
                "Calendar unavailable",
                "The display range cannot be resolved in your timezone.",
            );
        }
    };
    let entries = match app.calendar_cache.get_or_init(|| {
        query::calendar_entries_filtered(&app.items, &app.overrides, from, to, &app.filter)
            .map_err(|e| e.to_string())
    }) {
        Ok(e) => e,
        Err(e) => return empty("Unable to expand this calendar", e),
    };
    let mut content=column![row![button("‹").on_press(Message::CalendarMove(-1)).style(soft_button),heading(app.month.format("%B %Y").to_string()).width(Fill),button("Today").on_press(Message::CalendarToday).style(soft_button),button("›").on_press(Message::CalendarMove(1)).style(soft_button)].spacing(12).align_y(Alignment::Center),small(format!("Scheduled · Deadline · Planned — times displayed in {tz}. Click an entry to edit its task."))].spacing(12);
    content = content.push(combo_box(
        &app.calendar_timezones,
        "Calendar timezone",
        Some(&app.calendar_timezone),
        Message::CalendarTimezone,
    ));
    content = content.push(
        Row::with_children(
            ["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"]
                .into_iter()
                .map(|d| container(small(d)).width(Fill).padding(8).into()),
        )
        .spacing(6),
    );
    for week in 0..6 {
        let mut days = Row::new().spacing(6);
        for day in 0..7 {
            let date = first + Duration::days(week * 7 + day);
            let today = Moment::from_utc(Utc::now(), &tz, false)
                .and_then(|m| m.date())
                .ok()
                == Some(date);
            let mut cell = column![text(date.day()).size(14).color(if today {
                app.theme().palette().primary
            } else if date.month() == app.month.month() {
                app.theme().palette().text
            } else {
                app.theme().extended_palette().secondary.base.color
            })]
            .spacing(5);
            for entry in entries {
                let prefix = match calendar_entry_label(entry, date, &tz) {
                    Ok(Some(label)) => label,
                    Ok(None) => continue,
                    Err(error) => {
                        return empty("Unable to display this calendar", &error.to_string());
                    }
                };
                let message = Message::Intent(Intent::Select(entry.item_id));
                let tint = match entry.kind {
                    CalendarKind::Deadline => app.theme().palette().danger,
                    CalendarKind::Planned => app.theme().palette().success,
                    _ => app.theme().palette().primary,
                };
                cell = cell.push(super::task_interactions::task(
                    app,
                    entry.item_id,
                    None,
                    button(
                        column![
                            tag_stripes(
                                app,
                                app.items
                                    .iter()
                                    .find(|i| i.id == entry.item_id)
                                    .into_iter()
                                    .flat_map(|i| &i.tags),
                                Fill,
                                4.into(),
                            )
                            .spacing(2),
                            text(prefix).size(10).color(tint),
                            text(truncate(&entry.name, 40)).size(12),
                            text(entry.status.to_string())
                                .size(10)
                                .color(status_color(entry.status))
                        ]
                        .spacing(2),
                    )
                    .padding(6)
                    .width(Fill)
                    .style(soft_button)
                    .on_press(message),
                ));
            }
            days = days.push(
                mouse_area(
                    container(scrollable(cell).height(Fill))
                        .padding(8)
                        .width(Fill)
                        .height(190)
                        .style(panel_style),
                )
                .on_press(Message::Intent(Intent::AddOn(date))),
            );
        }
        content = content.push(days);
    }
    scrollable(container(content).padding([0, 12]).width(Fill))
        .width(Fill)
        .height(Fill)
        .into()
}
fn calendar_clock(moment: &Moment) -> String {
    let clock = moment.value.get(11..).unwrap_or("");
    let mut label = clock
        .strip_suffix(":00")
        .filter(|_| clock.len() == 8)
        .unwrap_or(clock)
        .to_owned();
    if let Some(offset) = moment.offset_seconds {
        let sign = if offset < 0 { '-' } else { '+' };
        let seconds = offset.unsigned_abs();
        label.push_str(&format!(
            " UTC{sign}{:02}:{:02}",
            seconds / 3600,
            seconds % 3600 / 60,
        ));
        if seconds % 60 != 0 {
            label.push_str(&format!(":{:02}", seconds % 60));
        }
    }
    label
}

/// Format an entry in the calendar's display timezone, including continuation
/// labels for spans. All-day dates keep their original calendar precision.
fn calendar_entry_label(
    entry: &query::CalendarEntry,
    date: chrono::NaiveDate,
    timezone: &str,
) -> crate::core::Result<Option<String>> {
    let original_all_day = entry.start.is_date()
        && entry.start.timezone == timezone
        && entry
            .end
            .as_ref()
            .is_none_or(|e| e.is_date() && e.timezone == timezone);
    let start = query::calendar_moment(&entry.start, timezone, false)?;
    let first = start.date()?;
    let end = entry
        .end
        .as_ref()
        .map(|e| query::calendar_moment(e, timezone, true))
        .transpose()?;
    let last = match entry.end.as_ref() {
        Some(original) if !original.is_date() => {
            Moment::from_utc(original.utc()? - Duration::seconds(1), timezone, false)?
                .date()?
                .max(first)
        }
        Some(_) => end.as_ref().unwrap().date()?.max(first),
        None => first,
    };
    if date < first || date > last {
        return Ok(None);
    }
    let label = match entry.kind {
        CalendarKind::Deadline | CalendarKind::Planned => {
            let kind = if entry.kind == CalendarKind::Deadline {
                "Due"
            } else {
                "Plan"
            };
            if original_all_day {
                format!("{kind} · all day")
            } else {
                format!("{kind} {}", calendar_clock(&start))
            }
        }
        CalendarKind::Scheduled => match end {
            Some(_) if original_all_day => "All day".into(),
            Some(end) => {
                let start_time = if start.is_date() {
                    "Start of day".into()
                } else {
                    calendar_clock(&start)
                };
                let end_time = if end.is_date() {
                    "End of day".into()
                } else {
                    calendar_clock(&end)
                };
                if first == last {
                    format!("{start_time}–{end_time}")
                } else if date == first {
                    format!("{start_time} →")
                } else if date == last {
                    format!("→ {end_time}")
                } else {
                    "Continues".into()
                }
            }
            None if start.is_date() => "All day".into(),
            None => calendar_clock(&start),
        },
    };
    Ok(Some(label))
}

fn tags(app: &App) -> Element<'_, Message> {
    let form = panel(
        column![
            heading(if app.tag_id.is_some() {
                "Edit a tag"
            } else {
                "Create a tag"
            }),
            row![
                text_input("Tag name", &app.tag_name)
                    .on_input(Message::TagName)
                    .padding(12),
                text(&app.tag_color).font(iced::Font::MONOSPACE),
                swatch(color(&app.tag_color)),
                button("Pick color…")
                    .style(soft_button)
                    .on_press(Message::OpenColor),
                button("Save tag").padding(12).on_press(Message::SaveTag)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        ]
        .spacing(12),
    );
    let mut list = column![form].spacing(12);
    for tag in &app.tags {
        let count = app
            .items
            .iter()
            .filter(|i| i.tags.contains(&tag.id))
            .count();
        list = list.push(panel(
            row![
                swatch(color(&tag.color)),
                text(&tag.name).width(Fill),
                small(format!("{count} tasks")),
                button("Edit")
                    .style(soft_button)
                    .on_press(Message::EditTag(tag.id)),
                button("Remove tag")
                    .style(button::text)
                    .on_press(Message::DeleteTag(tag.id))
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ));
    }
    scrollable(list).height(Fill).into()
}
fn input<'a>(
    label: &str,
    placeholder: &str,
    value: &'a str,
    on: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    column![
        small(label),
        text_input(placeholder, value).on_input(on).padding(11)
    ]
    .spacing(6)
    .width(Fill)
    .into()
}
fn date_input<'a>(label: &str, slot: DateSlot, date: &'a DateInput) -> Element<'a, Message> {
    date_control(label, DateTarget::Task(slot), date)
}
fn date_control<'a>(label: &str, target: DateTarget, date: &'a DateInput) -> Element<'a, Message> {
    row![
        small(label).width(80),
        text_input("YYYY-MM-DD HH:MM", &date.value)
            .on_input(move |v| target.message(false, v))
            .padding(9)
            .width(iced::Length::FillPortion(3)),
        combo_box(
            &date.timezones,
            "Timezone…",
            Some(&date.timezone),
            move |v| target.message(true, v)
        )
        .on_input(move |v| target.message(true, v))
        .padding(9)
        .width(iced::Length::FillPortion(2)),
        button("Choose…")
            .style(soft_button)
            .padding(9)
            .on_press(Message::OpenPicker(target))
    ]
    .spacing(8)
    .align_y(Alignment::Center)
    .into()
}

fn editor_view(app: &App, compact: bool) -> Element<'_, Message> {
    let e = app.editor.as_ref().unwrap();
    let mut actions = row![].spacing(8);
    if app.llm_config.is_some() {
        actions =
            actions.push(button("Start chat").on_press(Message::ChatOpen(Some(e.original.id))));
    }
    if e.status != Status::Event {
        actions = actions.push(
            button("Plan for Today")
                .style(soft_button)
                .on_press(Message::PlanToday),
        );
    }
    let actions = actions
        .push(
            row![
                button("Apply changes").on_press(Message::Apply),
                button("Delete").style(button::danger).on_press_maybe(
                    app.items
                        .iter()
                        .any(|i| i.id == e.original.id)
                        .then_some(Message::DeleteRequest)
                )
            ]
            .spacing(8),
        )
        .wrap();
    let header = drawer_header("Edit task", actions.into(), compact);
    let mut content = column![].spacing(20);
    if app.delete_confirm {
        content = content.push(panel(
            column![
                text("Delete this task? Its child tasks will be kept."),
                row![
                    button("Delete task")
                        .style(button::danger)
                        .on_press(Message::DeleteConfirm),
                    button("Keep task")
                        .style(soft_button)
                        .on_press(Message::DeleteCancel)
                ]
                .spacing(10)
            ]
            .spacing(10),
        ));
    }
    content = content.push(panel(
        column![
            input("Name", "What would you like to do?", &e.name, |v| {
                Message::EditField(Field::Name, v)
            }),
            editor_properties(e, compact),
            small("Description · Markdown"),
            text_editor(&e.description)
                .on_action(Message::Description)
                .highlight_with::<super::markdown_editor::MarkdownHighlighter>(
                    app.preferences.dark_mode,
                    super::markdown_editor::format
                )
                .height(app.description_height)
                .padding(12),
            row![
                small("Editor height"),
                slider(
                    100.0..=650.0,
                    app.description_height,
                    Message::DescriptionHeight
                )
                .step(10.0_f32)
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        ]
        .spacing(16),
    ));
    let mut schedule = column![
        heading("Schedule"),
        date_input("Start", DateSlot::Start, &e.start),
        date_input("End", DateSlot::End, &e.end)
    ]
    .spacing(14);
    if e.status != Status::Event {
        schedule = schedule
            .push(date_input("Deadline", DateSlot::Deadline, &e.deadline))
            .push(date_input("Planned", DateSlot::Planned, &e.planned))
            .push(date_input("Completed", DateSlot::Completion, &e.completion));
    }
    let mut repeats = column![
        schedule,
        checkbox(e.recurring)
            .label("Recurring")
            .on_toggle(Message::EditRecurring)
    ]
    .spacing(12);
    if e.recurring {
        repeats=repeats.push(input("Occurs","FREQ=WEEKLY;BYDAY=MO,WE,FR;COUNT=12",&e.rule,|v|Message::EditField(Field::Rule,v))).push(small("RFC 5545: give every series an ending: COUNT=12 or UNTIL=20261231T235959Z. Daily example: FREQ=DAILY;COUNT=10. Changing the schedule resets existing occurrence changes."));
        if e.original.is_recurring {
            if e.dirty {
                repeats = repeats.push(small(
                    "Apply task changes before editing individual occurrences.",
                ));
            }
            let timezone = local_timezone();
            let window = app
                .month
                .checked_add_months(chrono::Months::new(1))
                .ok_or_else(|| crate::core::invalid("Calendar month is out of range"))
                .and_then(|end| {
                    Ok((
                        Moment::parse(&app.month.to_string(), &timezone)?.utc()?,
                        Moment::parse(&end.to_string(), &timezone)?.utc()?,
                    ))
                });
            let occurrences = window.and_then(|(from, to)| {
                crate::core::recurrence::expand(&e.original, &app.overrides, from, to)
            });
            repeats = repeats.push(small(format!(
                "Occurrences · {}",
                app.month.format("%B %Y")
            )));
            match occurrences {
                Ok(occurrences) => {
                    for occurrence in occurrences {
                        repeats = repeats.push(
                            row![
                                small(format!("{} · {}", occurrence.start, occurrence.status))
                                    .width(Fill),
                                button("Edit occurrence").style(soft_button).on_press_maybe(
                                    (!e.dirty)
                                        .then_some(Message::EditOccurrence(Box::new(occurrence)))
                                )
                            ]
                            .spacing(10),
                        );
                    }
                }
                Err(error) => repeats = repeats.push(text(error.to_string()).style(text::danger)),
            }
        }
        for event in app
            .overrides
            .iter()
            .filter(|o| o.item_id == e.original.id && o.cancelled)
        {
            let occurrence = crate::core::Occurrence {
                item_id: event.item_id,
                original_start: event.original_start.clone(),
                original_end: event.original_end.clone(),
                start: event
                    .new_start
                    .clone()
                    .unwrap_or_else(|| event.original_start.clone()),
                end: event
                    .new_end
                    .clone()
                    .unwrap_or_else(|| event.original_end.clone()),
                status: event.status,
                reminded: event.reminded,
                overridden: true,
            };
            repeats = repeats.push(
                row![
                    small(format!("Cancelled: {}", event.original_start)).width(Fill),
                    button("Review / restore")
                        .style(soft_button)
                        .on_press_maybe(
                            (!e.dirty).then_some(Message::EditOccurrence(Box::new(occurrence)))
                        )
                ]
                .spacing(10),
            );
        }
    }
    repeats = repeats.push(input(
        "Reminder · minutes before start, planned time, or deadline; blank disables",
        "15",
        &e.reminder,
        |v| Message::EditField(Field::Reminder, v),
    ));
    content = content.push(panel(repeats));
    let mut tag_options = column![heading("Tags")].spacing(10);
    for tag in &app.tags {
        let id = tag.id;
        tag_options = tag_options.push(
            row![
                swatch(color(&tag.color)),
                checkbox(e.tags.contains(&id))
                    .label(&tag.name)
                    .on_toggle(move |v| Message::ToggleTag(id, v))
            ]
            .spacing(10),
        );
    }
    if app.tags.is_empty() {
        tag_options = tag_options.push(small("Create your first tag in the Tags view."));
    }
    let mut parents = column![
        row![
            heading("Part of").width(Fill),
            button("Add…").on_press(Message::ParentPicker(true))
        ]
        .spacing(10)
    ]
    .spacing(10);
    for item in app.items.iter().filter(|i| e.parents.contains(&i.id)) {
        parents = parents.push(
            row![
                text(&item.name).width(Fill),
                button("Detach")
                    .style(soft_button)
                    .on_press(Message::ToggleParent(item.id, false))
            ]
            .spacing(8),
        );
    }
    let relations: Element<'_, Message> = if compact {
        column![panel(tag_options), panel(parents)]
            .spacing(16)
            .into()
    } else {
        row![panel(tag_options), panel(parents)].spacing(16).into()
    };
    content = content
        .push(panel(super::notes::task_section(app)))
        .push(relations)
        .push(panel(input(
            "Participants",
            "Alice, bob@example.com",
            &e.participants,
            |v| Message::EditField(Field::Participants, v),
        )));
    let drawer = column![header].spacing(16);
    drawer
        .push(
            scrollable(content.padding(8))
                .id("task-editor")
                .height(Fill),
        )
        .height(Fill)
        .into()
}
fn drawer_header<'a>(
    title: &str,
    actions: Element<'a, Message>,
    compact: bool,
) -> Element<'a, Message> {
    let close = button("× Close")
        .style(soft_button)
        .padding([9, 12])
        .on_press(Message::Intent(Intent::CloseDrawer));
    if compact {
        column![
            row![close, small(title).width(Fill)]
                .spacing(10)
                .align_y(Alignment::Center),
            actions
        ]
        .spacing(12)
        .into()
    } else {
        row![close, small(title).width(Fill), actions]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    }
}
fn status_buttons(status: Status, on: impl Fn(Status) -> Message) -> Element<'static, Message> {
    Row::with_children(Status::ALL.into_iter().map(|value| {
        let tint = status_color(value);
        let selected = value == status;
        button(text(value.to_string()).size(14))
            .padding([9, 12])
            .on_press_maybe((!selected).then(|| on(value)))
            .style(move |theme: &Theme, state| {
                let base = theme.palette().background;
                let active =
                    selected || matches!(state, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(
                        if active {
                            tint
                        } else {
                            Color {
                                r: base.r * 0.85 + tint.r * 0.15,
                                g: base.g * 0.85 + tint.g * 0.15,
                                b: base.b * 0.85 + tint.b * 0.15,
                                a: 1.0,
                            }
                        }
                        .into(),
                    ),
                    text_color: if active {
                        Color::from_rgb8(17, 21, 32)
                    } else {
                        theme.palette().text
                    },
                    border: Border {
                        color: tint,
                        width: if selected { 2.0 } else { 1.0 },
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                }
            })
            .into()
    }))
    .spacing(8)
    .wrap()
    .into()
}
fn rating_input<'a>(label: &str, value: &'a str, field: Field) -> Element<'a, Message> {
    let current = value.parse::<u8>().ok();
    column![
        small(label),
        row![
            text_input("3", value)
                .on_input(move |v| Message::EditField(field, v))
                .padding(11)
                .width(Fill),
            Column::with_children([("▲", 1), ("▼", -1)].map(|(label, delta)| {
                button(label)
                    .style(soft_button)
                    .padding([1, 7])
                    .on_press_maybe(
                        current
                            .is_none_or(|v| crate::core::step_rating(v, delta) != v)
                            .then_some(Message::StepRating(field, delta)),
                    )
                    .into()
            }))
            .spacing(2)
        ]
        .spacing(5)
    ]
    .spacing(6)
    .width(Fill)
    .into()
}
fn editor_properties(e: &super::editor::Editor, compact: bool) -> Element<'_, Message> {
    let priority = rating_input("Priority (1–5)", &e.priority, Field::Priority);
    let severity = rating_input("Severity (1–5)", &e.severity, Field::Severity);
    let effort = input("Effort / points", "0", &e.effort, |v| {
        Message::EditField(Field::Effort, v)
    });
    let ratings: Element<'_, Message> = if e.status == Status::Event {
        row![priority, severity].spacing(12).into()
    } else if compact {
        column![row![priority, severity].spacing(12), effort]
            .spacing(12)
            .into()
    } else {
        row![priority, severity, effort].spacing(12).into()
    };
    column![
        small("Status"),
        status_buttons(e.status, Message::EditStatus),
        ratings
    ]
    .spacing(12)
    .into()
}
fn occurrence_view(app: &App) -> Element<'_, Message> {
    let e = app.occurrence.as_ref().unwrap();
    let header = drawer_header(
        "Edit occurrence",
        button("Apply changes")
            .on_press(Message::ApplyOccurrence)
            .into(),
        false,
    );
    let drawer = column![header].spacing(16);
    drawer
        .push(
            scrollable(
                column![
                    heading(&e.name),
                    small(format!(
                        "This occurrence was originally scheduled for {}.",
                        e.occurrence.original_start
                    )),
                    panel(
                        column![
                            date_control("Start", DateTarget::Occurrence(false), &e.start),
                            date_control("End", DateTarget::Occurrence(true), &e.end),
                            row![
                                small("Status"),
                                status_buttons(e.status, Message::OccurrenceStatus)
                            ]
                            .spacing(12)
                            .align_y(Alignment::Center),
                            checkbox(e.cancelled)
                                .label("Cancel this occurrence")
                                .on_toggle(Message::OccurrenceCancel),
                            row![
                                button("Restore original occurrence")
                                    .style(soft_button)
                                    .on_press(Message::ResetOccurrence),
                                button("Edit whole series").style(button::text).on_press(
                                    Message::Intent(Intent::Select(e.occurrence.item_id))
                                )
                            ]
                            .spacing(10)
                            .wrap()
                        ]
                        .spacing(18)
                    )
                ]
                .spacing(20)
                .padding(8),
            )
            .height(Fill),
        )
        .height(Fill)
        .into()
}

fn color_dialog(value: &str) -> Element<'_, Message> {
    let palette = [
        "#EF4444", "#F97316", "#EAB308", "#22C55E", "#14B8A6", "#06B6D4", "#3B82F6", "#8B5CF6",
        "#D946EF", "#EC4899", "#FFFFFF", "#64748B", "#334155", "#000000",
    ];
    column![
        heading("Choose tag color"),
        Row::with_children(palette.into_iter().map(|hex| {
            button(Space::new().width(32).height(28))
                .style(move |_, _| button::Style {
                    background: Some(color(hex).into()),
                    border: Border {
                        color: Color::WHITE,
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                })
                .on_press(Message::ColorDraft(hex.into()))
                .into()
        }))
        .spacing(8)
        .wrap(),
        color_picker(value),
        text(value).font(iced::Font::MONOSPACE).size(20),
        row![
            button("Select").on_press(Message::SelectColor),
            button("Cancel")
                .style(soft_button)
                .on_press(Message::CancelColor)
        ]
        .spacing(12)
    ]
    .spacing(18)
    .into()
}
fn color_picker(value: &str) -> Element<'_, Message> {
    let rgb = color(value).into_rgba8();
    let mut controls = column![swatch(color(value))].spacing(5);
    for (index, label) in ["Red", "Green", "Blue"].into_iter().enumerate() {
        controls = controls.push(
            row![
                small(label).width(45),
                slider(0.0..=255.0, f32::from(rgb[index]), move |v| {
                    let mut next = rgb;
                    next[index] = v.round() as u8;
                    Message::ColorDraft(format!("#{:02X}{:02X}{:02X}", next[0], next[1], next[2]))
                })
                .step(1.0_f32)
            ]
            .spacing(6),
        );
    }
    controls.width(Fill).into()
}
fn parent_picker(app: &App) -> Element<'_, Message> {
    let mut content = column![
        row![
            heading("Add parent").width(Fill),
            button("× Close").on_press(Message::ParentPicker(false))
        ],
        text_input("Search tasks", &app.parent_filter.search).on_input(Message::ParentSearch),
        row![
            choices(
                "Any status",
                Status::ALL
                    .into_iter()
                    .map(|s| (s, s.to_string()))
                    .collect(),
                app.parent_filter.status,
                Message::ParentStatus
            ),
            choices(
                "Any tag",
                app.tags.iter().map(|t| (t.id, t.name.clone())).collect(),
                app.parent_filter.tag,
                Message::ParentTag
            ),
            text_input("Priority", &app.parent_priority).on_input(Message::ParentPriority)
        ]
        .spacing(8)
    ]
    .spacing(12);
    if app.interactions.linking_parent
        && let Some(error) = &app.error
    {
        content = content.push(text(error).size(14).style(text::danger));
    }
    let mut results = Column::new().spacing(4);
    if app.interactions.linking_parent {
        results = results.push(super::task_interactions::parent_options(app));
    } else if let Some(editor) = &app.editor {
        for item in query::filtered(&app.items, &app.parent_filter)
            .into_iter()
            .filter(|i| i.id != editor.original.id && !editor.parents.contains(&i.id))
        {
            results = results.push(
                button(text(&item.name))
                    .width(Fill)
                    .on_press(Message::ToggleParent(item.id, true)),
            );
        }
    }
    content = content.push(scrollable(results).height(340));
    content.into()
}

#[cfg(test)]
mod calendar_tests {
    use super::*;

    fn entry(kind: CalendarKind, start: &str, end: Option<&str>) -> query::CalendarEntry {
        query::CalendarEntry {
            item_id: Uuid::now_v7(),
            name: "Calendar task".into(),
            kind,
            start: Moment::parse(start, "UTC").unwrap(),
            end: end.map(|value| Moment::parse(value, "UTC").unwrap()),
            status: Status::Pending,
            occurrence: None,
        }
    }

    fn label(entry: &query::CalendarEntry, date: &str, timezone: &str) -> Option<String> {
        calendar_entry_label(entry, date.parse().unwrap(), timezone).unwrap()
    }

    #[test]
    fn calendar_shows_timed_spans_and_marker_times_in_display_timezone() {
        let scheduled = entry(
            CalendarKind::Scheduled,
            "2030-01-10 09:00",
            Some("2030-01-10 10:30"),
        );
        assert_eq!(
            label(&scheduled, "2030-01-10", "Europe/Budapest").as_deref(),
            Some("10:00–11:30")
        );
        let due = entry(CalendarKind::Deadline, "2030-01-10 09:15", None);
        assert_eq!(
            label(&due, "2030-01-10", "Europe/Budapest").as_deref(),
            Some("Due 10:15")
        );
        let plan = entry(CalendarKind::Planned, "2030-01-10 23:30", None);
        assert_eq!(label(&plan, "2030-01-10", "Europe/Budapest"), None);
        assert_eq!(
            label(&plan, "2030-01-11", "Europe/Budapest").as_deref(),
            Some("Plan 00:30")
        );
    }

    #[test]
    fn calendar_distinguishes_continuations_and_excludes_midnight_end_date() {
        let scheduled = entry(
            CalendarKind::Scheduled,
            "2030-01-10 09:00",
            Some("2030-01-12 11:30"),
        );
        assert_eq!(
            label(&scheduled, "2030-01-10", "UTC").as_deref(),
            Some("09:00 →")
        );
        assert_eq!(
            label(&scheduled, "2030-01-11", "UTC").as_deref(),
            Some("Continues")
        );
        assert_eq!(
            label(&scheduled, "2030-01-12", "UTC").as_deref(),
            Some("→ 11:30")
        );
        let midnight = entry(
            CalendarKind::Scheduled,
            "2030-01-10 21:00",
            Some("2030-01-11 00:00"),
        );
        assert_eq!(
            label(&midnight, "2030-01-10", "UTC").as_deref(),
            Some("21:00–00:00")
        );
        assert_eq!(label(&midnight, "2030-01-11", "UTC"), None);
    }

    #[test]
    fn calendar_converts_all_day_bounds_and_untimed_markers() {
        let all_day = entry(CalendarKind::Scheduled, "2030-01-10", Some("2030-01-12"));
        assert_eq!(
            label(&all_day, "2030-01-10", "UTC").as_deref(),
            Some("All day")
        );
        assert_eq!(
            label(&all_day, "2030-01-09", "America/Los_Angeles").as_deref(),
            Some("16:00 →")
        );
        assert_eq!(
            label(&all_day, "2030-01-12", "America/Los_Angeles").as_deref(),
            Some("→ 15:59")
        );
        assert_eq!(label(&all_day, "2030-01-13", "America/Los_Angeles"), None);
        let due = entry(CalendarKind::Deadline, "2030-01-10", None);
        assert_eq!(
            label(&due, "2030-01-09", "America/Los_Angeles").as_deref(),
            Some("Due 16:00")
        );
    }

    #[test]
    fn calendar_disambiguates_repeated_clock_times_and_preserves_seconds() {
        let repeated = entry(
            CalendarKind::Scheduled,
            "2030-10-27 00:30",
            Some("2030-10-27 01:30"),
        );
        assert_eq!(
            label(&repeated, "2030-10-27", "Europe/Budapest").as_deref(),
            Some("02:30 UTC+02:00–02:30 UTC+01:00")
        );
        let seconds = entry(CalendarKind::Planned, "2030-01-10 09:15:30", None);
        assert_eq!(
            label(&seconds, "2030-01-10", "UTC").as_deref(),
            Some("Plan 09:15:30")
        );
    }
}

#[cfg(test)]
pub(super) mod render_tests {
    use super::*;
    use iced::advanced::{Layout, layout, renderer, widget};
    use iced::{Event, Rectangle, Renderer, Size, mouse};

    #[derive(Default)]
    pub(crate) struct Snapshot {
        pub text: Vec<(String, Rectangle)>,
        pub inputs: Vec<String>,
        pub scrolls: Vec<(Rectangle, Rectangle, iced::Vector)>,
    }
    impl Snapshot {
        pub fn contains(&self, expected: &str) -> bool {
            self.text.iter().any(|(text, _)| text == expected)
        }
        pub(crate) fn bounds(&self, expected: &str) -> Rectangle {
            self.text
                .iter()
                .find(|(text, _)| text == expected)
                .unwrap_or_else(|| panic!("missing text: {expected}"))
                .1
        }
    }
    impl widget::Operation for Snapshot {
        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn widget::Operation)) {
            operate(self);
        }
        fn scrollable(
            &mut self,
            _: Option<&widget::Id>,
            bounds: Rectangle,
            content_bounds: Rectangle,
            translation: iced::Vector,
            _: &mut dyn widget::operation::Scrollable,
        ) {
            self.scrolls.push((bounds, content_bounds, translation));
        }
        fn text(&mut self, _: Option<&widget::Id>, bounds: Rectangle, text: &str) {
            self.text.push((text.into(), bounds));
        }
        fn text_input(
            &mut self,
            _: Option<&widget::Id>,
            _: Rectangle,
            state: &mut dyn widget::operation::TextInput,
        ) {
            self.inputs.push(state.text().into());
        }
    }

    pub(crate) struct Screen<'a> {
        element: Element<'a, Message>,
        tree: widget::Tree,
        renderer: Renderer,
        layout: layout::Node,
        viewport: Rectangle,
    }
    impl<'a> Screen<'a> {
        pub fn new(mut element: Element<'a, Message>, size: Size) -> Self {
            let renderer = Renderer::new(iced::Font::DEFAULT, 16.into());
            let mut tree = widget::Tree::new(element.as_widget());
            let layout = element.as_widget_mut().layout(
                &mut tree,
                &renderer,
                &layout::Limits::new(Size::ZERO, size),
            );
            fn valid(node: &layout::Node) {
                let bounds = node.bounds();
                for value in [bounds.x, bounds.y, bounds.width, bounds.height] {
                    assert!(value.is_finite(), "non-finite layout: {bounds:?}");
                }
                assert!(bounds.width >= 0.0 && bounds.height >= 0.0);
                node.children().iter().for_each(valid);
            }
            valid(&layout);
            assert!(layout.size().width > 0.0 && layout.size().height > 0.0);
            assert!(layout.size().width <= size.width && layout.size().height <= size.height);
            Self {
                element,
                tree,
                renderer,
                layout,
                viewport: Rectangle::with_size(size),
            }
        }
        pub fn snapshot(&mut self) -> Snapshot {
            let mut snapshot = Snapshot::default();
            self.element.as_widget_mut().operate(
                &mut self.tree,
                Layout::new(&self.layout),
                &self.renderer,
                &mut snapshot,
            );
            snapshot
        }
        pub(crate) fn event(&mut self, event: Event, cursor: mouse::Cursor) -> Vec<Message> {
            self.event_with_status(event, cursor).0
        }
        pub(crate) fn event_with_status(
            &mut self,
            event: Event,
            cursor: mouse::Cursor,
        ) -> (Vec<Message>, iced::event::Status) {
            let mut messages = Vec::new();
            let mut shell = iced::advanced::Shell::new(&mut messages);
            self.element.as_widget_mut().update(
                &mut self.tree,
                &event,
                Layout::new(&self.layout),
                cursor,
                &self.renderer,
                &mut iced::advanced::clipboard::Null,
                &mut shell,
                &self.viewport,
            );
            let status = if shell.is_event_captured() {
                iced::event::Status::Captured
            } else {
                iced::event::Status::Ignored
            };
            (messages, status)
        }
        pub fn draw(&mut self, theme: &Theme) {
            let cursor = mouse::Cursor::Unavailable;
            let messages = self.event(
                Event::Window(iced::window::Event::RedrawRequested(
                    iced::time::Instant::now(),
                )),
                cursor,
            );
            assert!(
                messages
                    .iter()
                    .all(|message| matches!(message, Message::TableScrolled(_) | Message::Noop))
            );
            self.element.as_widget().draw(
                &self.tree,
                &mut self.renderer,
                theme,
                &renderer::Style::default(),
                Layout::new(&self.layout),
                cursor,
                &self.viewport,
            );
        }
        pub(crate) fn quads_with_color(&mut self, color: Color) -> Vec<Rectangle> {
            self.renderer
                .layers()
                .iter()
                .flat_map(|layer| &layer.quads)
                .filter_map(|(quad, background)| {
                    (*background == iced::Background::Color(color)).then_some(quad.bounds)
                })
                .collect()
        }
        pub fn click(&mut self, label: &str) -> Vec<Message> {
            let cursor = mouse::Cursor::Available(self.snapshot().bounds(label).center());
            let mut messages = self.event(
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                cursor,
            );
            messages.extend(self.event(
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
                cursor,
            ));
            messages
        }
    }
    pub(crate) fn render(element: Element<'_, Message>, theme: &Theme, width: f32) -> Snapshot {
        let mut screen = Screen::new(element, Size::new(width, 900.0));
        screen.draw(theme);
        screen.snapshot()
    }

    #[test]
    fn all_pages_layout_and_draw_in_both_themes_and_responsive_widths() {
        let mut app = super::super::preview::test_app();
        app.advanced = true;
        app.show_columns = true;
        app.columns = super::super::table::TaskColumn::ALL.into();
        for dark in [false, true] {
            app.preferences.dark_mode = dark;
            for width in [880.0, 1400.0] {
                for page in Page::ALL {
                    app.page = page;
                    let snapshot = render(view(&app), &app.theme(), width);
                    assert!(snapshot.contains(page.title()), "missing {page:?} heading");
                    assert!(snapshot.contains("antodo"));
                    for item in &app.items {
                        assert!(!snapshot.contains(&item.id.to_string()));
                        assert!(!snapshot.inputs.contains(&item.id.to_string()));
                    }
                }
            }
        }
    }

    #[test]
    fn welcome_file_dialog_and_empty_pages_show_recovery_actions() {
        let mut app = App::empty(Default::default());
        let welcome = render(view(&app), &app.theme(), 880.0);
        assert!(welcome.contains("Create a workspace"));
        assert!(welcome.contains("Open a file"));
        app.dialog_open = true;
        let dialog = render(view(&app), &app.theme(), 1400.0);
        assert!(dialog.contains("Choose a workspace file"));
        assert!(!dialog.contains("Create a workspace"));
        app.dialog_open = false;
        app.document = Some(crate::core::Document::new().unwrap());
        for (page, expected) in [
            (Page::Overview, "Your next chapter starts here"),
            (Page::Outline, "Everything starts with a first step"),
            (Page::Tags, "Create a tag"),
        ] {
            app.page = page;
            assert!(render(view(&app), &app.theme(), 880.0).contains(expected));
        }
        app.page = Page::Calendar;
        app.calendar_timezone = "Invalid/Timezone".into();
        assert!(render(view(&app), &app.theme(), 880.0).contains("Calendar unavailable"));
        app.page = Page::Overview;
        app.dashboard_cache.take();
        app.dashboard_cache
            .set(Err("An invalid recurrence".into()))
            .unwrap();
        assert!(
            render(view(&app), &app.theme(), 880.0).contains("Unable to calculate the overview")
        );
        app.page = Page::Calendar;
        app.calendar_timezone = "UTC".into();
        app.calendar_cache
            .set(Err("An invalid recurrence".into()))
            .unwrap();
        assert!(render(view(&app), &app.theme(), 880.0).contains("Unable to expand this calendar"));
    }

    #[test]
    fn drawers_render_task_event_recurrence_and_occurrence_controls() {
        let mut app = super::super::preview::test_app();
        let task = app
            .items
            .iter()
            .find(|item| item.is_recurring)
            .unwrap()
            .clone();
        let occurrences = crate::core::recurrence::expand(
            &task,
            &[],
            task.start.as_ref().unwrap().utc().unwrap() - Duration::days(1),
            task.end.as_ref().unwrap().utc().unwrap() + Duration::days(10),
        )
        .unwrap();
        app.editor = Some(super::super::editor::Editor::new(task.clone()));
        app.llm_config = Some(Default::default());
        for width in [880.0, 1400.0] {
            let snapshot = render(view(&app), &app.theme(), width);
            for label in [
                "Edit task",
                "Apply changes",
                "Plan for Today",
                "Start chat",
                "Edit occurrence",
            ] {
                assert!(snapshot.contains(label), "missing {label}");
            }
        }
        app.editor.as_mut().unwrap().dirty = true;
        app.delete_confirm = true;
        let snapshot = render(view(&app), &app.theme(), 1400.0);
        assert!(snapshot.contains("Apply task changes before editing individual occurrences."));
        assert!(snapshot.contains("Keep task"));
        app.editor.as_mut().unwrap().status = Status::Event;
        let snapshot = render(editor_view(&app, true), &app.theme(), 600.0);
        for label in [
            "Deadline",
            "Planned",
            "Completed",
            "Effort / points",
            "Plan for Today",
        ] {
            assert!(!snapshot.contains(label), "event showed task field {label}");
        }
        app.editor = None;
        app.occurrence = Some(super::super::editor::OccurrenceEditor::new(
            task.name,
            occurrences[0].clone(),
        ));
        let snapshot = render(view(&app), &app.theme(), 880.0);
        assert!(snapshot.contains("Edit occurrence"));
        assert!(snapshot.contains("Restore original occurrence"));
        assert!(snapshot.contains("Edit whole series"));
    }

    #[test]
    fn modal_content_and_message_routing_preserve_background_state() {
        let mut app = super::super::preview::test_app();
        app.pending = Some(Intent::CloseFile);
        app.error = Some("The selected file cannot be saved".into());
        let snapshot = render(view(&app), &app.theme(), 880.0);
        assert!(snapshot.contains("Save and continue"));
        assert!(snapshot.contains("The selected file cannot be saved"));
        let mut screen = Screen::new(view(&app), Size::new(880.0, 900.0));
        let messages = screen.click("New");
        assert!(messages.iter().any(|m| matches!(m, Message::CancelIntent)));
        assert!(!messages.iter().any(|m| matches!(m, Message::Intent(_))));
        assert!(matches!(
            screen.click("Stay here").as_slice(),
            [Message::CancelIntent]
        ));
        drop(screen);
        app.pending = Some(Intent::CloseDrawer);
        assert!(render(view(&app), &app.theme(), 1400.0).contains("Apply and continue"));
        app.pending = None;
        app.color_draft = Some("#48CFA9".into());
        assert!(render(view(&app), &app.theme(), 880.0).contains("Choose tag color"));
        app.color_draft = None;
        app.editor = Some(super::super::editor::Editor::new(app.items[0].clone()));
        app.parent_picker = true;
        assert!(render(view(&app), &app.theme(), 1400.0).contains("Add parent"));
        app.parent_picker = false;
        let input = DateInput::from(&Some(Moment::parse("2030-01-10 12:30:45", "UTC").unwrap()));
        app.picker = Some(super::super::datetime::DatePicker::new(
            DateTarget::Task(DateSlot::Start),
            &input,
        ));
        app.picker.as_mut().unwrap().error = Some("Invalid local time".into());
        let snapshot = render(view(&app), &app.theme(), 880.0);
        assert!(snapshot.contains("Second"));
        assert!(snapshot.contains("Invalid local time"));
        app.picker = None;
        app.chat_open = true;
        app.chat_context = Some(crate::core::llm::Context::Task(app.items[0].id));
        app.chat.log = vec![("Assistant".into(), "A focused next step".into())];
        app.chat.error = Some("The provider is unavailable".into());
        let snapshot = render(view(&app), &app.theme(), 1400.0);
        assert!(snapshot.contains("Task chat"));
        assert!(snapshot.contains("A focused next step"));
        assert!(snapshot.contains("The provider is unavailable"));
    }
}
