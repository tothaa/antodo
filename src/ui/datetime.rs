//! Presentation-only calendar and clock controls. Core validates chosen moments.
use super::app::Message;
use super::editor::{DateInput, DateSlot};
use crate::core::Moment;
use chrono::{Datelike, Local, Months, NaiveDate, NaiveDateTime, Timelike, Utc};
use iced::widget::{
    Row, button, checkbox, column, combo_box, container, pick_list, row, text, text_input,
};
use iced::{Alignment, Element, Fill};
use std::fmt;

pub fn timezones() -> combo_box::State<String> {
    let mut names: Vec<_> = chrono_tz::TZ_VARIANTS
        .iter()
        .map(ToString::to_string)
        .collect();
    names.sort_unstable();
    combo_box::State::new(names)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateTarget {
    Task(DateSlot),
    Occurrence(bool),
    Filter(bool),
}
impl DateTarget {
    fn label(self) -> &'static str {
        match self {
            Self::Task(DateSlot::Start) | Self::Occurrence(false) => "Start",
            Self::Task(DateSlot::End) | Self::Occurrence(true) => "End",
            Self::Task(DateSlot::Deadline) => "Deadline",
            Self::Task(DateSlot::Planned) => "Planned",
            Self::Task(DateSlot::Completion) => "Completed",
            Self::Filter(false) => "From date",
            Self::Filter(true) => "To date",
        }
    }
    pub fn message(self, zone: bool, value: String) -> Message {
        match self {
            Self::Task(slot) => Message::EditDate(slot, zone, value),
            Self::Occurrence(end) => Message::OccurrenceDate(end, zone, value),
            Self::Filter(end) => Message::FilterText(
                if end {
                    super::app::FilterText::To
                } else {
                    super::app::FilterText::From
                },
                value,
            ),
        }
    }
}

#[derive(Debug, Clone)]
pub enum PickerAction {
    Move(i32),
    Month(u32),
    Year(String),
    Day(NaiveDate),
    Today,
    Timed(bool),
    Hour(u32),
    Minute(u32),
    Second(u32),
    Apply,
    Clear,
    Cancel,
}

pub struct DatePicker {
    pub target: DateTarget,
    date: NaiveDate,
    month: NaiveDate,
    year: String,
    timed: bool,
    hour: u32,
    minute: u32,
    second: u32,
    seconds: bool,
    initial: DateInput,
    pub error: Option<String>,
}
impl DatePicker {
    pub fn new(target: DateTarget, input: &DateInput) -> Self {
        let parsed = input.parse_naive_for_picker().ok();
        let date = parsed
            .map(|(d, _)| d.date())
            .unwrap_or_else(|| today(&input.timezone));
        let timed = parsed.is_some_and(|(_, timed)| timed);
        Self {
            target,
            date,
            month: date.with_day(1).unwrap(),
            year: date.year().to_string(),
            timed: timed && !matches!(target, DateTarget::Filter(_)),
            hour: parsed.map(|(d, _)| d.hour()).unwrap_or(9),
            minute: parsed.map(|(d, _)| d.minute()).unwrap_or(0),
            second: parsed.map(|(d, _)| d.second()).unwrap_or(0),
            seconds: timed && input.value.trim().len() > 16,
            initial: input.clone(),
            error: None,
        }
    }
    pub fn update(&mut self, action: PickerAction) {
        self.error = None;
        match action {
            PickerAction::Move(delta) => {
                let moved = if delta >= 0 {
                    self.month.checked_add_months(Months::new(delta as u32))
                } else {
                    self.month
                        .checked_sub_months(Months::new(delta.unsigned_abs()))
                };
                if let Some(month) = moved {
                    self.month = month;
                    self.year = month.year().to_string();
                }
            }
            PickerAction::Month(month) => {
                if let Some(date) = self.month.with_month(month) {
                    self.month = date;
                }
            }
            PickerAction::Year(value) => {
                self.year = value;
                if let Some(date) = self
                    .year
                    .parse()
                    .ok()
                    .and_then(|year| self.month.with_year(year))
                {
                    self.month = date;
                }
            }
            PickerAction::Day(date) => self.date = date,
            PickerAction::Today => {
                self.date = today(&self.initial.timezone);
                self.month = self.date.with_day(1).unwrap();
                self.year = self.date.year().to_string();
            }
            PickerAction::Timed(timed) if !matches!(self.target, DateTarget::Filter(_)) => {
                self.timed = timed
            }
            PickerAction::Hour(hour) if hour < 24 => self.hour = hour,
            PickerAction::Minute(minute) if minute < 60 => self.minute = minute,
            PickerAction::Second(second) if second < 60 => self.second = second,
            _ => {}
        }
    }
    pub fn value(&self) -> String {
        let naive = self
            .date
            .and_hms_opt(self.hour, self.minute, self.second)
            .unwrap();
        let value = if self.timed {
            naive
                .format(if self.seconds {
                    "%Y-%m-%d %H:%M:%S"
                } else {
                    "%Y-%m-%d %H:%M"
                })
                .to_string()
        } else {
            self.date.to_string()
        };
        // Preserve the exact representation so unchanged generated fold times keep
        // their offset when DateInput::parse_preserving is called by the editor.
        if let Ok(original) = self.initial.parse_naive_for_picker()
            && original == (naive, self.timed)
        {
            return self.initial.value.clone();
        }
        value
    }
    pub fn validate(&self) -> crate::core::Result<()> {
        if self.year.parse::<i32>().ok() != Some(self.month.year()) {
            return Err(crate::core::invalid(
                "Choose a valid year before using this date",
            ));
        }
        let value = self.value();
        if value != self.initial.value {
            Moment::parse(&value, &self.initial.timezone)?;
        }
        Ok(())
    }
    pub fn view(&self) -> Element<'_, Message> {
        let action = |a| Message::DatePicker(a);
        let months: Vec<_> = (1..=12).map(Month).collect();
        let mut content = column![
            text(format!("Choose {}", self.target.label())).size(25),
            row![
                button("‹").on_press(action(PickerAction::Move(-1))),
                pick_list(months, Some(Month(self.month.month())), |m| {
                    Message::DatePicker(PickerAction::Month(m.0))
                })
                .padding(8),
                text_input("Year", &self.year)
                    .on_input(|v| Message::DatePicker(PickerAction::Year(v)))
                    .padding(8)
                    .width(90),
                button("›").on_press(action(PickerAction::Move(1))),
                button("Today")
                    .style(button::secondary)
                    .on_press(action(PickerAction::Today))
            ]
            .spacing(8)
            .align_y(Alignment::Center),
            Row::with_children(["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"].map(|day| {
                container(text(day).size(13))
                    .center_x(Fill)
                    .width(Fill)
                    .into()
            }))
            .spacing(4)
        ]
        .spacing(12);
        let offset = self.month.weekday().num_days_from_monday();
        for week in 0u32..6 {
            let mut days = Row::new().spacing(4);
            for weekday in 0..7 {
                let day = week * 7 + weekday;
                let date = day
                    .checked_sub(offset)
                    .and_then(|day| self.month.with_day(day + 1));
                days = if let Some(date) = date {
                    days.push(
                        button(text(date.day().to_string()).center())
                            .width(Fill)
                            .padding(8)
                            .style(if date == self.date {
                                button::primary
                            } else {
                                button::secondary
                            })
                            .on_press(action(PickerAction::Day(date))),
                    )
                } else {
                    days.push(container(text(" ")).width(Fill).padding(8))
                };
            }
            content = content.push(days);
        }
        content = content.push(text(format!("Selected: {}", self.value())).size(16));
        if !matches!(self.target, DateTarget::Filter(_)) {
            content = content.push(
                checkbox(self.timed)
                    .label("Include a time")
                    .on_toggle(|v| Message::DatePicker(PickerAction::Timed(v))),
            );
            if self.timed {
                let mut clock = row![
                    text("Hour"),
                    clock_input(self.hour, 24, PickerAction::Hour),
                    text("Minute"),
                    clock_input(self.minute, 60, PickerAction::Minute)
                ]
                .spacing(8)
                .align_y(Alignment::Center);
                if self.seconds {
                    clock = clock.push(text("Second")).push(clock_input(
                        self.second,
                        60,
                        PickerAction::Second,
                    ));
                }
                content = content.push(clock);
            }
            content = content.push(text(format!("Timezone: {}", self.initial.timezone)).size(13));
        }
        if let Some(error) = &self.error {
            content = content.push(text(error).style(text::danger));
        }
        content
            .push(
                row![
                    button("Use date").on_press(action(PickerAction::Apply)),
                    button("Clear")
                        .style(button::secondary)
                        .on_press(action(PickerAction::Clear)),
                    button("Cancel")
                        .style(button::text)
                        .on_press(action(PickerAction::Cancel))
                ]
                .spacing(10),
            )
            .into()
    }
}

fn clock_input(
    value: u32,
    limit: u32,
    action: fn(u32) -> PickerAction,
) -> Element<'static, Message> {
    pick_list(
        (0..limit).map(Clock).collect::<Vec<_>>(),
        Some(Clock(value)),
        move |v| Message::DatePicker(action(v.0)),
    )
    .padding(8)
    .into()
}

impl DateInput {
    fn parse_naive_for_picker(&self) -> Result<(NaiveDateTime, bool), chrono::ParseError> {
        let normalized = self.value.trim().replace('T', " ");
        NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S")
            .or_else(|_| NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M"))
            .map(|d| (d, true))
            .or_else(|_| {
                NaiveDate::parse_from_str(&normalized, "%Y-%m-%d")
                    .map(|d| (d.and_hms_opt(9, 0, 0).unwrap(), false))
            })
    }
}
fn today(timezone: &str) -> NaiveDate {
    timezone
        .parse::<chrono_tz::Tz>()
        .map(|tz| Utc::now().with_timezone(&tz).date_naive())
        .unwrap_or_else(|_| Local::now().date_naive())
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Clock(u32);
impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:02}", self.0)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Month(u32);
impl fmt::Display for Month {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(
            [
                "January",
                "February",
                "March",
                "April",
                "May",
                "June",
                "July",
                "August",
                "September",
                "October",
                "November",
                "December",
            ][(self.0 - 1) as usize],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_picker_preserves_seconds_and_explicit_fold_offset() {
        let instant = chrono::DateTime::parse_from_rfc3339("2026-10-25T01:30:27Z")
            .unwrap()
            .with_timezone(&Utc);
        let original = Some(Moment::from_utc(instant, "Europe/Budapest", false).unwrap());
        assert!(original.as_ref().unwrap().offset_seconds.is_some());
        let mut input = DateInput::from(&original);
        let picker = DatePicker::new(DateTarget::Task(DateSlot::Start), &input);
        picker.validate().unwrap();
        input.value = picker.value();
        assert_eq!(input.parse_preserving(&original).unwrap(), original);
    }

    #[test]
    fn picker_selects_leap_day_and_clock_without_changing_timezone() {
        let input = DateInput::from(&Some(Moment::parse("2027-01-31", "Asia/Tokyo").unwrap()));
        let mut picker = DatePicker::new(DateTarget::Task(DateSlot::Deadline), &input);
        picker.update(PickerAction::Year("2028".into()));
        picker.update(PickerAction::Month(2));
        picker.update(PickerAction::Day(
            NaiveDate::from_ymd_opt(2028, 2, 29).unwrap(),
        ));
        assert_eq!(picker.value(), "2028-02-29");
        picker.update(PickerAction::Timed(true));
        picker.update(PickerAction::Hour(23));
        picker.update(PickerAction::Minute(45));
        assert_eq!(picker.value(), "2028-02-29 23:45");
        assert_eq!(picker.initial.timezone, "Asia/Tokyo");
        picker.validate().unwrap();
        picker.update(PickerAction::Move(1));
        assert_eq!(picker.month, NaiveDate::from_ymd_opt(2028, 3, 1).unwrap());
        picker.update(PickerAction::Year("invalid".into()));
        assert!(picker.validate().is_err());
    }

    #[test]
    fn changed_picker_rejects_dst_gaps_and_repeated_times() {
        let input = DateInput::from(&Some(
            Moment::parse("2026-03-28 02:30", "Europe/Budapest").unwrap(),
        ));
        let mut picker = DatePicker::new(DateTarget::Task(DateSlot::Start), &input);
        picker.update(PickerAction::Day(
            NaiveDate::from_ymd_opt(2026, 3, 29).unwrap(),
        ));
        assert!(picker.validate().is_err());
        picker.update(PickerAction::Day(
            NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
        ));
        assert!(picker.validate().is_err());
        picker.update(PickerAction::Hour(4));
        picker.validate().unwrap();
    }

    #[test]
    fn filter_picker_stays_date_only_and_timezone_options_cover_iana_list() {
        let input = DateInput::from(&Some(
            Moment::parse("2026-11-10", "Pacific/Auckland").unwrap(),
        ));
        let mut picker = DatePicker::new(DateTarget::Filter(false), &input);
        picker.update(PickerAction::Timed(true));
        assert_eq!(picker.value(), "2026-11-10");
        assert_eq!(
            input.timezones.options().len(),
            chrono_tz::TZ_VARIANTS.len()
        );
        assert!(
            input
                .timezones
                .options()
                .iter()
                .any(|tz| tz == "Pacific/Auckland")
        );
        assert!(input.timezones.options().iter().any(|tz| tz == "UTC"));
    }

    #[test]
    fn picker_keeps_date_precision_and_uses_morning_when_adding_time() {
        let input = DateInput::from(&Some(Moment::parse("2030-01-10", "UTC").unwrap()));
        let mut picker = DatePicker::new(DateTarget::Task(DateSlot::Start), &input);
        assert_eq!(picker.value(), "2030-01-10");
        picker.update(PickerAction::Timed(true));
        assert_eq!(picker.value(), "2030-01-10 09:00");

        let input = DateInput::from(&Some(Moment::parse("2030-01-10 12:30:45", "UTC").unwrap()));
        let picker = DatePicker::new(DateTarget::Filter(false), &input);
        assert_eq!(picker.value(), "2030-01-10");
    }

    #[test]
    fn picker_reads_iso_input_and_preserves_unchanged_representation() {
        let mut input = DateInput::from(&None);
        input.value = " 2030-01-10T09:15 ".into();
        let mut picker = DatePicker::new(DateTarget::Task(DateSlot::Planned), &input);
        assert_eq!(picker.date, NaiveDate::from_ymd_opt(2030, 1, 10).unwrap());
        assert!(picker.timed);
        assert!(!picker.seconds);
        assert_eq!(picker.value(), input.value);
        picker.update(PickerAction::Minute(30));
        assert_eq!(picker.value(), "2030-01-10 09:30");
    }
    #[test]
    fn calendar_navigation_and_invalid_clock_values_preserve_selection() {
        let input = DateInput::from(&Some(Moment::parse("2030-01-31 12:34:56", "UTC").unwrap()));
        let mut picker = DatePicker::new(DateTarget::Occurrence(true), &input);
        picker.update(PickerAction::Move(-13));
        assert_eq!(picker.month, NaiveDate::from_ymd_opt(2028, 12, 1).unwrap());
        assert_eq!(
            picker.value(),
            input.value,
            "browsing must not silently select a different date"
        );
        for action in [
            PickerAction::Month(0),
            PickerAction::Month(13),
            PickerAction::Move(i32::MIN),
            PickerAction::Hour(24),
            PickerAction::Minute(60),
            PickerAction::Second(60),
        ] {
            picker.update(action);
            assert_eq!(picker.value(), input.value);
        }
        assert_eq!(picker.month, NaiveDate::from_ymd_opt(2028, 12, 1).unwrap());
        picker.update(PickerAction::Second(5));
        assert_eq!(picker.value(), "2030-01-31 12:34:05");
        picker.update(PickerAction::Today);
        assert_eq!(picker.date, today("UTC"));
        assert_eq!(picker.month.day(), 1);
        assert_eq!(picker.year, picker.date.year().to_string());
        picker.validate().unwrap();
    }
}
