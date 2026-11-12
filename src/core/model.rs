use super::{Result, invalid};
use chrono::{DateTime, Duration, LocalResult, NaiveDate, NaiveDateTime, Offset, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Step a priority or severity rating, retaining its domain bounds.
pub fn step_rating(value: u8, delta: i8) -> u8 {
    (i16::from(value) + i16::from(delta)).clamp(1, 5) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Status {
    #[default]
    Pending,
    Wip,
    Done,
    OnHold,
    WontDo,
    Someday,
    Event,
}
impl Status {
    pub const ALL: [Self; 7] = [
        Self::Pending,
        Self::Wip,
        Self::Done,
        Self::OnHold,
        Self::WontDo,
        Self::Someday,
        Self::Event,
    ];
    pub fn is_closed(self) -> bool {
        matches!(self, Self::Done | Self::WontDo)
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Wip => "WIP",
            Self::Done => "Done",
            Self::OnHold => "On-Hold",
            Self::WontDo => "Won't Do",
            Self::Someday => "Someday",
            Self::Event => "Event",
        }
    }
    pub fn parse(value: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.key() == value)
            .ok_or_else(|| invalid("Unknown status"))
    }
}
impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// A user's local date or wall-clock time, with precision and timezone preserved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Moment {
    pub value: String,
    pub timezone: String,
    /// Retains the selected instant when a generated wall clock repeats at DST.
    /// Direct user input rejects ambiguous times and leaves this unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset_seconds: Option<i32>,
}
impl Moment {
    pub fn parse(value: &str, timezone: &str) -> Result<Self> {
        let moment = Self {
            value: value.trim().replace('T', " "),
            timezone: timezone.trim().to_string(),
            offset_seconds: None,
        };
        let tz: chrono_tz::Tz = moment
            .timezone
            .parse()
            .map_err(|_| invalid("Use a valid IANA timezone, for example Europe/Budapest"))?;
        if moment.is_date() {
            moment.utc()?;
        } else if tz.from_local_datetime(&moment.naive()?).single().is_none() {
            return Err(invalid(
                "This local time is ambiguous or does not exist because of a clock change; choose another time",
            ));
        }
        Ok(moment)
    }
    pub fn is_date(&self) -> bool {
        self.value.len() == 10
    }
    pub fn naive(&self) -> Result<NaiveDateTime> {
        if self.is_date() {
            return NaiveDate::parse_from_str(&self.value, "%Y-%m-%d")
                .map(|d| d.and_hms_opt(0, 0, 0).unwrap())
                .map_err(|_| invalid("Use YYYY-MM-DD for dates"));
        }
        NaiveDateTime::parse_from_str(&self.value, "%Y-%m-%d %H:%M")
            .or_else(|_| NaiveDateTime::parse_from_str(&self.value, "%Y-%m-%d %H:%M:%S"))
            .map_err(|_| invalid("Use YYYY-MM-DD or YYYY-MM-DD HH:MM for dates"))
    }
    pub fn utc(&self) -> Result<DateTime<Utc>> {
        let tz: chrono_tz::Tz = self
            .timezone
            .parse()
            .map_err(|_| invalid("Use a valid IANA timezone, for example Europe/Budapest"))?;
        let naive = self.naive()?;
        if self.is_date() {
            if self.offset_seconds.is_some() {
                return Err(invalid("A date-only value cannot contain a time offset"));
            }
            // A calendar date can exist even when local midnight was skipped.
            // Find its first valid instant without turning the date into a time.
            for minute in 0..1440 {
                let candidate = naive
                    .checked_add_signed(Duration::minutes(minute))
                    .ok_or_else(|| invalid("Date exceeds the supported range"))?;
                if let Some(value) = tz.from_local_datetime(&candidate).earliest() {
                    if minute == 0 {
                        return Ok(value.with_timezone(&Utc));
                    }
                    // Historical timezone transitions can occur between minutes.
                    for second in 1..=60 {
                        let exact = candidate - Duration::seconds(60 - second);
                        if let Some(value) = tz.from_local_datetime(&exact).earliest() {
                            return Ok(value.with_timezone(&Utc));
                        }
                    }
                }
            }
            return Err(invalid("This calendar date does not exist in its timezone"));
        }
        let matches_offset = |value: &DateTime<chrono_tz::Tz>| {
            self.offset_seconds
                .is_none_or(|offset| value.offset().fix().local_minus_utc() == offset)
        };
        let resolved = match tz.from_local_datetime(&naive) {
            LocalResult::Single(value) if matches_offset(&value) => Some(value),
            LocalResult::Ambiguous(first, second) if self.offset_seconds.is_some() => {
                [first, second].into_iter().filter(matches_offset).min()
            }
            _ => None,
        };
        resolved.map(|value| value.with_timezone(&Utc)).ok_or_else(|| invalid(
            "This local time or its saved timezone offset is invalid because of a clock change"
        ))
    }

    pub fn date(&self) -> Result<NaiveDate> {
        Ok(self.naive()?.date())
    }

    /// Compare saved dates independently of optional seconds in their text.
    pub(crate) fn equivalent(&self, other: &Self) -> bool {
        self.timezone == other.timezone
            && self.is_date() == other.is_date()
            && self.utc().ok() == other.utc().ok()
    }

    pub fn from_utc(value: DateTime<Utc>, timezone: &str, date_only: bool) -> Result<Self> {
        let tz: chrono_tz::Tz = timezone.parse().map_err(|_| invalid("Invalid timezone"))?;
        let local = value.with_timezone(&tz);
        let offset_seconds = if !date_only
            && matches!(
                tz.from_local_datetime(&local.naive_local()),
                LocalResult::Ambiguous(_, _)
            ) {
            Some(local.offset().fix().local_minus_utc())
        } else {
            None
        };
        Ok(Self {
            value: local
                .format(if date_only {
                    "%Y-%m-%d"
                } else {
                    "%Y-%m-%d %H:%M:%S"
                })
                .to_string(),
            timezone: timezone.to_owned(),
            offset_seconds,
        })
    }
}
impl fmt::Display for Moment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}
pub fn local_timezone() -> String {
    iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".into())
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub start: Option<Moment>,
    pub end: Option<Moment>,
    pub deadline: Option<Moment>,
    pub status: Status,
    pub tags: Vec<Uuid>,
    pub parents: Vec<Uuid>,
    pub is_recurring: bool,
    pub rrule: String,
    pub participants: Vec<String>,
    pub reminder: Option<u32>,
    pub reminded: bool,
    pub priority: u8,
    pub severity: u8,
    pub effort: u32,
    pub completion: Option<Moment>,
    pub planned: Option<Moment>,
}
impl Item {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            description: String::new(),
            start: None,
            end: None,
            deadline: None,
            status: Status::Pending,
            tags: vec![],
            parents: vec![],
            is_recurring: false,
            rrule: String::new(),
            participants: vec![],
            reminder: None,
            reminded: false,
            priority: 3,
            severity: 3,
            effort: 0,
            completion: None,
            planned: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub name: String,
    /// Markdown source, preserved exactly as entered.
    pub content: String,
    #[serde(default)]
    pub tags: Vec<Uuid>,
    #[serde(default)]
    pub tasks: Vec<Uuid>,
}
impl Note {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            content: String::new(),
            tags: vec![],
            tasks: vec![],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tag {
    pub id: Uuid,
    pub name: String,
    pub color: String,
}
impl Tag {
    pub fn new(name: impl Into<String>, color: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            color: color.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventOverride {
    pub id: Uuid,
    pub item_id: Uuid,
    pub original_start: Moment,
    pub original_end: Moment,
    pub new_start: Option<Moment>,
    pub new_end: Option<Moment>,
    pub cancelled: bool,
    pub status: Status,
    pub reminded: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Occurrence {
    pub item_id: Uuid,
    pub original_start: Moment,
    pub original_end: Moment,
    pub start: Moment,
    pub end: Moment,
    pub status: Status,
    pub reminded: bool,
    pub overridden: bool,
}

#[derive(Debug, Clone)]
pub struct Reminder {
    pub item_id: Uuid,
    pub name: String,
    pub at: Moment,
    pub occurrence: Option<Occurrence>,
}
