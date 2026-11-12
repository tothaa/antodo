//! Finite RFC 5545 recurrence and occurrence overrides.
//!
//! RRULE candidates are generated in a UTC surrogate of the user's wall clock.
//! Resolving each candidate ourselves prevents the recurrence dependency from
//! silently shifting nonexistent clock-change times. Gaps are skipped without
//! consuming COUNT; repeated times use the first instant. Date-only ends are inclusive; timed
//! intervals use an exclusive end and retain their elapsed duration.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use rrule::{RRule, RRuleSet, Tz, Unvalidated};

use super::{EventOverride, Item, Moment, Occurrence, Result, invalid};

/// The entire finite series must fit within this bound, not just the viewport.
pub const MAX_OCCURRENCES: usize = 10_000;
const MAX_RULE_LENGTH: usize = 2048;
const MAX_CANDIDATES: usize = 20_000;
// Stay below the dependency's 100,000-period search limit for UNTIL series.
const MAX_PERIODS: i64 = 90_000;

struct ParsedRule {
    set: RRuleSet,
    count: Option<usize>,
    until: Option<DateTime<Utc>>,
}

/// Validate dates and finite recurrence, including every generated occurrence.
pub fn validate_rule(item: &Item) -> Result<()> {
    base_occurrences(item).map(|_| ())
}

fn paired_dates(item: &Item) -> Result<Option<(&Moment, &Moment)>> {
    match (&item.start, &item.end) {
        (None, None) if item.is_recurring => {
            Err(invalid("Recurring tasks need both a start and end date"))
        }
        (None, None) => Ok(None),
        (Some(start), Some(end)) => {
            if end.utc()? < start.utc()? {
                return Err(invalid("The end must not be before the start"));
            }
            if item.is_recurring && start.is_date() != end.is_date() {
                return Err(invalid(
                    "Recurring start and end must both be dates or both include a time",
                ));
            }
            Ok(Some((start, end)))
        }
        _ => Err(invalid(
            "Provide both a start and an end, or leave both empty",
        )),
    }
}

fn parse_rule(item: &Item, start: &Moment) -> Result<ParsedRule> {
    let upper = item.rrule.trim().to_ascii_uppercase();
    let raw = upper.strip_prefix("RRULE:").unwrap_or(&upper);
    if raw.is_empty() || raw.len() > MAX_RULE_LENGTH || raw.chars().any(char::is_whitespace) {
        return Err(invalid(
            "Enter one RRULE, without whitespace, ending in COUNT or UNTIL",
        ));
    }
    let mut parts = BTreeMap::new();
    for part in raw.split(';') {
        let (key, value) = part
            .split_once('=')
            .ok_or_else(|| invalid("Each recurrence property needs a name and value"))?;
        if value.is_empty() || parts.insert(key.to_string(), value.to_string()).is_some() {
            return Err(invalid(
                "Recurrence properties must be nonempty and cannot repeat",
            ));
        }
    }
    if parts.contains_key("COUNT") == parts.contains_key("UNTIL") {
        return Err(invalid(
            "Recurrence needs exactly one ending: COUNT or UNTIL",
        ));
    }
    if parts
        .get("INTERVAL")
        .is_some_and(|v| v.parse::<u16>().ok().filter(|n| *n > 0).is_none())
    {
        return Err(invalid(
            "Recurrence interval must be a positive number up to 65535",
        ));
    }
    let frequency = parts
        .get("FREQ")
        .ok_or_else(|| invalid("Recurrence needs FREQ"))?;
    if start.is_date()
        && (matches!(frequency.as_str(), "HOURLY" | "MINUTELY" | "SECONDLY")
            || ["BYHOUR", "BYMINUTE", "BYSECOND"]
                .iter()
                .any(|key| parts.contains_key(*key)))
    {
        return Err(invalid(
            "An all-day recurrence cannot specify hours, minutes, or seconds",
        ));
    }
    let times_per_day: usize = ["BYHOUR", "BYMINUTE", "BYSECOND"]
        .iter()
        .map(|key| parts.get(*key).map_or(1, |v| v.split(',').count()))
        .product();
    if times_per_day > 1000 {
        return Err(invalid(
            "Recurrence has too many time combinations; use a simpler rule",
        ));
    }
    let count = parts
        .get("COUNT")
        .map(|value| {
            value
                .parse::<usize>()
                .ok()
                .filter(|n| (1..=MAX_OCCURRENCES).contains(n))
                .ok_or_else(|| invalid(format!("COUNT must be between 1 and {MAX_OCCURRENCES}")))
        })
        .transpose()?;
    let start_naive = start.naive()?;
    if !(1..=9999).contains(&start_naive.year()) {
        return Err(invalid(
            "Recurring dates must use years between 0001 and 9999",
        ));
    }
    let mut until = None;
    if let Some(value) = parts.get("UNTIL") {
        let actual_until = if start.is_date() {
            let date = NaiveDate::parse_from_str(value, "%Y%m%d")
                .map_err(|_| invalid("For all-day tasks, use UNTIL=YYYYMMDD"))?;
            Moment::parse(&date.format("%Y-%m-%d").to_string(), &start.timezone)?.utc()?
        } else {
            if value.len() != 16 || !value.ends_with('Z') {
                return Err(invalid(
                    "For timed tasks, use a UTC ending: UNTIL=YYYYMMDDTHHMMSSZ",
                ));
            }
            NaiveDateTime::parse_from_str(value, "%Y%m%dT%H%M%SZ")
                .map_err(|_| invalid("UNTIL is not a valid UTC date and time"))?
                .and_utc()
        };
        if actual_until < start.utc()? {
            return Err(invalid("Recurrence ending must not be before the start"));
        }
        let timezone: chrono_tz::Tz = start
            .timezone
            .parse()
            .map_err(|_| invalid("Invalid timezone"))?;
        let local_until = actual_until.with_timezone(&timezone).naive_local();
        if local_until.year() > 9999 {
            return Err(invalid(
                "Recurring dates must use years between 0001 and 9999",
            ));
        }
        let span = local_until - start_naive;
        let periods = match frequency.as_str() {
            "SECONDLY" => span.num_seconds(),
            "MINUTELY" => span.num_minutes(),
            "HOURLY" => span.num_hours(),
            "DAILY" => span.num_days(),
            "WEEKLY" => span.num_weeks(),
            "MONTHLY" => {
                i64::from(local_until.year() - start_naive.year()) * 12
                    + i64::from(local_until.month())
                    - i64::from(start_naive.month())
            }
            "YEARLY" => i64::from(local_until.year() - start_naive.year()),
            _ => return Err(invalid("Unknown recurrence frequency")),
        };
        // Deliberately conservative: sparse constraints must not hide truncated
        // output behind the dependency's internal search-loop guard.
        if periods > MAX_PERIODS {
            return Err(invalid(
                "Recurrence spans too many frequency periods; shorten UNTIL or use COUNT",
            ));
        }
        parts.insert(
            "UNTIL".into(),
            local_until.format("%Y%m%dT%H%M%SZ").to_string(),
        );
        until = Some(actual_until);
    }
    if count.is_some() {
        // Invalid local clock times do not consume RFC 5545 COUNT. Generate a
        // bounded surplus of candidates and stop after enough valid instants.
        parts.insert("COUNT".into(), (MAX_CANDIDATES + 1).to_string());
    }
    let normalized = parts
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(";");
    let rule: RRule<Unvalidated> = normalized
        .parse()
        .map_err(|error| invalid(format!("Invalid recurrence: {error}")))?;
    let surrogate = Tz::UTC.from_utc_datetime(&start_naive);
    let rule = rule
        .validate(surrogate)
        .map_err(|error| invalid(format!("Invalid recurrence: {error}")))?;
    Ok(ParsedRule {
        set: RRuleSet::new(surrogate).rrule(rule),
        count,
        until,
    })
}

fn make_occurrence(item: &Item, start: Moment, end: Moment) -> Occurrence {
    Occurrence {
        item_id: item.id,
        original_start: start.clone(),
        original_end: end.clone(),
        start,
        end,
        status: item.status,
        reminded: if item.is_recurring {
            false
        } else {
            item.reminded
        },
        overridden: false,
    }
}

fn base_occurrences(item: &Item) -> Result<Vec<Occurrence>> {
    let Some((start, end)) = paired_dates(item)? else {
        return Ok(vec![]);
    };
    if !item.is_recurring {
        return Ok(vec![make_occurrence(item, start.clone(), end.clone())]);
    }
    let parsed = parse_rule(item, start)?;
    let elapsed = end.utc()? - start.utc()?;
    let calendar_days = end.date()? - start.date()?;
    let timezone: chrono_tz::Tz = start
        .timezone
        .parse()
        .map_err(|_| invalid("Invalid timezone"))?;
    let mut result = Vec::new();
    let bounded_set = parsed.set.limit();
    for (index, date) in bounded_set.into_iter().take(MAX_CANDIDATES + 1).enumerate() {
        if index == 0 && date.naive_utc() != start.naive()? {
            return Err(invalid(
                "Choose a start date and time that matches the recurrence pattern; the start must be its first occurrence",
            ));
        }
        if index >= MAX_CANDIDATES {
            return Err(invalid(
                "Recurrence exceeds its candidate safety limit; simplify the rule",
            ));
        }
        let occurrence_start = if index == 0 {
            // DTSTART is an explicit value, potentially retaining a particular
            // repeated-hour offset from an imported or previously generated date.
            Moment::from_utc(start.utc()?, &start.timezone, start.is_date())?
        } else if start.is_date() {
            // Midnight may be skipped even though the calendar date exists.
            // A wholly skipped date (e.g. Samoa 2011-12-30) does not count.
            let candidate = Moment {
                value: date.format("%Y-%m-%d").to_string(),
                timezone: start.timezone.clone(),
                offset_seconds: None,
            };
            if candidate.utc().is_err() {
                continue;
            }
            candidate
        } else {
            let resolved = match timezone.from_local_datetime(&date.naive_utc()) {
                LocalResult::None => continue,
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(first, second) => first.min(second),
            };
            Moment::from_utc(resolved.with_timezone(&Utc), &start.timezone, false)?
        };
        if parsed
            .until
            .is_some_and(|until| occurrence_start.utc().is_ok_and(|value| value > until))
        {
            continue;
        }
        let occurrence_end = if start.is_date() {
            let date = occurrence_start
                .date()?
                .checked_add_signed(calendar_days)
                .ok_or_else(|| invalid("Recurring end date exceeds the supported date range"))?;
            Moment::parse(&date.format("%Y-%m-%d").to_string(), &end.timezone)?
        } else {
            let end_utc = occurrence_start
                .utc()?
                .checked_add_signed(elapsed)
                .ok_or_else(|| invalid("Recurring end time exceeds the supported date range"))?;
            let end_moment = Moment::from_utc(end_utc, &end.timezone, false)?;
            end_moment.utc()?;
            end_moment
        };
        result.push(make_occurrence(item, occurrence_start, occurrence_end));
        if parsed.count == Some(result.len()) {
            break;
        }
        if result.len() > MAX_OCCURRENCES {
            return Err(invalid(format!(
                "Recurrence exceeds the {MAX_OCCURRENCES}-occurrence safety limit; shorten its ending"
            )));
        }
    }
    if parsed.count.is_some_and(|count| result.len() != count) {
        return Err(invalid(
            "Recurrence could not generate its full COUNT within safety limits; simplify the rule",
        ));
    }
    if result.is_empty() {
        return Err(invalid(
            "Recurrence produces no occurrences; check its start, ending, and filters",
        ));
    }
    Ok(result)
}

fn validate_against(item: &Item, value: &EventOverride, base: &[Occurrence]) -> Result<()> {
    if !item.is_recurring || value.item_id != item.id {
        return Err(invalid(
            "An occurrence change must belong to its recurring task",
        ));
    }
    let original = base
        .iter()
        .find(|event| {
            event.start.equivalent(&value.original_start)
                && event.end.equivalent(&value.original_end)
        })
        .ok_or_else(|| invalid("The original occurrence is not part of this task's recurrence"))?;
    match (&value.new_start, &value.new_end) {
        (None, None) => {}
        (Some(start), Some(end)) => {
            if start.is_date() != end.is_date() {
                return Err(invalid(
                    "An occurrence's new start and end must use the same date precision",
                ));
            }
            if end.utc()? < start.utc()? {
                return Err(invalid(
                    "An occurrence's new end must not be before its new start",
                ));
            }
        }
        _ => {
            return Err(invalid(
                "Provide both a new start and a new end, or leave both empty",
            ));
        }
    }
    // Keep the generated value live here: it establishes both instants rather
    // than merely accepting any arbitrary original_end after original_start.
    original.start.utc()?;
    Ok(())
}

/// Validate an override against the complete master series.
pub fn validate_override(item: &Item, value: &EventOverride) -> Result<()> {
    validate_against(item, value, &base_occurrences(item)?)
}

/// Exclusive boundary for an inclusive all-day end, or the exact timed end.
/// A timezone may skip local midnight or an entire date. The interval ends at
/// the first existing instant on or after the next local midnight in that case.
pub fn interval_end(end: &Moment) -> Result<DateTime<Utc>> {
    if !end.is_date() {
        return end.utc();
    }
    let midnight = end
        .naive()?
        .checked_add_signed(Duration::days(1))
        .ok_or_else(|| invalid("End date exceeds the supported date range"))?;
    let timezone: chrono_tz::Tz = end
        .timezone
        .parse()
        .map_err(|_| invalid("Invalid timezone"))?;
    for minutes in 0..=2880 {
        let candidate = midnight
            .checked_add_signed(Duration::minutes(minutes))
            .ok_or_else(|| invalid("End date exceeds the supported date range"))?;
        if let Some(value) = timezone.from_local_datetime(&candidate).earliest() {
            return Ok(value.with_timezone(&Utc));
        }
    }
    Err(invalid(
        "Could not resolve the end of this all-day interval in its timezone",
    ))
}

/// Intersect an inclusive all-day or exclusive timed interval with `[from, to)`.
/// Zero-duration timed events remain visible at their exact instant.
pub(crate) fn overlaps(
    start: &Moment,
    end: &Moment,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<bool> {
    let start = start.utc()?;
    let end = interval_end(end)?;
    Ok(start < to && (end > from || (start == end && start >= from)))
}

/// Expand intersecting occurrences in `[from, to)`, applying changes before
/// filtering. Therefore a moved occurrence can enter the window even when its
/// original date lies outside it. Cancelled occurrences are omitted.
pub fn expand(
    item: &Item,
    overrides: &[EventOverride],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<Occurrence>> {
    if to <= from {
        return Err(invalid("The calendar range must end after it starts"));
    }
    let base = base_occurrences(item)?;
    let mut changes = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for value in overrides.iter().filter(|value| value.item_id == item.id) {
        validate_against(item, value, &base)?;
        if changes.insert(value.original_start.utc()?, value).is_some() || !ids.insert(value.id) {
            return Err(invalid("An occurrence has more than one saved change"));
        }
    }
    let mut result = Vec::new();
    for mut event in base {
        if let Some(value) = changes.get(&event.original_start.utc()?) {
            if value.cancelled {
                continue;
            }
            if let (Some(start), Some(end)) = (&value.new_start, &value.new_end) {
                event.start = start.clone();
                event.end = end.clone();
            }
            event.status = value.status;
            event.reminded = value.reminded;
            event.overridden = true;
        }
        if overlaps(&event.start, &event.end, from, to)? {
            result.push(event);
        }
    }
    result.sort_by_key(|event| event.start.utc().ok());
    Ok(result)
}
