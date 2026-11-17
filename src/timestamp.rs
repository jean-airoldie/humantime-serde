//! Parsing and formatting of UTC timestamps such as `2018-02-14T00:28:07Z`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SECS_PER_DAY: u64 = 86_400;
/// Days between 0000-03-01 and 1970-01-01.
const DAYS_TO_UNIX_EPOCH: u64 = 719_468;
const DAYS_PER_400_YEARS: u64 = 146_097;
/// 10000-01-01T00:00:00Z, the first instant that needs a fifth year digit.
const SECS_TO_YEAR_10000: u64 = 253_402_300_800;

/// Parses a UTC timestamp.
///
/// The date and time may be separated by `T` or a space, and the `Z` or
/// `+00:00` suffix is optional. Any other offset is rejected.
///
/// Returns `None` if the input is malformed or not representable.
pub(crate) fn parse(s: &str) -> Option<SystemTime> {
    let b = s.as_bytes();
    if b.len() < 19
        || b[4] != b'-'
        || b[7] != b'-'
        || (b[10] != b'T' && b[10] != b' ')
        || b[13] != b':'
        || b[16] != b':'
    {
        return None;
    }

    let year = digits(&b[0..4])?;
    let month = digits(&b[5..7])?;
    let day = digits(&b[8..10])?;
    let hour = digits(&b[11..13])?;
    let minute = digits(&b[14..16])?;
    let second = digits(&b[17..19])?;
    let nanos = parse_suffix(&b[19..])?;

    // A leap second is folded into the previous one.
    if year < 1970 || hour > 23 || minute > 59 || second > 60 {
        return None;
    }
    let second = second.min(59);
    if !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
    {
        return None;
    }

    let days = days_from_civil(year, month, day);
    let secs = days * SECS_PER_DAY + hour * 3_600 + minute * 60 + second;
    UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}

/// Formats a timestamp as RFC 3339 in UTC, with nine fractional digits when
/// it has a sub-second part and none otherwise.
///
/// Returns `None` for times before the Unix epoch or after year 9999.
pub(crate) fn format(time: SystemTime) -> Option<String> {
    let since_epoch = time.duration_since(UNIX_EPOCH).ok()?;
    let secs = since_epoch.as_secs();
    if secs >= SECS_TO_YEAR_10000 {
        return None;
    }

    let (year, month, day) = civil_from_days(secs / SECS_PER_DAY);
    let secs_of_day = secs % SECS_PER_DAY;
    let mut out = format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        year,
        month,
        day,
        secs_of_day / 3_600,
        secs_of_day / 60 % 60,
        secs_of_day % 60,
    );
    let nanos = since_epoch.subsec_nanos();
    if nanos != 0 {
        out.push_str(&format!(".{:09}", nanos));
    }
    out.push('Z');
    Some(out)
}

/// Parses the optional fractional seconds and UTC designator that follow the
/// seconds field, returning the nanoseconds.
fn parse_suffix(suffix: &[u8]) -> Option<u32> {
    let (nanos, zone) = match suffix.split_first() {
        Some((b'.', rest)) => {
            let len = rest.iter().take_while(|b| b.is_ascii_digit()).count();
            (fraction_nanos(&rest[..len]), &rest[len..])
        }
        _ => (0, suffix),
    };

    match zone {
        b"" | b"Z" | b"+00:00" => Some(nanos),
        _ => None,
    }
}

/// Converts fractional second digits to nanoseconds, truncating anything
/// beyond nanosecond precision.
fn fraction_nanos(digits: &[u8]) -> u32 {
    let mut nanos = 0;
    let mut scale = 100_000_000;
    for d in digits.iter().take(9) {
        nanos += scale * u32::from(d - b'0');
        scale /= 10;
    }
    nanos
}

/// Parses a fixed-width, all-digit field.
fn digits(field: &[u8]) -> Option<u64> {
    field.iter().try_fold(0, |acc, &b| {
        if b.is_ascii_digit() {
            Some(acc * 10 + u64::from(b - b'0'))
        } else {
            None
        }
    })
}

fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn is_leap_year(year: u64) -> bool {
    match (year % 400, year % 100, year % 4) {
        (0, _, _) => true,
        (_, 0, _) => false,
        (_, _, 0) => true,
        _ => false,
    }
}

/// Number of days from 1970-01-01 to the given date, which must not precede
/// it. See <https://howardhinnant.github.io/date_algorithms.html>.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year % 400;
    let shifted_month = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + day - 1;
    let day_of_era =
        year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * DAYS_PER_400_YEARS + day_of_era - DAYS_TO_UNIX_EPOCH
}

/// Inverse of `days_from_civil`.
fn civil_from_days(days: u64) -> (u64, u64, u64) {
    let days = days + DAYS_TO_UNIX_EPOCH;
    let era = days / DAYS_PER_400_YEARS;
    let day_of_era = days % DAYS_PER_400_YEARS;
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524
        - day_of_era / 146_096)
        / 365;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + if month <= 2 { 1 } else { 0 };
    (year, month, day)
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(secs: u64, nanos: u32) -> Option<SystemTime> {
        Some(UNIX_EPOCH + Duration::new(secs, nanos))
    }

    #[test]
    fn parses_timestamps() {
        assert_eq!(parse("1970-01-01T00:00:00Z"), at(0, 0));
        assert_eq!(parse("1970-01-01T00:00:01Z"), at(1, 0));
        assert_eq!(parse("2018-02-13T23:08:32Z"), at(1_518_563_312, 0));
        assert_eq!(parse("2012-01-01T00:00:00Z"), at(1_325_376_000, 0));
        assert_eq!(parse("9999-12-31T23:59:59Z"), at(253_402_300_799, 0));
    }

    #[test]
    fn parses_relaxed_forms() {
        let expected = at(1_526_063_310, 0);
        assert_eq!(parse("2018-05-11 18:28:30"), expected);
        assert_eq!(parse("2018-05-11T18:28:30"), expected);
        assert_eq!(parse("2018-05-11 18:28:30Z"), expected);
        assert_eq!(parse("2018-05-11 18:28:30+00:00"), expected);
        assert_eq!(parse("2018-05-11T18:28:30+00:00"), expected);
    }

    #[test]
    fn parses_fractional_seconds() {
        assert_eq!(parse("1970-01-01T00:00:00.123Z"), at(0, 123_000_000));
        assert_eq!(parse("1970-01-01 00:00:00.000123"), at(0, 123_000));
        assert_eq!(parse("1970-01-01T00:00:00.000123+00:00"), at(0, 123_000));
        assert_eq!(parse("1970-01-01T00:00:00.123456789Z"), at(0, 123_456_789));
        assert_eq!(
            parse("1970-01-01T00:00:00.1234567899Z"),
            at(0, 123_456_789)
        );
    }

    #[test]
    fn folds_leap_second() {
        assert_eq!(parse("2016-12-31T23:59:60Z"), at(1_483_228_799, 0));
    }

    #[test]
    fn rejects_malformed_timestamps() {
        for s in &[
            "",
            "1970-01-01",
            "1970-01-01T00:00",
            "1969-12-31T23:59:59Z",
            "1970-00-01T00:00:00Z",
            "1970-13-01T00:00:00Z",
            "1970-01-00T00:00:00Z",
            "1970-01-32T00:00:00Z",
            "1970-02-30T00:00:00Z",
            "2019-02-29T00:00:00Z",
            "1970-01-01T24:00:00Z",
            "1970-01-01T00:60:00Z",
            "1970-01-01T00:00:61Z",
            "1970/01/01T00:00:00Z",
            "1970-01-01X00:00:00Z",
            "1970-01-01T00:00:0xZ",
            "1970-01-01T00:00:00z",
            "1970-01-01T00:00:00+02:00",
            "1970-01-01T00:00:00+00",
            "1970-01-01T00:00:00 +00:00",
            "1970-01-01T00:00:00.1+02:00",
            "1970-01-01T00:00:00.1xZ",
            "1970-01-01T00:00:00Zjunk",
            "１９７０-01-01T00:00:00Z",
        ] {
            assert_eq!(parse(s), None, "{:?}", s);
        }
    }

    #[test]
    fn formats_timestamps() {
        let time = |secs, nanos| UNIX_EPOCH + Duration::new(secs, nanos);
        assert_eq!(format(time(0, 0)).unwrap(), "1970-01-01T00:00:00Z");
        assert_eq!(
            format(time(1_518_563_312, 0)).unwrap(),
            "2018-02-13T23:08:32Z"
        );
        assert_eq!(
            format(time(0, 123_000_000)).unwrap(),
            "1970-01-01T00:00:00.123000000Z"
        );
        assert_eq!(
            format(time(1_518_563_312, 789_456_123)).unwrap(),
            "2018-02-13T23:08:32.789456123Z"
        );
        assert_eq!(
            format(time(253_402_300_799, 0)).unwrap(),
            "9999-12-31T23:59:59Z"
        );
    }

    #[test]
    fn refuses_to_format_unrepresentable_times() {
        assert_eq!(format(UNIX_EPOCH - Duration::from_secs(1)), None);
        assert_eq!(
            format(UNIX_EPOCH + Duration::from_secs(253_402_300_800)),
            None
        );
    }

    #[test]
    fn round_trips_every_day_over_two_leap_cycles() {
        for day in 0..(366 + 365 + 365 + 365) * 4 {
            let time = UNIX_EPOCH + Duration::from_secs(day * 86_400 + 86_399);
            assert_eq!(parse(&format(time).unwrap()), Some(time));
        }
    }

    #[test]
    fn round_trips_across_the_whole_range() {
        let mut secs = 0;
        while secs < 253_402_300_800 {
            let time = UNIX_EPOCH + Duration::new(secs, (secs % 1_000) as u32);
            assert_eq!(parse(&format(time).unwrap()), Some(time));
            secs += 86_400 * 7 + 3_601;
        }
    }
}
