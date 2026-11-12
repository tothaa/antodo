use antodo::core::{EventOverride, Item, Moment, Status, recurrence};
use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

fn moment(value: &str) -> Moment {
    Moment::parse(value, "Europe/Budapest").unwrap()
}
fn utc(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}
fn task(rule: &str) -> Item {
    let mut task = Item::new("Team planning");
    task.start = Some(moment("2026-01-01 09:00"));
    task.end = Some(moment("2026-01-01 10:00"));
    task.is_recurring = true;
    task.rrule = rule.into();
    task
}
fn events(task: &Item) -> Vec<antodo::core::Occurrence> {
    recurrence::expand(
        task,
        &[],
        utc("2025-01-01T00:00:00Z"),
        utc("2028-01-01T00:00:00Z"),
    )
    .unwrap()
}
fn change(task: &Item) -> EventOverride {
    let event = events(task).remove(0);
    EventOverride {
        id: Uuid::now_v7(),
        item_id: task.id,
        original_start: event.original_start,
        original_end: event.original_end,
        new_start: None,
        new_end: None,
        cancelled: false,
        status: Status::Pending,
        reminded: false,
    }
}

#[test]
fn daily_count_and_timed_until_are_finite_and_inclusive() {
    let daily = task("FREQ=DAILY;COUNT=3");
    let result = events(&daily);
    assert_eq!(result.len(), 3);
    assert_eq!(result[2].start.value, "2026-01-03 09:00:00");
    let until = task("FREQ=DAILY;UNTIL=20260103T080000Z");
    assert_eq!(
        events(&until),
        result
            .iter()
            .cloned()
            .map(|mut event| {
                event.item_id = until.id;
                event
            })
            .collect::<Vec<_>>()
    );
}

#[test]
fn monthly_last_workday_and_weekly_byday_follow_rfc() {
    let mut monthly = task("FREQ=MONTHLY;BYDAY=MO,TU,WE,TH,FR;BYSETPOS=-1;COUNT=3");
    monthly.start = Some(moment("2026-01-30 09:00"));
    monthly.end = Some(moment("2026-01-30 10:00"));
    let result = events(&monthly);
    assert_eq!(
        result
            .iter()
            .map(|e| e.start.value.as_str())
            .collect::<Vec<_>>(),
        vec![
            "2026-01-30 09:00:00",
            "2026-02-27 09:00:00",
            "2026-03-31 09:00:00"
        ]
    );
    let mut weekly = task("FREQ=WEEKLY;BYDAY=MO,WE;COUNT=4");
    weekly.start = Some(moment("2026-01-05 09:00"));
    weekly.end = Some(moment("2026-01-05 10:00"));
    assert_eq!(events(&weekly)[0].start.value, "2026-01-05 09:00:00");
    assert_eq!(events(&weekly)[3].start.value, "2026-01-14 09:00:00");
}

#[test]
fn leap_year_monthday_and_intervals_work() {
    let mut yearly = task("FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=29;COUNT=2");
    yearly.start = Some(moment("2024-02-29 09:00"));
    yearly.end = Some(moment("2024-02-29 10:00"));
    let result = recurrence::expand(
        &yearly,
        &[],
        utc("2024-01-01T00:00:00Z"),
        utc("2029-01-01T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(result.len(), 2);
    assert!(result[1].start.value.starts_with("2028-02-29"));
    let every_other = task("RRULE:FREQ=DAILY;INTERVAL=2;COUNT=3");
    assert!(
        events(&every_other)[2]
            .start
            .value
            .starts_with("2026-01-05")
    );
    let mut monthday = task("FREQ=MONTHLY;BYMONTHDAY=31;COUNT=3");
    monthday.start = Some(moment("2026-01-31 09:00"));
    monthday.end = Some(moment("2026-01-31 10:00"));
    assert!(events(&monthday)[2].start.value.starts_with("2026-05-31"));
}

#[test]
fn daily_all_day_skips_a_missing_calendar_day() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(Moment::parse("2011-12-29", "Pacific/Apia").unwrap());
    daily.end = daily.start.clone();
    let result = recurrence::expand(
        &daily,
        &[],
        utc("2011-12-28T00:00:00Z"),
        utc("2012-01-03T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[1].start.value, "2011-12-31");
    assert_eq!(result[2].start.value, "2012-01-01");
    assert_eq!(
        recurrence::interval_end(daily.end.as_ref().unwrap()).unwrap(),
        utc("2011-12-30T10:00:00Z")
    );
}

#[test]
fn rejects_unbounded_zero_duplicate_and_invalid_rules() {
    for rule in [
        "",
        "FREQ=DAILY",
        "FREQ=DAILY;COUNT=0",
        "FREQ=DAILY;COUNT=10001",
        "FREQ=DAILY;COUNT=2;UNTIL=20260105T000000Z",
        "FREQ=DAILY;COUNT=2;COUNT=3",
        "FREQ=DAILY;COUNT=2;INTERVAL=0",
        "FREQ=DAILY;COUNT=2\nRRULE:FREQ=DAILY",
        "FREQ=DAILY;COUNT=2;BOGUS=1",
        "FREQ=YEARLY;COUNT=2;BYMONTH=13",
        "FREQ=DAILY;UNTIL=20260103",
        "FREQ=DAILY;UNTIL=20250103T080000Z",
        "FREQ=SECONDLY;UNTIL=20270103T080000Z",
        "FREQ=DAILY;COUNT=2;",
    ] {
        assert!(
            recurrence::validate_rule(&task(rule)).is_err(),
            "accepted {rule}"
        );
    }
}

#[test]
fn rejects_safety_limit_without_silent_truncation() {
    let many = task("FREQ=DAILY;UNTIL=20660101T080000Z");
    assert!(
        recurrence::validate_rule(&many)
            .unwrap_err()
            .to_string()
            .contains("safety limit")
    );
    let impossible = task("FREQ=YEARLY;COUNT=1;BYMONTH=2;BYMONTHDAY=30");
    assert!(recurrence::validate_rule(&impossible).is_err());
}

#[test]
fn retains_local_clock_time_across_dst() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(moment("2026-03-28 09:00"));
    daily.end = Some(moment("2026-03-28 10:00"));
    let result = events(&daily);
    assert_eq!(result[0].start.utc().unwrap(), utc("2026-03-28T08:00:00Z"));
    assert_eq!(result[1].start.utc().unwrap(), utc("2026-03-29T07:00:00Z"));
    assert!(result.iter().all(|e| e.start.value.ends_with("09:00:00")));
    assert!(result.iter().all(|e| e.start.timezone == "Europe/Budapest"));
}

#[test]
fn skips_generated_dst_gap_without_consuming_count() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(moment("2026-03-28 02:30"));
    daily.end = Some(moment("2026-03-28 03:30"));
    let result = events(&daily);
    assert_eq!(result.len(), 3);
    assert_eq!(result[1].start.value, "2026-03-30 02:30:00");
    assert_eq!(result[2].start.value, "2026-03-31 02:30:00");
}

#[test]
fn generated_repeated_time_uses_first_instant() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(moment("2026-10-24 02:30"));
    daily.end = Some(moment("2026-10-24 02:45"));
    let result = events(&daily);
    assert_eq!(result[1].start.utc().unwrap(), utc("2026-10-25T00:30:00Z"));
    assert!(Moment::parse("2026-10-25 02:30", "Europe/Budapest").is_err());
}

#[test]
fn recurring_duration_retains_second_fold_end_and_serializes_it() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(moment("2026-10-24 01:30"));
    daily.end = Some(moment("2026-10-24 03:30"));
    let result = events(&daily);
    assert_eq!(result[1].start.utc().unwrap(), utc("2026-10-24T23:30:00Z"));
    assert_eq!(result[1].end.utc().unwrap(), utc("2026-10-25T01:30:00Z"));
    assert_eq!(result[1].end.value, "2026-10-25 02:30:00");
    assert_eq!(result[1].end.offset_seconds, Some(3600));
    assert_eq!(
        result[1].end.utc().unwrap() - result[1].start.utc().unwrap(),
        Duration::hours(2)
    );
    let serialized = serde_json::to_string(&result[1].end).unwrap();
    let saved: Moment = serde_json::from_str(&serialized).unwrap();
    assert_eq!(saved.utc().unwrap(), utc("2026-10-25T01:30:00Z"));
    let legacy: Moment =
        serde_json::from_str(r#"{"value":"2026-10-24 09:00","timezone":"Europe/Budapest"}"#)
            .unwrap();
    assert!(legacy.offset_seconds.is_none());
    let mut invalid = saved;
    invalid.offset_seconds = Some(123);
    assert!(invalid.utc().is_err());
}

#[test]
fn all_day_date_exists_even_if_its_midnight_is_skipped() {
    let day = Moment::parse("2018-11-04", "America/Sao_Paulo").unwrap();
    assert_eq!(day.utc().unwrap(), utc("2018-11-04T03:00:00Z"));
    assert!(Moment::parse("2018-11-04 00:30", "America/Sao_Paulo").is_err());
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.start = Some(Moment::parse("2018-11-03", "America/Sao_Paulo").unwrap());
    daily.end = daily.start.clone();
    let result = recurrence::expand(
        &daily,
        &[],
        utc("2018-11-01T00:00:00Z"),
        utc("2018-11-07T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(result.len(), 3);
    assert_eq!(result[1].start.value, "2018-11-04");
    assert_eq!(
        recurrence::interval_end(&result[0].end).unwrap(),
        day.utc().unwrap()
    );
}

#[test]
fn date_only_until_keeps_precision_and_inclusive_end_across_dst() {
    let mut daily = task("FREQ=DAILY;UNTIL=20260330");
    daily.start = Some(moment("2026-03-28"));
    daily.end = Some(moment("2026-03-28"));
    let result = events(&daily);
    assert_eq!(result.len(), 3);
    assert!(result.iter().all(|e| e.start.is_date() && e.end.is_date()));
    let midday = recurrence::expand(
        &daily,
        &[],
        utc("2026-03-29T12:00:00Z"),
        utc("2026-03-29T13:00:00Z"),
    )
    .unwrap();
    assert_eq!(midday.len(), 1);
    assert_eq!(midday[0].start.value, "2026-03-29");
    for rule in [
        "FREQ=HOURLY;COUNT=3",
        "FREQ=DAILY;COUNT=3;BYHOUR=9",
        "FREQ=DAILY;UNTIL=20260330T000000Z",
    ] {
        daily.rrule = rule.into();
        assert!(recurrence::validate_rule(&daily).is_err());
    }
}

#[test]
fn moved_occurrence_enters_window_and_original_leaves() {
    let daily = task("FREQ=DAILY;COUNT=3");
    let mut moved = change(&daily);
    moved.new_start = Some(moment("2026-04-01 14:00"));
    moved.new_end = Some(moment("2026-04-01 15:00"));
    moved.status = Status::Wip;
    moved.reminded = true;
    recurrence::validate_override(&daily, &moved).unwrap();
    let april = recurrence::expand(
        &daily,
        &[moved.clone()],
        utc("2026-04-01T00:00:00Z"),
        utc("2026-04-02T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(april.len(), 1);
    assert_eq!(april[0].status, Status::Wip);
    assert!(april[0].reminded && april[0].overridden);
    assert_eq!(april[0].original_start.value, "2026-01-01 09:00:00");
    let original = recurrence::expand(
        &daily,
        &[moved],
        utc("2026-01-01T00:00:00Z"),
        utc("2026-01-02T00:00:00Z"),
    )
    .unwrap();
    assert!(original.is_empty());
}

#[test]
fn cancellation_and_independent_status_and_reminders() {
    let mut daily = task("FREQ=DAILY;COUNT=3");
    daily.reminded = true;
    assert!(events(&daily).iter().all(|event| !event.reminded));
    let mut cancelled = change(&daily);
    cancelled.cancelled = true;
    let mut done = change(&daily);
    let second = &events(&daily)[1];
    done.original_start = second.original_start.clone();
    done.original_end = second.original_end.clone();
    done.status = Status::Done;
    done.reminded = true;
    let result = recurrence::expand(
        &daily,
        &[cancelled, done],
        utc("2026-01-01T00:00:00Z"),
        utc("2026-02-01T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].status, Status::Done);
    assert!(result[0].reminded);
    assert_eq!(result[1].status, Status::Pending);
}

#[test]
fn invalid_or_duplicate_overrides_are_rejected() {
    let daily = task("FREQ=DAILY;COUNT=3");
    let original = change(&daily);
    let mut bad = original.clone();
    bad.original_start = moment("2026-02-01 09:00");
    assert!(recurrence::validate_override(&daily, &bad).is_err());
    bad = original.clone();
    bad.original_end = moment("2026-01-01 12:00");
    assert!(recurrence::validate_override(&daily, &bad).is_err());
    bad = original.clone();
    bad.new_start = Some(moment("2026-01-04 09:00"));
    assert!(recurrence::validate_override(&daily, &bad).is_err());
    bad.new_end = Some(moment("2026-01-03 09:00"));
    assert!(recurrence::validate_override(&daily, &bad).is_err());
    bad = original.clone();
    bad.item_id = Uuid::now_v7();
    assert!(recurrence::validate_override(&daily, &bad).is_err());
    assert!(
        recurrence::expand(
            &daily,
            &[original.clone(), original],
            utc("2026-01-01T00:00:00Z"),
            utc("2027-01-01T00:00:00Z")
        )
        .is_err()
    );
}

#[test]
fn semantic_original_times_accept_minute_format_and_ignore_other_masters() {
    let daily = task("FREQ=DAILY;COUNT=3");
    let mut edited = change(&daily);
    edited.original_start = moment("2026-01-01 09:00");
    edited.original_end = moment("2026-01-01 10:00");
    recurrence::validate_override(&daily, &edited).unwrap();
    let unrelated = change(&task("FREQ=DAILY;COUNT=3"));
    assert_eq!(
        recurrence::expand(
            &daily,
            &[unrelated],
            utc("2026-01-01T00:00:00Z"),
            utc("2027-01-01T00:00:00Z")
        )
        .unwrap()
        .len(),
        3
    );
}

#[test]
fn nonrecurring_overlap_zero_length_and_invalid_ranges() {
    let mut item = task("");
    item.is_recurring = false;
    item.reminded = true;
    let at = item.start.as_ref().unwrap().utc().unwrap();
    assert!(
        recurrence::expand(
            &item,
            &[],
            at + Duration::minutes(30),
            at + Duration::hours(2)
        )
        .unwrap()[0]
            .reminded
    );
    assert!(
        recurrence::expand(&item, &[], at + Duration::hours(1), at + Duration::hours(2))
            .unwrap()
            .is_empty()
    );
    item.end = item.start.clone();
    assert_eq!(
        recurrence::expand(&item, &[], at, at + Duration::seconds(1))
            .unwrap()
            .len(),
        1
    );
    assert!(recurrence::expand(&item, &[], at, at).is_err());
    item.end = None;
    assert!(recurrence::validate_rule(&item).is_err());
    item.start = None;
    assert!(recurrence::validate_rule(&item).is_ok());
    assert!(events(&item).is_empty());
    item.is_recurring = true;
    assert!(recurrence::validate_rule(&item).is_err());
}

#[test]
fn initial_date_must_match_the_recurrence_pattern_and_counts_once() {
    let mismatch = task("FREQ=WEEKLY;BYDAY=MO;COUNT=3");
    assert!(
        recurrence::validate_rule(&mismatch)
            .unwrap_err()
            .to_string()
            .contains("first occurrence")
    );
    let matching = task("FREQ=WEEKLY;BYDAY=TH;COUNT=1");
    let result = events(&matching);
    assert_eq!(result.len(), 1);
    assert_eq!(
        result[0].start.utc().unwrap(),
        matching.start.unwrap().utc().unwrap()
    );
}

#[test]
fn explicit_dtstart_offset_is_not_changed_by_generated_fold_policy() {
    let mut daily = task("FREQ=DAILY;COUNT=2");
    daily.start =
        Some(Moment::from_utc(utc("2026-10-25T01:30:00Z"), "Europe/Budapest", false).unwrap());
    daily.end =
        Some(Moment::from_utc(utc("2026-10-25T01:45:00Z"), "Europe/Budapest", false).unwrap());
    let result = events(&daily);
    assert_eq!(result[0].start.utc().unwrap(), utc("2026-10-25T01:30:00Z"));
    assert_eq!(result[1].start.utc().unwrap(), utc("2026-10-26T01:30:00Z"));
}

#[test]
fn until_supports_weekly_monthly_and_yearly_inclusive_endings() {
    for (rule, dates) in [
        (
            "FREQ=WEEKLY;UNTIL=20260115T080000Z",
            ["2026-01-01", "2026-01-08", "2026-01-15"],
        ),
        (
            "FREQ=MONTHLY;UNTIL=20260301T080000Z",
            ["2026-01-01", "2026-02-01", "2026-03-01"],
        ),
        (
            "FREQ=YEARLY;UNTIL=20280101T080000Z",
            ["2026-01-01", "2027-01-01", "2028-01-01"],
        ),
    ] {
        let result = recurrence::expand(
            &task(rule),
            &[],
            utc("2026-01-01T00:00:00Z"),
            utc("2029-01-01T00:00:00Z"),
        )
        .unwrap();
        assert_eq!(
            result
                .iter()
                .map(|event| &event.start.value[..10])
                .collect::<Vec<_>>(),
            dates,
            "{rule}"
        );
    }
}

#[test]
fn direct_recurrence_validation_rejects_invalid_date_pairs_and_complexity() {
    let mut item = task("FREQ=DAILY;COUNT=2");
    item.end = Some(moment("2025-12-31 10:00"));
    assert!(
        recurrence::validate_rule(&item)
            .unwrap_err()
            .to_string()
            .contains("before")
    );
    item.end = Some(moment("2026-01-02"));
    assert!(
        recurrence::validate_rule(&item)
            .unwrap_err()
            .to_string()
            .contains("both be dates")
    );
    item.start = Some(Moment::parse("0000-01-01", "UTC").unwrap());
    item.end = item.start.clone();
    assert!(
        recurrence::validate_rule(&item)
            .unwrap_err()
            .to_string()
            .contains("0001 and 9999")
    );

    let values = (0..11)
        .map(|value| value.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let rule = format!("FREQ=DAILY;COUNT=2;BYHOUR={values};BYMINUTE={values};BYSECOND={values}");
    assert!(
        recurrence::validate_rule(&task(&rule))
            .unwrap_err()
            .to_string()
            .contains("time combinations")
    );
    assert!(
        recurrence::validate_rule(&task("FREQ=UNKNOWN;UNTIL=20260103T080000Z"))
            .unwrap_err()
            .to_string()
            .contains("Unknown recurrence frequency")
    );
}

#[test]
fn an_impossible_until_series_returns_an_error_instead_of_an_empty_calendar() {
    let item = task("FREQ=YEARLY;BYMONTH=2;BYMONTHDAY=30;UNTIL=20270101T080000Z");
    assert!(
        recurrence::validate_rule(&item)
            .unwrap_err()
            .to_string()
            .contains("no occurrences")
    );
}

#[test]
fn a_missing_day_cannot_exhaust_candidates_and_return_a_partial_count() {
    let mut item = task("FREQ=SECONDLY;COUNT=2");
    item.start = Some(Moment::parse("2011-12-29 23:59:59", "Pacific/Apia").unwrap());
    item.end = item.start.clone();
    assert!(
        recurrence::validate_rule(&item)
            .unwrap_err()
            .to_string()
            .contains("candidate safety limit")
    );
}
