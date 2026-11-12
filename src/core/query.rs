//! UI-independent filtering, planning views, reporting, and reminder decisions.
use super::{
    EventOverride, Item, Moment, Occurrence, Reminder, Result, Status, Tag, invalid, recurrence,
};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use std::{
    collections::{HashMap, HashSet},
    fmt,
};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateField {
    Start,
    End,
    Deadline,
    Completion,
    Planned,
}
impl DateField {
    pub const ALL: [Self; 5] = [
        Self::Start,
        Self::End,
        Self::Deadline,
        Self::Completion,
        Self::Planned,
    ];
    pub fn value(self, item: &Item) -> Option<&Moment> {
        match self {
            Self::Start => item.start.as_ref(),
            Self::End => item.end.as_ref(),
            Self::Deadline => item.deadline.as_ref(),
            Self::Completion => item.completion.as_ref(),
            Self::Planned => item.planned.as_ref(),
        }
    }
}
impl fmt::Display for DateField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Start => "Start",
            Self::End => "End",
            Self::Deadline => "Deadline",
            Self::Completion => "Completed",
            Self::Planned => "Planned",
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub search: String,
    pub status: Option<Status>,
    pub tag: Option<Uuid>,
    pub parent: Option<Uuid>,
    pub participant: String,
    pub priority: Option<u8>,
    pub severity: Option<u8>,
    pub effort: Option<u32>,
    pub recurring: Option<bool>,
    pub has_reminder: Option<bool>,
    pub reminder_minutes: Option<u32>,
    /// Case-insensitive substring of the master's recurrence rule.
    pub rule: String,
    pub reminded: Option<bool>,
    pub date_field: Option<DateField>,
    pub date_from: Option<NaiveDate>,
    pub date_to: Option<NaiveDate>,
    /// Exact IANA zone to match on any date field; empty means all zones.
    pub timezone: String,
}
impl Filter {
    /// Validate a complete filter before replacing the active query.
    pub fn validate(&self) -> Result<()> {
        if self.priority.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(invalid("Priority filter must be between 1 and 5"));
        }
        if self.severity.is_some_and(|value| !(1..=5).contains(&value)) {
            return Err(invalid("Severity filter must be between 1 and 5"));
        }
        if self
            .reminder_minutes
            .is_some_and(|minutes| minutes > 525_600)
        {
            return Err(invalid(
                "Reminder filter must be between 0 and 525600 minutes",
            ));
        }
        if self
            .date_from
            .zip(self.date_to)
            .is_some_and(|(from, to)| from > to)
        {
            return Err(invalid("Filter end date must not be before its start date"));
        }
        if !self.timezone.trim().is_empty()
            && self.timezone.trim().parse::<chrono_tz::Tz>().is_err()
        {
            return Err(invalid("Use a valid IANA timezone for the filter"));
        }
        Ok(())
    }

    pub fn matches(&self, item: &Item) -> bool {
        let search = self.search.trim().to_lowercase();
        let participant = self.participant.trim().to_lowercase();
        (search.is_empty()
            || item.name.to_lowercase().contains(&search)
            || item.description.to_lowercase().contains(&search))
            && self.status.is_none_or(|v| item.status == v)
            && self.tag.is_none_or(|v| item.tags.contains(&v))
            && self.parent.is_none_or(|v| item.parents.contains(&v))
            && (participant.is_empty()
                || item
                    .participants
                    .iter()
                    .any(|p| p.to_lowercase().contains(&participant)))
            && self.priority.is_none_or(|v| item.priority == v)
            && self.severity.is_none_or(|v| item.severity == v)
            && self.effort.is_none_or(|v| item.effort == v)
            && self.recurring.is_none_or(|v| item.is_recurring == v)
            && self
                .has_reminder
                .is_none_or(|v| item.reminder.is_some() == v)
            && self
                .reminder_minutes
                .is_none_or(|value| item.reminder == Some(value))
            && (self.rule.trim().is_empty()
                || item
                    .rrule
                    .to_ascii_uppercase()
                    .contains(&self.rule.trim().to_ascii_uppercase()))
            && self.reminded.is_none_or(|v| item.reminded == v)
            && (self.timezone.trim().is_empty()
                || DateField::ALL.iter().any(|field| {
                    field
                        .value(item)
                        .is_some_and(|date| date.timezone == self.timezone.trim())
                }))
            && self.date_field.is_none_or(|field| {
                field
                    .value(item)
                    .and_then(|date| date.date().ok())
                    .is_some_and(|date| {
                        self.date_from.is_none_or(|from| date >= from)
                            && self.date_to.is_none_or(|to| date <= to)
                    })
            })
    }
}

pub fn filtered<'a>(items: &'a [Item], filter: &Filter) -> Vec<&'a Item> {
    items.iter().filter(|item| filter.matches(item)).collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarKind {
    Scheduled,
    Deadline,
    Planned,
}
impl fmt::Display for CalendarKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Scheduled => "Scheduled",
            Self::Deadline => "Deadline",
            Self::Planned => "Planned",
        })
    }
}
#[derive(Debug, Clone)]
pub struct CalendarEntry {
    pub item_id: Uuid,
    pub name: String,
    pub kind: CalendarKind,
    pub start: Moment,
    pub end: Option<Moment>,
    pub status: Status,
    pub occurrence: Option<Occurrence>,
}

impl CalendarEntry {
    fn is_active(&self, now: DateTime<Utc>) -> Result<bool> {
        Ok(self.kind == CalendarKind::Scheduled
            && self.start.utc()? <= now
            && self
                .end
                .as_ref()
                .is_some_and(|end| recurrence::interval_end(end).is_ok_and(|end| end > now)))
    }
}

fn shifted_marker(
    marker: &Moment,
    master_start: &Moment,
    occurrence_start: &Moment,
) -> Result<Moment> {
    let master_in_marker_zone = Moment::from_utc(master_start.utc()?, &marker.timezone, false)?;
    let occurrence_in_marker_zone =
        Moment::from_utc(occurrence_start.utc()?, &marker.timezone, false)?;
    let offset = marker.naive()? - master_in_marker_zone.naive()?;
    let shifted = occurrence_in_marker_zone
        .naive()?
        .checked_add_signed(offset)
        .ok_or_else(|| invalid("Recurring date is outside the supported date range"))?;
    if marker.is_date() {
        return Moment::parse(&shifted.format("%Y-%m-%d").to_string(), &marker.timezone).map_err(
            |error| {
                invalid(format!(
                    "Generated date {} in {} is invalid: {error}",
                    shifted.date(),
                    marker.timezone
                ))
            },
        );
    }
    let timezone: chrono_tz::Tz = marker
        .timezone
        .parse()
        .map_err(|_| invalid("Invalid timezone"))?;
    let resolved = timezone.from_local_datetime(&shifted).earliest().ok_or_else(|| invalid(format!(
        "Generated time {} in {} falls in a skipped clock-change time; choose another planned date or deadline",
        shifted.format("%Y-%m-%d %H:%M:%S"), marker.timezone
    )))?;
    Moment::from_utc(resolved.with_timezone(&Utc), &marker.timezone, false)
}

fn append_calendar_entries(
    entries: &mut Vec<CalendarEntry>,
    item: &Item,
    occurrence: Option<&Occurrence>,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<()> {
    let schedule = occurrence
        .map(|event| (&event.start, &event.end))
        .or_else(|| item.start.as_ref().zip(item.end.as_ref()));
    for (kind, start, end) in [
        (
            CalendarKind::Scheduled,
            schedule.map(|(start, _)| start),
            schedule.map(|(_, end)| end),
        ),
        (CalendarKind::Deadline, item.deadline.as_ref(), None),
        (CalendarKind::Planned, item.planned.as_ref(), None),
    ] {
        let Some(start) = start else { continue };
        let start = if kind != CalendarKind::Scheduled
            && let Some(event) = occurrence
        {
            let master_start = item
                .start
                .as_ref()
                .ok_or_else(|| invalid("Recurring task needs a start date"))?;
            shifted_marker(start, master_start, &event.start)
                .map_err(|error| invalid(format!("Task '{}', {kind}: {error}", item.name)))?
        } else {
            start.clone()
        };
        if recurrence::overlaps(&start, end.unwrap_or(&start), from, to)? {
            entries.push(CalendarEntry {
                item_id: item.id,
                name: item.name.clone(),
                kind,
                start,
                end: end.cloned(),
                status: occurrence.map_or(item.status, |event| event.status),
                occurrence: occurrence.cloned(),
            });
        }
    }
    Ok(())
}

/// Calendar range is start-inclusive, end-exclusive. Date-only tasks retain their precision.
/// Recurring planned/deadline markers follow each occurrence's rescheduling.
pub fn calendar_entries(
    items: &[Item],
    overrides: &[EventOverride],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<Vec<CalendarEntry>> {
    if from >= to {
        return Err(invalid("Calendar range must end after it starts"));
    }
    let mut entries = vec![];
    for item in items {
        if item.is_recurring {
            let master_start = item
                .start
                .as_ref()
                .ok_or_else(|| invalid("Recurring task needs a start date"))?;
            // Expand a widened range so markers can fall into this view even when their
            // associated occurrence is before/after it. A DST margin covers offset changes.
            let mut before = Duration::zero();
            let mut after = Duration::zero();
            for marker in [&item.deadline, &item.planned].into_iter().flatten() {
                let delta = marker.utc()? - master_start.utc()?;
                before = before.max(delta);
                after = after.max(-delta);
            }
            let expansion_from = from
                .checked_sub_signed(before + Duration::days(2))
                .unwrap_or(DateTime::<Utc>::MIN_UTC);
            let expansion_to = to
                .checked_add_signed(after + Duration::days(2))
                .unwrap_or(DateTime::<Utc>::MAX_UTC);
            for occurrence in recurrence::expand(item, overrides, expansion_from, expansion_to)? {
                append_calendar_entries(&mut entries, item, Some(&occurrence), from, to)?;
            }
        } else {
            append_calendar_entries(&mut entries, item, None, from, to)?;
        }
    }
    entries.sort_by_key(|entry| (entry.start.utc().ok(), entry.name.to_lowercase()));
    Ok(entries)
}

/// Validate derived dates before committing a task/override or accepting a file.
/// This prevents a future invalid planned/deadline marker from making every
/// dashboard render fail. Each finite series is expanded once by this pass.
pub fn validate_calendar(items: &[Item], overrides: &[EventOverride]) -> Result<()> {
    let derived: Vec<_> = items
        .iter()
        .filter(|item| item.is_recurring && (item.deadline.is_some() || item.planned.is_some()))
        .cloned()
        .collect();
    calendar_entries(
        &derived,
        overrides,
        DateTime::<Utc>::MIN_UTC,
        DateTime::<Utc>::MAX_UTC,
    )
    .map(|_| ())
}

fn effective_item(master: &Item, occurrence: &Occurrence) -> Result<Item> {
    let mut item = master.clone();
    item.start = Some(occurrence.start.clone());
    item.end = Some(occurrence.end.clone());
    item.status = occurrence.status;
    item.reminded = occurrence.reminded;
    if item.status != Status::Done {
        item.completion = None;
    }
    let master_start = master
        .start
        .as_ref()
        .ok_or_else(|| invalid("Recurring task needs a start"))?;
    item.deadline = master
        .deadline
        .as_ref()
        .map(|marker| shifted_marker(marker, master_start, &occurrence.start))
        .transpose()?;
    item.planned = master
        .planned
        .as_ref()
        .map(|marker| shifted_marker(marker, master_start, &occurrence.start))
        .transpose()?;
    Ok(item)
}

/// Deadline for the occurrence represented by a dashboard card, including moves.
pub fn dashboard_deadline(item: &Item, entry: Option<&CalendarEntry>) -> Result<Option<Moment>> {
    if let Some(occurrence) = entry.and_then(|e| e.occurrence.as_ref()) {
        Ok(effective_item(item, occurrence)?.deadline)
    } else {
        Ok(item.deadline.clone())
    }
}

/// Filter calendar data after applying occurrence changes. A Done occurrence of
/// a Pending master remains discoverable by Done, and moved dates use their new
/// dates. Name, description, participants, tags, hierarchy, priority, severity,
/// effort, reminder lead, and recurrence text remain properties of the master.
pub fn calendar_entries_filtered(
    items: &[Item],
    overrides: &[EventOverride],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    filter: &Filter,
) -> Result<Vec<CalendarEntry>> {
    filter.validate()?;
    let mut master_filter = filter.clone();
    master_filter.status = None;
    master_filter.reminded = None;
    master_filter.date_field = None;
    master_filter.timezone.clear();
    let masters: Vec<_> = items
        .iter()
        .filter(|item| master_filter.matches(item))
        .cloned()
        .collect();
    let by_id: HashMap<_, _> = masters.iter().map(|item| (item.id, item)).collect();
    let entries = calendar_entries(&masters, overrides, from, to)?;
    let mut matching = Vec::new();
    // Each occurrence may yield a span and two markers. Cache its filter result
    // so recurrence shifts and task descriptions are only cloned once.
    let mut occurrence_matches = HashMap::new();
    for entry in entries {
        let master = by_id[&entry.item_id];
        let included = if let Some(occurrence) = &entry.occurrence {
            let key = (entry.item_id, occurrence.original_start.utc()?);
            if let Some(included) = occurrence_matches.get(&key) {
                *included
            } else {
                let included = filter.matches(&effective_item(master, occurrence)?);
                occurrence_matches.insert(key, included);
                included
            }
        } else {
            filter.matches(master)
        };
        if included {
            matching.push(entry);
        }
    }
    Ok(matching)
}

/// Date-only deadlines become overdue after their local calendar day has ended.
pub fn is_overdue(moment: &Moment, now: DateTime<Utc>) -> Result<bool> {
    if moment.is_date() {
        let tz: chrono_tz::Tz = moment
            .timezone
            .parse()
            .map_err(|_| invalid("Invalid timezone"))?;
        Ok(moment.date()? < now.with_timezone(&tz).date_naive())
    } else {
        Ok(moment.utc()? < now)
    }
}

pub fn display_date(moment: &Moment, timezone: &str) -> Result<NaiveDate> {
    if moment.is_date() {
        return moment.date();
    }
    let tz: chrono_tz::Tz = timezone.parse().map_err(|_| invalid("Invalid timezone"))?;
    Ok(moment.utc()?.with_timezone(&tz).date_naive())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DashboardSection {
    Overdue,
    Ongoing,
    Today,
    Upcoming,
    NearestDeadlines,
}

#[derive(Debug, Clone, Default)]
pub struct Dashboard {
    pub total: usize,
    pub done: usize,
    pub percent_complete: f32,
    pub overdue: Vec<Uuid>,
    pub ongoing: Vec<Uuid>,
    pub today: Vec<Uuid>,
    pub upcoming: Vec<Uuid>,
    pub nearest_deadlines: Vec<Uuid>,
    /// Effective dates/status for the occurrence represented in each section.
    /// Unscheduled WIP tasks have no calendar entry and use their master data.
    pub details: HashMap<(DashboardSection, Uuid), CalendarEntry>,
}

impl Dashboard {
    pub fn representative(
        &self,
        section: DashboardSection,
        item_id: Uuid,
    ) -> Option<&CalendarEntry> {
        self.details.get(&(section, item_id))
    }
}

fn push_once(ids: &mut Vec<Uuid>, id: Uuid) {
    if !ids.contains(&id) {
        ids.push(id);
    }
}

fn remember_earliest(summary: &mut Dashboard, section: DashboardSection, entry: &CalendarEntry) {
    // calendar_entries_filtered preserves chronological ordering.
    summary
        .details
        .entry((section, entry.item_id))
        .or_insert_with(|| entry.clone());
}

fn ongoing_rank(entry: &CalendarEntry, now: DateTime<Utc>) -> Result<(u8, u64)> {
    let start = entry.start.utc()?;
    let rank = if entry.is_active(now)? {
        0
    } else if entry.kind == CalendarKind::Scheduled {
        1
    } else {
        2
    };
    Ok((rank, (start - now).num_seconds().unsigned_abs()))
}

fn remember_ongoing(
    summary: &mut Dashboard,
    entry: &CalendarEntry,
    now: DateTime<Utc>,
) -> Result<()> {
    let key = (DashboardSection::Ongoing, entry.item_id);
    let replace = match summary.details.get(&key) {
        None => true,
        Some(previous) => ongoing_rank(entry, now)? < ongoing_rank(previous, now)?,
    };
    if replace {
        summary.details.insert(key, entry.clone());
    }
    Ok(())
}

pub fn dashboard(
    items: &[Item],
    overrides: &[EventOverride],
    now: DateTime<Utc>,
    timezone: &str,
    days: u32,
) -> Result<Dashboard> {
    dashboard_filtered(items, overrides, now, timezone, days, &Filter::default())
}

/// Dashboard groups use effective occurrence status and dates. Counters count
/// unique tasks represented in this filtered view. A recurring task counts as
/// Done only when every matching occurrence is Done; cancelled occurrences are
/// excluded. The completed-task report separately reports master task records.
pub fn dashboard_filtered(
    items: &[Item],
    overrides: &[EventOverride],
    now: DateTime<Utc>,
    timezone: &str,
    days: u32,
    filter: &Filter,
) -> Result<Dashboard> {
    let tz: chrono_tz::Tz = timezone.parse().map_err(|_| invalid("Invalid timezone"))?;
    let today = now.with_timezone(&tz).date_naive();
    let entries = calendar_entries_filtered(
        items,
        overrides,
        DateTime::<Utc>::MIN_UTC,
        DateTime::<Utc>::MAX_UTC,
        filter,
    )?;
    let mut statuses: HashMap<Uuid, Vec<Status>> = HashMap::new();
    for item in items
        .iter()
        .filter(|item| !item.is_recurring && filter.matches(item))
    {
        statuses.entry(item.id).or_default().push(item.status);
    }
    for entry in &entries {
        statuses
            .entry(entry.item_id)
            .or_default()
            .push(entry.status);
    }
    let done = statuses
        .values()
        .filter(|values| values.iter().all(|status| *status == Status::Done))
        .count();
    let mut summary = Dashboard {
        total: statuses.len(),
        done,
        percent_complete: percentage(done, statuses.len()),
        ongoing: items
            .iter()
            .filter(|item| {
                statuses
                    .get(&item.id)
                    .is_some_and(|values| values.contains(&Status::Wip))
            })
            .map(|item| item.id)
            .collect(),
        ..Dashboard::default()
    };
    let last_day = today
        .checked_add_signed(Duration::days(i64::from(days.min(366))))
        .ok_or_else(|| invalid("Dashboard date is outside the supported range"))?;
    for entry in &entries {
        if entry.status.is_closed() {
            continue;
        }
        let date = display_date(&entry.start, timezone)?;
        if entry.kind == CalendarKind::Deadline {
            if is_overdue(&entry.start, now)? {
                push_once(&mut summary.overdue, entry.item_id);
                remember_earliest(&mut summary, DashboardSection::Overdue, entry);
            } else {
                push_once(&mut summary.nearest_deadlines, entry.item_id);
                remember_earliest(&mut summary, DashboardSection::NearestDeadlines, entry);
            }
        }
        if date == today {
            push_once(&mut summary.today, entry.item_id);
            remember_earliest(&mut summary, DashboardSection::Today, entry);
        }
        if date > today && date <= last_day {
            push_once(&mut summary.upcoming, entry.item_id);
            remember_earliest(&mut summary, DashboardSection::Upcoming, entry);
        }
        let active = entry.is_active(now)?;
        if active {
            push_once(&mut summary.ongoing, entry.item_id);
        }
        if active || entry.status == Status::Wip {
            remember_ongoing(&mut summary, entry, now)?;
        }
    }
    Ok(summary)
}

#[derive(Debug, Clone)]
pub struct HierarchyRow {
    pub item_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub depth: usize,
    pub shared: bool,
}

/// Each task's children expand once; further parents display a shared-task reference.
/// Defensive visited sets also prevent malformed imported graphs from looping.
pub fn hierarchy(items: &[Item]) -> Vec<HierarchyRow> {
    let ids: HashSet<_> = items.iter().map(|item| item.id).collect();
    let mut children: HashMap<Uuid, Vec<&Item>> = HashMap::new();
    for item in items {
        for parent in &item.parents {
            children.entry(*parent).or_default().push(item);
        }
    }
    for siblings in children.values_mut() {
        siblings.sort_by_key(|i| (i.name.to_lowercase(), i.id));
    }
    let mut rows = vec![];
    let mut expanded = HashSet::new();
    let mut roots: Vec<_> = items
        .iter()
        .filter(|i| !i.parents.iter().any(|p| ids.contains(p)))
        .collect();
    // Add every item as a fallback for malformed cyclic graphs. Expanded roots
    // are skipped; child references can still appear beneath multiple parents.
    roots.sort_by_key(|i| (i.name.to_lowercase(), i.id));
    let mut fallback: Vec<_> = items.iter().collect();
    fallback.sort_by_key(|i| (i.name.to_lowercase(), i.id));
    roots.extend(fallback);
    for root in roots {
        if expanded.contains(&root.id) {
            continue;
        }
        let mut stack = vec![(root, 0, None)];
        while let Some((item, depth, parent_id)) = stack.pop() {
            let shared = !expanded.insert(item.id);
            rows.push(HierarchyRow {
                item_id: item.id,
                parent_id,
                depth,
                shared,
            });
            if !shared && let Some(children_here) = children.get(&item.id) {
                stack.extend(
                    children_here
                        .iter()
                        .rev()
                        .map(|child| (*child, depth + 1, Some(item.id))),
                );
            }
        }
    }
    rows
}

#[derive(Debug, Clone)]
pub struct TagReport {
    pub tag_id: Uuid,
    pub name: String,
    pub total: usize,
    pub done: usize,
    pub percent_complete: f32,
}
#[derive(Debug, Clone)]
pub struct Report {
    pub total: usize,
    pub done: usize,
    pub wont_do: usize,
    pub percent_complete: f32,
    pub total_effort: u64,
    pub completed_effort: u64,
    pub by_status: Vec<(Status, usize)>,
    pub by_tag: Vec<TagReport>,
    pub completed: Vec<Uuid>,
}
fn percentage(numerator: usize, denominator: usize) -> f32 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f32 / denominator as f32 * 100.0
    }
}
pub fn report(items: &[Item], tags: &[Tag]) -> Report {
    let done = items.iter().filter(|i| i.status == Status::Done).count();
    let mut completed: Vec<_> = items.iter().filter(|i| i.status.is_closed()).collect();
    completed.sort_by_key(|i| std::cmp::Reverse(i.completion.as_ref().and_then(|d| d.utc().ok())));
    Report {
        total: items.len(),
        done,
        wont_do: items.iter().filter(|i| i.status == Status::WontDo).count(),
        percent_complete: percentage(completed.len(), items.len()),
        total_effort: items.iter().map(|i| u64::from(i.effort)).sum(),
        completed_effort: completed.iter().map(|i| u64::from(i.effort)).sum(),
        by_status: Status::ALL
            .into_iter()
            .map(|s| (s, items.iter().filter(|i| i.status == s).count()))
            .collect(),
        by_tag: tags
            .iter()
            .map(|tag| {
                let tagged: Vec<_> = items.iter().filter(|i| i.tags.contains(&tag.id)).collect();
                let done = tagged.iter().filter(|i| i.status.is_closed()).count();
                TagReport {
                    tag_id: tag.id,
                    name: tag.name.clone(),
                    total: tagged.len(),
                    done,
                    percent_complete: percentage(done, tagged.len()),
                }
            })
            .collect(),
        completed: completed.into_iter().map(|i| i.id).collect(),
    }
}

/// Emit each unacknowledged reminder from its trigger time until 24 hours after
/// the scheduled start. Running-app reminders deliberately do not replay old history.
pub fn due_reminders(
    items: &[Item],
    overrides: &[EventOverride],
    now: DateTime<Utc>,
) -> Result<Vec<Reminder>> {
    let mut reminders = vec![];
    for item in items {
        let Some(minutes) = item.reminder else {
            continue;
        };
        if minutes > 525_600 {
            return Err(invalid(
                "Reminders can be at most one year before an occurrence",
            ));
        }
        let lead = Duration::minutes(i64::from(minutes));
        let from = now
            .checked_sub_signed(Duration::days(1))
            .unwrap_or(DateTime::<Utc>::MIN_UTC);
        let to = now
            .checked_add_signed(lead + Duration::seconds(1))
            .ok_or_else(|| invalid("Reminder date is outside the supported range"))?;
        if item.is_recurring {
            for occurrence in recurrence::expand(item, overrides, from, to)? {
                let start = occurrence.start.utc()?;
                if !occurrence.status.is_closed()
                    && !occurrence.reminded
                    && start
                        .checked_sub_signed(lead)
                        .unwrap_or(DateTime::<Utc>::MIN_UTC)
                        <= now
                    && start >= from
                {
                    reminders.push(Reminder {
                        item_id: item.id,
                        name: item.name.clone(),
                        at: occurrence.start.clone(),
                        occurrence: Some(occurrence),
                    });
                }
            }
        } else if !item.status.is_closed()
            && !item.reminded
            && let Some(at) = item
                .start
                .as_ref()
                .or(item.planned.as_ref())
                .or(item.deadline.as_ref())
        {
            let start = at.utc()?;
            if start
                .checked_sub_signed(lead)
                .unwrap_or(DateTime::<Utc>::MIN_UTC)
                <= now
                && start >= from
            {
                reminders.push(Reminder {
                    item_id: item.id,
                    name: item.name.clone(),
                    at: at.clone(),
                    occurrence: None,
                });
            }
        }
    }
    reminders.sort_by_key(|reminder| reminder.at.utc().ok());
    Ok(reminders)
}

/// Convert date-only calendar bounds in their original timezone before display.
/// Show the first valid instant and last minute of an all-day interval, even
/// across skipped/repeated midnights. Timed values retain their exact instant.
pub fn calendar_moment(moment: &Moment, timezone: &str, end: bool) -> Result<Moment> {
    let mut value = moment.utc()?;
    if moment.is_date() && end {
        value = recurrence::interval_end(moment)?
            .checked_sub_signed(Duration::minutes(1))
            .ok_or_else(|| invalid("Calendar date is outside the supported range"))?;
    }
    Moment::from_utc(value, timezone, false)
}
