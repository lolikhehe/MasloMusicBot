//! Small UTC helpers. The project deliberately avoids pulling in `chrono` just
//! to print a timestamp or read the current hour: both are pure integer math
//! on unix seconds and are needed by the history entries and digest scheduler.

/// Current unix time in seconds.
pub fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Hour of day in UTC (0..=23).
pub fn utc_hour(ts: i64) -> i16 {
    (ts.rem_euclid(86_400) / 3_600) as i16
}

/// Render unix seconds as `YYYY-MM-DD HH:MM` in UTC.
///
/// Uses the era-based civil-date algorithm ( Howard Hinnant ), valid for the
/// whole range of `i64` unix seconds and independent of the host timezone.
pub fn utc_stamp(ts: i64) -> String {
    let days = ts.div_euclid(86_400);
    let seconds = ts.rem_euclid(86_400);
    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let day = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let month = mp + if mp < 10 { 3 } else { -9 }; // [1, 12]
    if month <= 2 {
        year += 1;
    }

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// True when `hour` falls into the half-open quiet window `[start, end)`.
/// `start == end` means the window is disabled. Windows may wrap midnight.
pub fn in_quiet_window(hour: i16, start: i16, end: i16) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        (start..end).contains(&hour)
    } else {
        hour >= start || hour < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_timestamps() {
        let epoch = utc_stamp(0);
        assert!(epoch.starts_with("1970-01-01 00:00"), "{epoch}");
        let known = utc_stamp(1_700_000_000);
        assert!(known.starts_with("2023-11-14 22:13"), "{known}");
    }

    #[test]
    fn quiet_windows_handle_wrap() {
        assert!(!in_quiet_window(12, 23, 7));
        assert!(in_quiet_window(23, 23, 7));
        assert!(in_quiet_window(3, 23, 7));
        assert!(in_quiet_window(6, 23, 7));
        assert!(!in_quiet_window(7, 23, 7));
        assert!(!in_quiet_window(9, 9, 9));
        assert!(in_quiet_window(9, 9, 18));
    }
}
