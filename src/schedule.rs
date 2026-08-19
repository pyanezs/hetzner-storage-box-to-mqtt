use chrono::{DateTime, Days, Local, LocalResult, NaiveTime, TimeZone};

/// Parses one daily clock time in 24-hour `HH:MM` format (e.g. `"03:00"`).
pub fn parse_run_time(s: &str) -> Result<NaiveTime, String> {
    NaiveTime::parse_from_str(s, "%H:%M").map_err(|_| {
        format!("invalid time '{s}' — must be in 24-hour HH:MM format (e.g. \"03:00\", \"15:30\")")
    })
}

/// Computes the next run instant strictly after `now`, given daily clock times
/// interpreted in the local timezone. `times` need not be sorted/deduplicated.
///
/// DST handling: an ambiguous local time (fall-back overlap) resolves to the
/// earlier of the two real instants; a nonexistent local time (spring-forward
/// gap) is skipped in favor of the next candidate. Scanning two calendar days
/// is enough margin — a DST shift is at most a few hours, never a whole day.
pub fn next_run_after(now: DateTime<Local>, times: &[NaiveTime]) -> DateTime<Local> {
    assert!(
        !times.is_empty(),
        "next_run_after requires at least one time"
    );
    let mut sorted = times.to_vec();
    sorted.sort();
    sorted.dedup();

    let today = now.date_naive();
    for day_offset in 0..2 {
        let date = today + Days::new(day_offset);
        for t in &sorted {
            if let Some(candidate) = resolve_local(date.and_time(*t))
                && candidate > now
            {
                return candidate;
            }
        }
    }
    unreachable!("no resolvable run time found within 2 days of {now}");
}

fn resolve_local(naive: chrono::NaiveDateTime) -> Option<DateTime<Local>> {
    match Local.from_local_datetime(&naive) {
        LocalResult::Single(dt) => Some(dt),
        LocalResult::Ambiguous(earlier, _later) => Some(earlier),
        LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn parses_valid_time() {
        assert_eq!(parse_run_time("03:00"), Ok(t(3, 0)));
        assert_eq!(parse_run_time("15:30"), Ok(t(15, 30)));
    }

    #[test]
    fn rejects_invalid_time() {
        assert!(parse_run_time("25:00").is_err());
        assert!(parse_run_time("not-a-time").is_err());
    }

    #[test]
    fn picks_soonest_time_today() {
        let now = Local.with_ymd_and_hms(2026, 8, 19, 10, 0, 0).unwrap();
        let next = next_run_after(now, &[t(3, 0), t(15, 30)]);
        assert_eq!(
            next,
            Local.with_ymd_and_hms(2026, 8, 19, 15, 30, 0).unwrap()
        );
    }

    #[test]
    fn wraps_to_tomorrow_when_all_times_passed() {
        let now = Local.with_ymd_and_hms(2026, 8, 19, 23, 0, 0).unwrap();
        let next = next_run_after(now, &[t(3, 0), t(15, 30)]);
        assert_eq!(next, Local.with_ymd_and_hms(2026, 8, 20, 3, 0, 0).unwrap());
    }

    #[test]
    fn sorts_regardless_of_input_order() {
        let now = Local.with_ymd_and_hms(2026, 8, 19, 0, 0, 0).unwrap();
        let next = next_run_after(now, &[t(15, 30), t(3, 0), t(9, 0)]);
        assert_eq!(next, Local.with_ymd_and_hms(2026, 8, 19, 3, 0, 0).unwrap());
    }

    #[test]
    fn single_time_repeats_daily() {
        let now = Local.with_ymd_and_hms(2026, 8, 19, 3, 0, 1).unwrap();
        let next = next_run_after(now, &[t(3, 0)]);
        assert_eq!(next, Local.with_ymd_and_hms(2026, 8, 20, 3, 0, 0).unwrap());
    }
}
