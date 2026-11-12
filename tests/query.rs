use antodo::core::{EventOverride, Item, Moment, Status, Tag, query::*};
use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

fn moment(value: &str) -> Moment {
    Moment::parse(value, "Europe/Budapest").unwrap()
}
fn utc(value: &str) -> DateTime<Utc> {
    value.parse().unwrap()
}
fn scheduled(name: &str, start: &str, end: &str) -> Item {
    let mut item = Item::new(name);
    item.start = Some(moment(start));
    item.end = Some(moment(end));
    item
}
fn recurring() -> Item {
    let mut item = scheduled("Daily review", "2026-11-10 09:00", "2026-11-10 10:00");
    item.is_recurring = true;
    item.rrule = "FREQ=DAILY;COUNT=3".into();
    item
}
fn moved(
    item: &Item,
    original: &str,
    original_end: &str,
    new: &str,
    new_end: &str,
) -> EventOverride {
    EventOverride {
        id: Uuid::now_v7(),
        item_id: item.id,
        original_start: moment(original),
        original_end: moment(original_end),
        new_start: Some(moment(new)),
        new_end: Some(moment(new_end)),
        cancelled: false,
        status: Status::Pending,
        reminded: false,
    }
}

#[test]
fn search_and_all_property_filters_compose() {
    let tag = Tag::new("work", "#aabbcc");
    let parent = Item::new("parent");
    let mut item = Item::new("Build Rust app");
    item.description = "**Review** the architecture".into();
    item.tags.push(tag.id);
    item.parents.push(parent.id);
    item.participants.push("Somebody@Example.com".into());
    item.status = Status::Wip;
    item.priority = 2;
    item.severity = 4;
    item.effort = 8;
    item.reminder = Some(15);
    item.planned = Some(moment("2026-11-10"));
    let mut filter = Filter {
        search: "ARCHITECTURE".into(),
        status: Some(Status::Wip),
        tag: Some(tag.id),
        parent: Some(parent.id),
        participant: "EXAMPLE.COM".into(),
        priority: Some(2),
        severity: Some(4),
        effort: Some(8),
        recurring: Some(false),
        has_reminder: Some(true),
        reminded: Some(false),
        date_field: Some(DateField::Planned),
        date_from: NaiveDate::from_ymd_opt(2026, 11, 10),
        date_to: NaiveDate::from_ymd_opt(2026, 11, 10),
        timezone: "Europe/Budapest".into(),
        reminder_minutes: Some(15),
        rule: String::new(),
    };
    assert!(filter.matches(&item));
    filter.search = "missing".into();
    assert!(!filter.matches(&item));
    filter.search = "RUST".into();
    assert!(filter.matches(&item));
    filter.date_field = Some(DateField::Deadline);
    assert!(!filter.matches(&item));
    assert_eq!(
        filtered(&[item.clone(), parent], &Filter::default()).len(),
        2
    );
    filter = Filter {
        timezone: "UTC".into(),
        ..Filter::default()
    };
    assert!(!filter.matches(&item));
}

#[test]
fn recurrence_rule_and_exact_reminder_filters_are_available() {
    let mut item = recurring();
    item.reminder = Some(15);
    let mut filter = Filter {
        rule: "freq=daily".into(),
        reminder_minutes: Some(15),
        ..Filter::default()
    };
    assert!(filter.matches(&item));
    filter.reminder_minutes = Some(30);
    assert!(!filter.matches(&item));
    filter.reminder_minutes = Some(600_000);
    assert!(filter.validate().is_err());
    filter.reminder_minutes = None;
    filter.rule = "WEEKLY".into();
    assert!(!filter.matches(&item));
}

#[test]
fn calendar_and_dashboard_filters_use_changed_occurrence_properties() {
    let mut item = recurring();
    item.reminder = Some(15);
    item.deadline = Some(moment("2026-11-10 12:00"));
    let mut changed = moved(
        &item,
        "2026-11-11 09:00",
        "2026-11-11 10:00",
        "2026-11-20 14:00",
        "2026-11-20 15:00",
    );
    changed.status = Status::Done;
    changed.reminded = true;
    let filter = Filter {
        status: Some(Status::Done),
        reminded: Some(true),
        reminder_minutes: Some(15),
        rule: "DAILY".into(),
        date_field: Some(DateField::Start),
        date_from: NaiveDate::from_ymd_opt(2026, 11, 20),
        date_to: NaiveDate::from_ymd_opt(2026, 11, 20),
        ..Filter::default()
    };
    assert!(!filter.matches(&item)); // The master remains Pending on November 10.
    let entries = calendar_entries_filtered(
        std::slice::from_ref(&item),
        std::slice::from_ref(&changed),
        utc("2026-11-01T00:00:00Z"),
        utc("2026-12-01T00:00:00Z"),
        &filter,
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.status == Status::Done));
    let summary = dashboard_filtered(
        std::slice::from_ref(&item),
        std::slice::from_ref(&changed),
        utc("2026-11-20T12:00:00Z"),
        "Europe/Budapest",
        7,
        &filter,
    )
    .unwrap();
    assert_eq!(summary.total, 1);
    assert_eq!(summary.done, 1);
    assert_eq!(summary.percent_complete, 100.0);
    assert!(summary.ongoing.is_empty());
    assert!(summary.overdue.is_empty());
    let pending = Filter {
        status: Some(Status::Pending),
        ..Filter::default()
    };
    let summary = dashboard_filtered(
        &[item],
        &[changed],
        utc("2026-11-20T12:00:00Z"),
        "Europe/Budapest",
        7,
        &pending,
    )
    .unwrap();
    assert_eq!(summary.total, 1);
    assert_eq!(summary.done, 0);
    assert_eq!(summary.overdue.len(), 1);
}

#[test]
fn dashboard_sections_keep_their_actual_occurrence_dates_and_statuses() {
    let mut item = recurring();
    item.planned = Some(moment("2026-11-10 08:00"));
    item.deadline = Some(moment("2026-11-10 11:00"));
    let mut active = moved(
        &item,
        "2026-11-11 09:00",
        "2026-11-11 10:00",
        "2026-11-20 14:00",
        "2026-11-20 15:00",
    );
    active.status = Status::Wip;
    let future = moved(
        &item,
        "2026-11-12 09:00",
        "2026-11-12 10:00",
        "2026-11-21 09:00",
        "2026-11-21 10:00",
    );
    let summary = dashboard(
        std::slice::from_ref(&item),
        &[active, future],
        utc("2026-11-20T13:30:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert_eq!(summary.total, 1);
    assert_eq!(summary.done, 0);
    let overdue = summary
        .representative(DashboardSection::Overdue, item.id)
        .unwrap();
    assert_eq!(overdue.start.value, "2026-11-10 11:00:00");
    assert_eq!(overdue.status, Status::Pending);
    let today = summary
        .representative(DashboardSection::Today, item.id)
        .unwrap();
    assert_eq!(today.start.value, "2026-11-20 13:00:00");
    assert_eq!(today.kind, CalendarKind::Planned);
    assert_eq!(today.status, Status::Wip);
    let ongoing = summary
        .representative(DashboardSection::Ongoing, item.id)
        .unwrap();
    assert_eq!(ongoing.start.value, "2026-11-20 14:00");
    assert_eq!(ongoing.kind, CalendarKind::Scheduled);
    assert_eq!(ongoing.status, Status::Wip);
    assert_eq!(
        ongoing.occurrence.as_ref().unwrap().original_start.value,
        "2026-11-11 09:00:00"
    );
    let nearest = summary
        .representative(DashboardSection::NearestDeadlines, item.id)
        .unwrap();
    assert_eq!(nearest.start.value, "2026-11-20 16:00:00");
    let upcoming = summary
        .representative(DashboardSection::Upcoming, item.id)
        .unwrap();
    assert_eq!(upcoming.start.value, "2026-11-21 08:00:00");
    assert_eq!(upcoming.status, Status::Pending);
}

#[test]
fn dashboard_representatives_respect_display_timezone_and_unscheduled_fallback() {
    let item = recurring();
    let mut shifted = moved(
        &item,
        "2026-11-11 09:00",
        "2026-11-11 10:00",
        "2026-11-20 23:30",
        "2026-11-21 00:30",
    );
    shifted.new_start = Some(Moment::parse("2026-11-20 23:30", "America/New_York").unwrap());
    shifted.new_end = Some(Moment::parse("2026-11-21 00:30", "America/New_York").unwrap());
    let mut unscheduled = Item::new("In progress without a date");
    unscheduled.status = Status::Wip;
    let summary = dashboard(
        &[item.clone(), unscheduled.clone()],
        &[shifted],
        utc("2026-11-20T12:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert!(!summary.today.contains(&item.id));
    let upcoming = summary
        .representative(DashboardSection::Upcoming, item.id)
        .unwrap();
    assert_eq!(
        display_date(&upcoming.start, "Europe/Budapest").unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 21).unwrap()
    );
    assert_eq!(upcoming.start.timezone, "America/New_York");
    assert!(summary.ongoing.contains(&unscheduled.id));
    assert!(
        summary
            .representative(DashboardSection::Ongoing, unscheduled.id)
            .is_none()
    );
}

#[test]
fn date_only_task_remains_ongoing_through_its_whole_end_day() {
    let item = scheduled("All day", "2026-11-10", "2026-11-10");
    let summary = dashboard(
        std::slice::from_ref(&item),
        &[],
        utc("2026-11-10T12:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert_eq!(summary.ongoing, vec![item.id]);
    let summary = dashboard(
        &[item],
        &[],
        utc("2026-11-10T23:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert!(summary.ongoing.is_empty());
}

#[test]
fn generated_marker_in_repeated_hour_uses_first_instant() {
    let mut item = scheduled("Early review", "2026-10-24 01:00", "2026-10-24 01:30");
    item.is_recurring = true;
    item.rrule = "FREQ=DAILY;COUNT=2".into();
    item.deadline = Some(moment("2026-10-24 02:30"));
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-10-25T00:00:00Z"),
        utc("2026-10-26T00:00:00Z"),
    )
    .unwrap();
    let deadline = entries
        .iter()
        .find(|entry| entry.kind == CalendarKind::Deadline)
        .unwrap();
    assert_eq!(deadline.start.utc().unwrap(), utc("2026-10-25T00:30:00Z"));
    assert_eq!(deadline.start.offset_seconds, Some(7200));
}

#[test]
fn derived_date_validation_explains_future_clock_gap_before_commit() {
    let mut item = scheduled("Early review", "2026-03-28 01:00", "2026-03-28 01:30");
    item.is_recurring = true;
    item.rrule = "FREQ=DAILY;COUNT=3".into();
    item.deadline = Some(moment("2026-03-28 02:30"));
    let error = validate_calendar(std::slice::from_ref(&item), &[])
        .unwrap_err()
        .to_string();
    for expected in [
        "Early review",
        "Deadline",
        "2026-03-29 02:30:00",
        "Europe/Budapest",
        "skipped",
    ] {
        assert!(error.contains(expected), "{error} lacks {expected}");
    }
    item.deadline = Some(moment("2026-03-28 03:30"));
    validate_calendar(&[item], &[]).unwrap();
}

#[test]
fn reminder_date_extremes_return_errors_without_panicking() {
    let mut item = recurring();
    item.reminder = Some(15);
    assert!(due_reminders(std::slice::from_ref(&item), &[], DateTime::<Utc>::MAX_UTC).is_err());
    assert!(
        due_reminders(&[item], &[], DateTime::<Utc>::MIN_UTC)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn date_only_deadline_stays_current_until_its_local_day_finishes() {
    let deadline = moment("2026-11-10");
    assert!(!is_overdue(&deadline, utc("2026-11-10T22:59:59Z")).unwrap());
    assert!(is_overdue(&deadline, utc("2026-11-10T23:00:00Z")).unwrap());
    assert!(is_overdue(&moment("2026-11-10 10:00"), utc("2026-11-10T09:00:01Z")).unwrap());
    assert_eq!(
        display_date(&moment("2026-11-11 00:30"), "UTC").unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 10).unwrap()
    );
    assert_eq!(
        display_date(&moment("2026-11-11"), "UTC").unwrap(),
        NaiveDate::from_ymd_opt(2026, 11, 11).unwrap()
    );
}

#[test]
fn calendar_contains_spans_plans_deadlines_and_excludes_outside_range() {
    let mut item = scheduled("Trip", "2026-11-09 10:00", "2026-11-12 10:00");
    item.deadline = Some(moment("2026-11-11 12:00"));
    item.planned = Some(moment("2026-11-10 12:00"));
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-11-10T00:00:00Z"),
        utc("2026-11-11T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == CalendarKind::Scheduled)
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry.kind == CalendarKind::Planned)
    );
    assert!(
        calendar_entries(
            &[],
            &[],
            utc("2026-11-11T00:00:00Z"),
            utc("2026-11-10T00:00:00Z")
        )
        .is_err()
    );
}

#[test]
fn moved_occurrence_moves_markers_and_uses_override_status() {
    let mut item = recurring();
    item.deadline = Some(moment("2026-11-10 12:00"));
    item.planned = Some(moment("2026-11-10 08:00"));
    let mut override_ = moved(
        &item,
        "2026-11-11 09:00",
        "2026-11-11 10:00",
        "2026-11-20 14:00",
        "2026-11-20 15:00",
    );
    override_.status = Status::Done;
    let entries = calendar_entries(
        &[item],
        &[override_],
        utc("2026-11-20T00:00:00Z"),
        utc("2026-11-21T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(entries.len(), 3);
    assert!(entries.iter().all(|entry| entry.status == Status::Done));
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.kind == CalendarKind::Deadline)
            .unwrap()
            .start
            .value,
        "2026-11-20 17:00:00"
    );
    assert_eq!(
        entries
            .iter()
            .find(|entry| entry.kind == CalendarKind::Planned)
            .unwrap()
            .start
            .value,
        "2026-11-20 13:00:00"
    );
}

#[test]
fn recurrence_markers_outside_event_window_are_still_in_calendar() {
    let mut item = recurring();
    item.deadline = Some(moment("2026-11-17 12:00"));
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-11-18T00:00:00Z"),
        utc("2026-11-19T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].kind, CalendarKind::Deadline);
    assert_eq!(entries[0].start.value, "2026-11-18 12:00:00");
}

#[test]
fn dashboard_prioritizes_overdue_and_respects_occurrence_completion() {
    let mut overdue = Item::new("Overdue");
    overdue.deadline = Some(moment("2026-11-09"));
    let mut today = scheduled("Now", "2026-11-10 09:00", "2026-11-10 17:00");
    today.status = Status::Wip;
    today.planned = Some(moment("2026-11-10"));
    today.deadline = Some(moment("2026-11-10"));
    let mut tomorrow = Item::new("Tomorrow");
    tomorrow.planned = Some(moment("2026-11-11"));
    let mut later = Item::new("Later");
    later.deadline = Some(moment("2026-12-25"));
    let mut done = Item::new("Done");
    done.status = Status::Done;
    done.deadline = Some(moment("2026-10-01"));
    let tasks = [
        overdue.clone(),
        today.clone(),
        tomorrow.clone(),
        later.clone(),
        done,
    ];
    let summary = dashboard(
        &tasks,
        &[],
        utc("2026-11-10T11:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert_eq!(summary.total, 5);
    assert_eq!(summary.percent_complete, 20.0);
    assert_eq!(summary.overdue, vec![overdue.id]);
    assert_eq!(summary.ongoing, vec![today.id]);
    assert_eq!(summary.today, vec![today.id]);
    assert_eq!(summary.upcoming, vec![tomorrow.id]);
    assert_eq!(summary.nearest_deadlines, vec![today.id, later.id]);

    let mut repeating = recurring();
    repeating.deadline = Some(moment("2026-11-10 11:00"));
    let mut occurrence = moved(
        &repeating,
        "2026-11-10 09:00",
        "2026-11-10 10:00",
        "2026-11-10 09:00",
        "2026-11-10 10:00",
    );
    occurrence.status = Status::Done;
    let summary = dashboard(
        &[repeating],
        &[occurrence],
        utc("2026-11-10T11:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert!(summary.overdue.is_empty());
}

#[test]
fn hierarchy_displays_shared_references_without_recursive_duplicates() {
    let a = Item::new("A");
    let b = Item::new("B");
    let mut child = Item::new("Shared");
    child.parents = vec![a.id, b.id];
    let mut leaf = Item::new("Leaf");
    leaf.parents.push(child.id);
    let rows = hierarchy(&[a, b, child.clone(), leaf.clone()]);
    assert_eq!(rows.len(), 5);
    assert_eq!(rows.iter().filter(|row| row.item_id == leaf.id).count(), 1);
    assert_eq!(
        rows.iter()
            .filter(|row| row.item_id == child.id && row.shared)
            .count(),
        1
    );
    let mut cycle_a = Item::new("A");
    let mut cycle_b = Item::new("B");
    cycle_a.parents.push(cycle_b.id);
    cycle_b.parents.push(cycle_a.id);
    assert_eq!(hierarchy(&[cycle_a, cycle_b]).len(), 3);
}

#[test]
fn reports_count_done_and_wont_do_as_closed() {
    let tag = Tag::new("Work", "#112233");
    let mut done = Item::new("Done");
    done.status = Status::Done;
    done.effort = 8;
    done.tags.push(tag.id);
    done.completion = Some(moment("2026-11-10 12:00"));
    let mut abandoned = Item::new("Abandoned");
    abandoned.status = Status::WontDo;
    abandoned.effort = 2;
    abandoned.tags.push(tag.id);
    let pending = Item::new("Pending");
    let result = report(&[done.clone(), abandoned.clone(), pending], &[tag]);
    assert_eq!(result.total, 3);
    assert_eq!(result.done, 1);
    assert_eq!(result.wont_do, 1);
    assert!((result.percent_complete - 66.66667).abs() < 0.001);
    assert_eq!(result.total_effort, 10);
    assert_eq!(result.completed_effort, 10);
    assert_eq!(result.completed, vec![done.id, abandoned.id]);
    assert_eq!(result.by_tag[0].percent_complete, 100.0);
    assert_eq!(
        result
            .by_status
            .iter()
            .find(|(status, _)| *status == Status::Pending)
            .unwrap()
            .1,
        1
    );
    assert_eq!(report(&[], &[]).percent_complete, 0.0);
}

#[test]
fn reminders_fire_in_lead_window_and_do_not_replay_closed_or_old_tasks() {
    let mut item = scheduled("Call", "2026-11-10 12:00", "2026-11-10 13:00");
    item.reminder = Some(15);
    assert!(
        due_reminders(
            std::slice::from_ref(&item),
            &[],
            utc("2026-11-10T10:44:59Z")
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        due_reminders(
            std::slice::from_ref(&item),
            &[],
            utc("2026-11-10T10:45:00Z")
        )
        .unwrap()
        .len(),
        1
    );
    assert!(
        due_reminders(
            std::slice::from_ref(&item),
            &[],
            utc("2026-11-12T10:45:00Z")
        )
        .unwrap()
        .is_empty()
    );
    item.reminded = true;
    assert!(
        due_reminders(
            std::slice::from_ref(&item),
            &[],
            utc("2026-11-10T10:45:00Z")
        )
        .unwrap()
        .is_empty()
    );
    item.reminded = false;
    item.status = Status::Done;
    assert!(
        due_reminders(
            std::slice::from_ref(&item),
            &[],
            utc("2026-11-10T10:45:00Z")
        )
        .unwrap()
        .is_empty()
    );
    item.reminder = Some(525_601);
    assert!(due_reminders(&[item], &[], utc("2026-11-10T10:45:00Z")).is_err());
}

#[test]
fn occurrence_reminders_are_independent_and_honor_moves_and_cancellation() {
    let mut item = recurring();
    item.reminder = Some(30);
    let mut override_ = moved(
        &item,
        "2026-11-11 09:00",
        "2026-11-11 10:00",
        "2026-11-20 14:00",
        "2026-11-20 15:00",
    );
    assert_eq!(
        due_reminders(
            std::slice::from_ref(&item),
            &[override_.clone()],
            utc("2026-11-20T12:30:00Z")
        )
        .unwrap()
        .len(),
        1
    );
    override_.reminded = true;
    assert!(
        due_reminders(
            std::slice::from_ref(&item),
            &[override_.clone()],
            utc("2026-11-20T12:30:00Z")
        )
        .unwrap()
        .is_empty()
    );
    override_.reminded = false;
    override_.cancelled = true;
    assert!(
        due_reminders(&[item], &[override_], utc("2026-11-20T12:30:00Z"))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn all_day_spans_cover_the_entire_end_day() {
    let item = scheduled("All day", "2026-11-10", "2026-11-10");
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-11-10T10:00:00Z"),
        utc("2026-11-10T11:00:00Z"),
    )
    .unwrap();
    assert_eq!(entries.len(), 1);
}

#[test]
fn far_future_recurrence_is_still_a_nearest_deadline() {
    let mut item = scheduled("Future renewal", "2036-11-10 09:00", "2036-11-10 10:00");
    item.is_recurring = true;
    item.rrule = "FREQ=YEARLY;COUNT=2".into();
    item.deadline = Some(moment("2036-11-10 12:00"));
    let summary = dashboard(
        std::slice::from_ref(&item),
        &[],
        utc("2026-11-10T11:00:00Z"),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert_eq!(summary.nearest_deadlines, vec![item.id]);
    assert!(summary.upcoming.is_empty());
}

#[test]
fn recurrence_markers_preserve_wall_time_over_dst() {
    let mut item = scheduled("DST review", "2026-03-28 09:00", "2026-03-28 10:00");
    item.is_recurring = true;
    item.rrule = "FREQ=DAILY;COUNT=3".into();
    item.deadline = Some(moment("2026-03-28 12:00"));
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-03-29T00:00:00Z"),
        utc("2026-03-30T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(
        entries
            .iter()
            .find(|e| e.kind == CalendarKind::Deadline)
            .unwrap()
            .start
            .utc()
            .unwrap(),
        utc("2026-03-29T10:00:00Z")
    );
}

#[test]
fn hierarchy_is_iterative_for_very_deep_chains() {
    let mut items: Vec<Item> = Vec::new();
    for index in 0..10_000 {
        let mut item = Item::new(index.to_string());
        if let Some(parent) = items.last() {
            item.parents.push(parent.id);
        }
        items.push(item);
    }
    let rows = hierarchy(&items);
    assert_eq!(rows.len(), 10_000);
    assert_eq!(rows.last().unwrap().depth, 9_999);
}

#[test]
fn unscheduled_tasks_use_planned_then_deadline_for_reminders() {
    let mut planned = Item::new("Planned task");
    planned.planned = Some(moment("2026-11-10 12:00"));
    planned.deadline = Some(moment("2026-11-20 12:00"));
    planned.reminder = Some(15);
    let reminders = due_reminders(&[planned], &[], utc("2026-11-10T10:45:00Z")).unwrap();
    assert_eq!(reminders.len(), 1);
    assert!(reminders[0].occurrence.is_none());
    assert_eq!(reminders[0].at.value, "2026-11-10 12:00");
}

#[test]
fn timed_intervals_do_not_spill_into_the_next_day_at_midnight() {
    let item = scheduled("Late", "2026-11-10 22:00", "2026-11-11 00:00");
    let entries = calendar_entries(
        &[item],
        &[],
        utc("2026-11-10T23:00:00Z"),
        utc("2026-11-11T23:00:00Z"),
    )
    .unwrap();
    assert!(entries.is_empty());
}

#[test]
fn all_day_interval_crosses_a_missing_midnight() {
    let mut item = Item::new("Brazil all day");
    item.start = Some(Moment::parse("2018-11-03", "America/Sao_Paulo").unwrap());
    item.end = item.start.clone();
    assert_eq!(
        calendar_entries(
            &[item],
            &[],
            utc("2018-11-03T12:00:00Z"),
            utc("2018-11-04T12:00:00Z")
        )
        .unwrap()
        .len(),
        1
    );
}

#[test]
fn invalid_filter_ranges_and_properties_are_explicit_errors() {
    assert!(
        Filter {
            priority: Some(0),
            ..Filter::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Filter {
            severity: Some(6),
            ..Filter::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Filter {
            timezone: "Invalid/Timezone".into(),
            ..Filter::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Filter {
            date_from: NaiveDate::from_ymd_opt(2026, 12, 1),
            date_to: NaiveDate::from_ymd_opt(2026, 11, 1),
            ..Filter::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        Filter {
            priority: Some(1),
            severity: Some(5),
            timezone: "Europe/Budapest".into(),
            ..Filter::default()
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn calendar_all_day_bounds_convert_across_dst_and_date_line() {
    let day = Moment::parse("2026-03-29", "Europe/Budapest").unwrap();
    assert_eq!(
        calendar_moment(&day, "UTC", false).unwrap().value,
        "2026-03-28 23:00:00"
    );
    assert_eq!(
        calendar_moment(&day, "UTC", true).unwrap().value,
        "2026-03-29 21:59:00"
    );
    let day = Moment::parse("2030-01-10", "Pacific/Kiritimati").unwrap();
    assert_eq!(
        calendar_moment(&day, "America/Adak", false)
            .unwrap()
            .date()
            .unwrap()
            .to_string(),
        "2030-01-09"
    );
    assert_eq!(
        calendar_moment(&day, "America/Adak", true).unwrap().value,
        "2030-01-09 23:59:00"
    );
    let timed = Moment::parse("2030-01-10 09:15:20", "Asia/Tokyo").unwrap();
    assert_eq!(
        calendar_moment(&timed, "UTC", true).unwrap().utc().unwrap(),
        timed.utc().unwrap()
    );
}

#[test]
fn calendar_all_day_display_resolves_skipped_and_repeated_clock_boundaries() {
    let skipped_midnight = Moment::parse("2018-11-04", "America/Sao_Paulo").unwrap();
    assert_eq!(
        calendar_moment(&skipped_midnight, "UTC", false)
            .unwrap()
            .utc()
            .unwrap(),
        skipped_midnight.utc().unwrap()
    );
    let repeated_midnight = Moment::parse("2026-11-01", "America/Havana").unwrap();
    assert_eq!(
        calendar_moment(&repeated_midnight, "UTC", false)
            .unwrap()
            .utc()
            .unwrap(),
        repeated_midnight.utc().unwrap()
    );
    let before_skipped_date = Moment::parse("2011-12-29", "Pacific/Apia").unwrap();
    assert_eq!(
        calendar_moment(&before_skipped_date, "UTC", true)
            .unwrap()
            .value,
        "2011-12-30 09:59:00"
    );
}

#[test]
fn hierarchy_sorts_roots_and_siblings_by_name() {
    let z = Item::new("Zulu");
    let a = Item::new("alpha");
    let mut c = Item::new("Charlie");
    c.parents.push(a.id);
    let mut b = Item::new("bravo");
    b.parents.push(a.id);
    let expected = vec![a.id, b.id, c.id, z.id];
    assert_eq!(
        hierarchy(&[z, c, b, a])
            .into_iter()
            .map(|r| r.item_id)
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn dashboard_card_deadline_follows_rescheduled_occurrence() {
    let mut task = recurring();
    task.deadline = Some(moment("2026-11-10 12:00"));
    let event = moved(
        &task,
        "2026-11-10 09:00",
        "2026-11-10 10:00",
        "2026-11-15 14:00",
        "2026-11-15 15:00",
    );
    let entries = calendar_entries(
        &[task.clone()],
        &[event],
        utc("2026-11-15T00:00:00Z"),
        utc("2026-11-16T00:00:00Z"),
    )
    .unwrap();
    let scheduled = entries
        .iter()
        .find(|entry| entry.kind == CalendarKind::Scheduled)
        .unwrap();
    assert_eq!(
        dashboard_deadline(&task, Some(scheduled))
            .unwrap()
            .unwrap()
            .value,
        "2026-11-15 17:00:00"
    );
    assert_eq!(dashboard_deadline(&task, None).unwrap(), task.deadline);
    task.deadline = None;
    assert!(
        dashboard_deadline(&task, Some(scheduled))
            .unwrap()
            .is_none()
    );
}

#[test]
fn ongoing_wip_card_chooses_nearest_schedule_over_a_closer_marker() {
    let mut item = recurring();
    item.status = Status::Wip;
    item.planned = Some(moment("2026-11-10 07:59"));
    let summary = dashboard(
        std::slice::from_ref(&item),
        &[],
        moment("2026-11-11 08:00").utc().unwrap(),
        "Europe/Budapest",
        7,
    )
    .unwrap();
    assert_eq!(summary.ongoing, vec![item.id]);
    let representative = summary
        .representative(DashboardSection::Ongoing, item.id)
        .unwrap();
    assert_eq!(representative.kind, CalendarKind::Scheduled);
    assert_eq!(representative.start.value, "2026-11-11 09:00:00");
}
