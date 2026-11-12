use super::app::{App, Intent, Message};
use crate::core::Item;
use iced::widget::{
    Row, Space, button, checkbox, column, container, keyed_column, responsive, row, scrollable,
    text, tooltip,
};
use iced::{Border, Color, Element, Fill, Theme};
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskColumn {
    Name,
    Description,
    Status,
    Priority,
    Severity,
    Effort,
    Start,
    End,
    Deadline,
    Planned,
    Completed,
    Tags,
    Participants,
}
impl TaskColumn {
    pub const ALL: [Self; 13] = [
        Self::Name,
        Self::Description,
        Self::Status,
        Self::Priority,
        Self::Severity,
        Self::Effort,
        Self::Start,
        Self::End,
        Self::Deadline,
        Self::Planned,
        Self::Completed,
        Self::Tags,
        Self::Participants,
    ];
    pub const DEFAULT: [Self; 5] = [
        Self::Name,
        Self::Description,
        Self::Status,
        Self::Priority,
        Self::Deadline,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Description => "Description",
            Self::Status => "Status",
            Self::Priority => "Priority",
            Self::Severity => "Severity",
            Self::Effort => "Effort",
            Self::Start => "Start",
            Self::End => "End",
            Self::Deadline => "Deadline",
            Self::Planned => "Planned",
            Self::Completed => "Completed",
            Self::Tags => "Tags",
            Self::Participants => "Participants",
        }
    }
    fn date(self, item: &Item) -> Option<&crate::core::Moment> {
        match self {
            Self::Start => item.start.as_ref(),
            Self::End => item.end.as_ref(),
            Self::Deadline => item.deadline.as_ref(),
            Self::Planned => item.planned.as_ref(),
            Self::Completed => item.completion.as_ref(),
            _ => None,
        }
    }
    fn value(self, item: &Item, app: &App) -> String {
        match self {
            Self::Name => item.name.clone(),
            Self::Description => item.description.replace(['\n', '\r'], " "),
            Self::Status => item.status.to_string(),
            Self::Priority => item.priority.to_string(),
            Self::Severity => item.severity.to_string(),
            Self::Effort => item.effort.to_string(),
            Self::Tags => item
                .tags
                .iter()
                .filter_map(|id| app.tags.iter().find(|t| t.id == *id))
                .map(|t| t.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            Self::Participants => item.participants.join(", "),
            _ => self
                .date(item)
                .map(|d| format!("{d} {}", d.timezone))
                .unwrap_or_default(),
        }
    }
    pub fn compare(self, a: &Item, b: &Item, app: &App) -> Ordering {
        match self {
            Self::Priority => a.priority.cmp(&b.priority),
            Self::Severity => a.severity.cmp(&b.severity),
            Self::Effort => a.effort.cmp(&b.effort),
            Self::Start | Self::End | Self::Deadline | Self::Planned | Self::Completed => self
                .date(a)
                .and_then(|d| d.utc().ok())
                .cmp(&self.date(b).and_then(|d| d.utc().ok())),
            _ => self
                .value(a, app)
                .to_lowercase()
                .cmp(&self.value(b, app).to_lowercase()),
        }
    }
}
pub fn view(app: &App) -> Element<'_, Message> {
    let mut content = column![button("Columns…").on_press(Message::Columns)].spacing(8);
    if app.show_columns {
        content = content.push(
            Row::with_children(TaskColumn::ALL.into_iter().map(|c| {
                checkbox(app.columns.contains(&c))
                    .label(c.label())
                    .on_toggle(move |v| Message::ToggleColumn(c, v))
                    .into()
            }))
            .spacing(12)
            .wrap(),
        );
        content = content.push(
            Row::with_children(app.columns.iter().enumerate().map(|(index, column)| {
                row![
                    button("←")
                        .on_press_maybe((index > 0).then_some(Message::MoveColumn(*column, -1)))
                        .style(super::views::soft_button),
                    text(column.label()).size(13),
                    button("→")
                        .on_press_maybe(
                            (index + 1 < app.columns.len())
                                .then_some(Message::MoveColumn(*column, 1))
                        )
                        .style(super::views::soft_button),
                ]
                .spacing(5)
                .align_y(iced::Alignment::Center)
                .into()
            }))
            .spacing(12)
            .wrap(),
        );
    }
    let columns = &app.columns;
    let header = Row::with_children(columns.iter().map(|c| {
        button(
            text(format!(
                "{}{}",
                c.label(),
                if app.sort_column == *c {
                    if app.sort_descending { " ↓" } else { " ↑" }
                } else {
                    ""
                }
            ))
            .size(13),
        )
        .width(Fill)
        .on_press(Message::SortColumn(*c))
        .into()
    }))
    .spacing(2);
    let order = app.table_order.get_or_init(|| sorted_indices(app));
    content
        .push(
            container(row![Space::new().width(24), header.width(Fill)].spacing(8)).padding([0, 8]),
        )
        .push(responsive(move |size| {
            let range = visible_range(order.len(), app.table_offset, size.height);
            let rows = row_elements(app, order, range.clone());
            scrollable(column![
                Space::new().height(range.start as f32 * ROW_PITCH),
                keyed_column(rows).spacing(2),
                Space::new().height(trailing_space(order.len(), range.end)),
            ])
            .height(Fill)
            .id("task-table")
            .on_scroll(Message::TableScrolled)
            .into()
        }))
        .height(Fill)
        .into()
}

fn row_elements<'a>(
    app: &'a App,
    order: &[usize],
    range: std::ops::Range<usize>,
) -> Vec<(uuid::Uuid, Element<'a, Message>)> {
    let columns = &app.columns;
    let mut rows = Vec::with_capacity(range.len());
    for index in range {
        let item = &app.items[order[index]];
        let cells = Row::with_children(columns.iter().map(|c| {
            let value = c.value(item, app);
            let full_value = if *c == TaskColumn::Description {
                item.description.clone()
            } else {
                value.clone()
            };
            let cell = container(
                responsive(move |size| {
                    let limit = (size.width / 8.0).max(1.0) as usize;
                    let mut label: String = value.chars().take(limit.saturating_sub(1)).collect();
                    if value.chars().count() >= limit {
                        label.push('…');
                    }
                    text(label)
                        .size(13)
                        .wrapping(iced::widget::text::Wrapping::None)
                        .into()
                })
                .height(20),
            )
            .width(Fill);
            tooltip(
                cell,
                container(text(full_value).size(14))
                    .max_width(560)
                    .padding(10)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .delay(std::time::Duration::from_millis(350))
            .into()
        }))
        .spacing(8);
        let stripes = super::views::tag_stripes(app, &item.tags, Fill, 20.into())
            .spacing(1)
            .width(24);
        let status = item.status;
        rows.push((
            item.id,
            super::task_interactions::task(
                app,
                item.id,
                None,
                button(row![stripes, cells.width(Fill)].spacing(8))
                    .width(Fill)
                    .height(32)
                    .padding([6, 8])
                    .style(move |theme, state| status_row_style(theme, state, status))
                    .on_press(Message::Intent(Intent::Select(item.id))),
            ),
        ));
    }
    rows
}

const ROW_PITCH: f32 = 34.0;
const OVERSCAN: usize = 12;

fn visible_range(count: usize, offset: f32, height: f32) -> std::ops::Range<usize> {
    let viewport_rows = (height.max(0.0) / ROW_PITCH).ceil() as usize;
    let first =
        ((offset.max(0.0) / ROW_PITCH).floor() as usize).min(count.saturating_sub(viewport_rows));
    first.saturating_sub(OVERSCAN)..(first + viewport_rows + OVERSCAN).min(count)
}
fn trailing_space(count: usize, end: usize) -> f32 {
    count.saturating_sub(end) as f32 * ROW_PITCH
}
fn sorted_indices(app: &App) -> Vec<usize> {
    let mut indices: Vec<_> = app
        .items
        .iter()
        .enumerate()
        .filter(|(_, item)| app.filter.matches(item))
        .map(|(index, _)| index)
        .collect();
    indices.sort_by(|a, b| {
        let a = &app.items[*a];
        let b = &app.items[*b];
        let order = app
            .sort_column
            .compare(a, b, app)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            .then(a.id.cmp(&b.id));
        if app.sort_descending {
            order.reverse()
        } else {
            order
        }
    });
    indices
}

fn status_row_style(
    theme: &Theme,
    state: button::Status,
    status: crate::core::Status,
) -> button::Style {
    let tint = super::views::status_color(status);
    let base = theme.palette().background;
    let amount = if matches!(state, button::Status::Hovered | button::Status::Pressed) {
        0.28
    } else {
        0.13
    };
    // Opaque fills avoid repeated alpha blending during partial redraws.
    let background = Color::from_rgb(
        base.r * (1.0 - amount) + tint.r * amount,
        base.g * (1.0 - amount) + tint.g * amount,
        base.b * (1.0 - amount) + tint.b * amount,
    );
    button::Style {
        background: Some(background.into()),
        text_color: theme.palette().text,
        border: Border {
            color: tint,
            width: 1.0,
            radius: 5.0.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn table_orders_numeric_dates_and_names_and_flattens_descriptions() {
        let app = App::empty(Default::default());
        let mut a = Item::new("alpha");
        let mut b = Item::new("Zulu");
        a.effort = 2;
        b.effort = 10;
        a.description = "First\nSecond\rThird".into();
        a.deadline = Some(crate::core::Moment::parse("2030-01-10 12:00", "Asia/Tokyo").unwrap());
        b.deadline = Some(crate::core::Moment::parse("2030-01-10 06:00", "UTC").unwrap());
        for column in [TaskColumn::Name, TaskColumn::Effort, TaskColumn::Deadline] {
            assert_eq!(column.compare(&a, &b, &app), Ordering::Less);
        }
        assert_eq!(
            TaskColumn::Description.value(&a, &app),
            "First Second Third"
        );
    }
    #[test]
    fn virtual_rows_cover_viewport_keep_full_height_and_bound_work() {
        let count = 10_000;
        for height in [200.0, 600.0, 2000.0] {
            for offset in [0.0, 500.0, 100_000.0, 340_000.0] {
                let range = visible_range(count, offset, height);
                assert!(range.len() <= (height / ROW_PITCH).ceil() as usize + OVERSCAN * 2);
                let actual_offset = offset.min((count as f32 * ROW_PITCH - 2.0 - height).max(0.0));
                assert!(range.start as f32 * ROW_PITCH <= actual_offset);
                assert!(range.end as f32 * ROW_PITCH >= actual_offset + height);
                let total = range.start as f32 * ROW_PITCH + range.len() as f32 * ROW_PITCH - 2.0
                    + trailing_space(count, range.end);
                assert_eq!(total, count as f32 * ROW_PITCH - 2.0);
            }
        }
        assert_eq!(visible_range(0, 9999.0, 600.0), 0..0);
        assert_eq!(visible_range(3, 9999.0, 600.0), 0..3);
    }

    #[test]
    fn table_filters_sorts_reversed_ties_and_renders_all_column_values() {
        use super::super::views::render_tests::{Screen, render};
        let mut app = App::empty(Default::default());
        let tag = crate::core::Tag::new("Focused", "#48CFA9");
        app.tags.push(tag.clone());
        let mut first = Item::new("alpha");
        first.description = "Two\nlines".into();
        first.tags = vec![tag.id];
        first.participants = vec!["Ada".into(), "Grace".into()];
        first.status = crate::core::Status::Wip;
        first.effort = 2;
        first.start = Some(crate::core::Moment::parse("2030-01-10", "UTC").unwrap());
        first.end = first.start.clone();
        first.planned = first.start.clone();
        first.completion = first.start.clone();
        let mut second = Item::new("Beta");
        second.effort = 10;
        app.items = vec![second.clone(), first.clone()];
        app.sort_column = TaskColumn::Effort;
        assert_eq!(sorted_indices(&app), vec![1, 0]);
        app.sort_descending = true;
        assert_eq!(sorted_indices(&app), vec![0, 1]);
        app.filter.tag = Some(tag.id);
        assert_eq!(sorted_indices(&app), vec![1]);
        app.filter = Default::default();
        app.sort_descending = false;
        app.columns = TaskColumn::ALL.into();
        assert_eq!(TaskColumn::Tags.value(&first, &app), "Focused");
        assert_eq!(TaskColumn::Participants.value(&first, &app), "Ada, Grace");
        for column in [
            TaskColumn::Start,
            TaskColumn::End,
            TaskColumn::Planned,
            TaskColumn::Completed,
        ] {
            assert_eq!(column.value(&first, &app), "2030-01-10 UTC");
            assert_eq!(column.compare(&second, &first, &app), Ordering::Less);
        }
        let snapshot = render(view(&app), &app.theme(), 1400.0);
        assert!(snapshot.contains("Two lines"));
        assert!(snapshot.contains("Focused"));
        let mut screen = Screen::new(view(&app), iced::Size::new(1400.0, 900.0));
        let task_click = screen.click("alpha");
        assert!(
            matches!(task_click.as_slice(), [Message::Tasks(super::super::task_interactions::Message::Press(target, _)), Message::Tasks(super::super::task_interactions::Message::Release(Some(released)))] if target.item_id == first.id && target == released)
        );
        assert!(matches!(
            screen.click("Name").as_slice(),
            [Message::SortColumn(TaskColumn::Name)]
        ));
        drop(screen);
        for message in task_click {
            drop(app.update(message));
        }
        assert_eq!(app.editor.as_ref().unwrap().original.id, first.id);
        app.editor = None;
        app.filter.search = "missing".into();
        app.table_order.take();
        assert!(!render(view(&app), &app.theme(), 1400.0).contains("alpha"));
    }

    #[test]
    fn status_rows_remain_opaque_and_interactive_in_both_themes() {
        for theme in [Theme::Dark, Theme::Light] {
            for status in crate::core::Status::ALL {
                let normal = status_row_style(&theme, button::Status::Active, status);
                let hover = status_row_style(&theme, button::Status::Hovered, status);
                assert_ne!(normal.background, hover.background);
                for style in [
                    normal,
                    hover,
                    status_row_style(&theme, button::Status::Pressed, status),
                ] {
                    assert!(
                        matches!(style.background, Some(iced::Background::Color(color)) if color.a == 1.0)
                    );
                    assert_eq!(style.text_color, theme.palette().text);
                    assert_eq!(
                        style.border.color,
                        super::super::views::status_color(status)
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "manual widget-construction benchmark"]
    fn benchmark_large_table_construction() {
        let mut app = App::empty(Default::default());
        app.items = (0..10_000)
            .map(|i| {
                let mut item = Item::new(format!("Task {i:05}"));
                item.description = "A description with details and notes".repeat(4);
                item
            })
            .collect();
        let order = sorted_indices(&app);
        let start = std::time::Instant::now();
        drop(std::hint::black_box(row_elements(
            &app,
            &order,
            0..order.len(),
        )));
        let full = start.elapsed();
        let range = visible_range(order.len(), 100_000.0, 600.0);
        let visible_count = range.len();
        let start = std::time::Instant::now();
        for _ in 0..100 {
            drop(std::hint::black_box(row_elements(
                &app,
                &order,
                range.clone(),
            )));
        }
        println!(
            "10,000 rows: full construction {:?}; {} visible/overscan rows: {:?} average (100 runs)",
            full,
            visible_count,
            start.elapsed() / 100
        );
    }
}
