//! Parsing and formatting of durations such as `1h 30m` or `2s 500ms`.

use std::time::Duration;

const NANOS_PER_SEC: u128 = 1_000_000_000;
const SECS_PER_MINUTE: u64 = 60;
const SECS_PER_HOUR: u64 = 3_600;
const SECS_PER_DAY: u64 = 86_400;
const SECS_PER_WEEK: u64 = 7 * SECS_PER_DAY;
/// 30.44 days.
const SECS_PER_MONTH: u64 = 2_630_016;
/// 365.25 days.
const SECS_PER_YEAR: u64 = 31_557_600;

/// Parses a concatenation of spans, each an integer or decimal number
/// followed by a unit (`1h 30m`, `2s500ms`, `1.5 hours`).
///
/// Returns `None` if the input is malformed or does not fit in a `Duration`.
pub(crate) fn parse(s: &str) -> Option<Duration> {
    if s == "0" {
        return Some(Duration::from_secs(0));
    }

    let mut cursor = Cursor::new(s);
    let mut total_nanos: u128 = 0;
    let mut has_span = false;
    loop {
        cursor.skip_whitespace();
        if cursor.peek().is_none() {
            break;
        }
        total_nanos = total_nanos.checked_add(parse_span(&mut cursor)?)?;
        has_span = true;
    }

    if has_span {
        duration_from_nanos(total_nanos)
    } else {
        None
    }
}

/// Formats a duration as a space separated list of spans, from years down to
/// nanoseconds, omitting the empty ones (`1h 30m`, `2s 500ms`).
pub(crate) fn format(duration: Duration) -> String {
    let secs = duration.as_secs();
    let nanos = duration.subsec_nanos();

    if secs == 0 && nanos == 0 {
        return "0s".to_owned();
    }

    let years = secs / SECS_PER_YEAR;
    let rest = secs % SECS_PER_YEAR;
    let months = rest / SECS_PER_MONTH;
    let rest = rest % SECS_PER_MONTH;
    let days = rest / SECS_PER_DAY;
    let rest = rest % SECS_PER_DAY;
    let hours = rest / SECS_PER_HOUR;
    let minutes = rest % SECS_PER_HOUR / SECS_PER_MINUTE;
    let seconds = rest % SECS_PER_MINUTE;

    let parts = [
        plural(years, "year"),
        plural(months, "month"),
        plural(days, "day"),
        span(hours, "h"),
        span(minutes, "m"),
        span(seconds, "s"),
        span(u64::from(nanos / 1_000_000), "ms"),
        span(u64::from(nanos / 1_000 % 1_000), "us"),
        span(u64::from(nanos % 1_000), "ns"),
    ];
    parts
        .iter()
        .filter_map(Option::as_deref)
        .collect::<Vec<_>>()
        .join(" ")
}

fn plural(value: u64, name: &str) -> Option<String> {
    let suffix = if value > 1 { "s" } else { "" };
    span(value, name).map(|s| s + suffix)
}

fn span(value: u64, unit: &str) -> Option<String> {
    if value == 0 {
        None
    } else {
        Some(format!("{}{}", value, unit))
    }
}

fn duration_from_nanos(nanos: u128) -> Option<Duration> {
    let secs = nanos / NANOS_PER_SEC;
    if secs > u128::from(u64::MAX) {
        return None;
    }
    Some(Duration::new(secs as u64, (nanos % NANOS_PER_SEC) as u32))
}

/// Parses one `<number>[.<fraction>]<unit>` span and returns its length in
/// nanoseconds.
fn parse_span(cursor: &mut Cursor) -> Option<u128> {
    let integer = cursor.integer()?;
    let fraction = if cursor.eat('.') {
        Some(cursor.fraction()?)
    } else {
        None
    };
    let unit_nanos = unit_nanos(cursor.unit()?)?;

    let mut nanos = u128::from(integer) * unit_nanos;
    if let Some((numerator, denominator)) = fraction {
        let scaled = u128::from(numerator) * unit_nanos;
        let denominator = u128::from(denominator);
        if scaled % denominator != 0 {
            // Sub-nanosecond precision cannot be represented.
            return None;
        }
        nanos += scaled / denominator;
    }
    Some(nanos)
}

fn unit_nanos(unit: &str) -> Option<u128> {
    let secs = |secs: u64| u128::from(secs) * NANOS_PER_SEC;
    let nanos = match unit {
        "nanos" | "nsec" | "ns" => 1,
        "usec" | "us" | "µs" => 1_000,
        "millis" | "msec" | "ms" => 1_000_000,
        "seconds" | "second" | "secs" | "sec" | "s" => secs(1),
        "minutes" | "minute" | "min" | "mins" | "m" => secs(SECS_PER_MINUTE),
        "hours" | "hour" | "hr" | "hrs" | "h" => secs(SECS_PER_HOUR),
        "days" | "day" | "d" => secs(SECS_PER_DAY),
        "weeks" | "week" | "wk" | "wks" | "w" => secs(SECS_PER_WEEK),
        "months" | "month" | "M" => secs(SECS_PER_MONTH),
        "years" | "year" | "yr" | "yrs" | "y" => secs(SECS_PER_YEAR),
        _ => return None,
    };
    Some(nanos)
}

struct Cursor<'a> {
    rest: &'a str,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a str) -> Self {
        Cursor { rest: src }
    }

    fn peek(&self) -> Option<char> {
        self.rest.chars().next()
    }

    fn bump(&mut self) {
        if let Some(c) = self.peek() {
            self.rest = &self.rest[c.len_utf8()..];
        }
    }

    fn eat(&mut self, expected: char) -> bool {
        let found = self.peek() == Some(expected);
        if found {
            self.bump();
        }
        found
    }

    fn skip_whitespace(&mut self) {
        self.rest = self.rest.trim_start_matches(char::is_whitespace);
    }

    /// Reads a number starting with a digit. Whitespace between its digits is
    /// ignored, so `1 5s` is fifteen seconds.
    fn integer(&mut self) -> Option<u64> {
        let mut value = digit(self.peek()?)?;
        self.bump();
        while let Some(c) = self.peek() {
            if let Some(d) = digit(c) {
                value = value.checked_mul(10)?.checked_add(d)?;
            } else if !c.is_whitespace() {
                break;
            }
            self.bump();
        }
        Some(value)
    }

    /// Reads the digits following a decimal separator as a
    /// `(numerator, denominator)` pair. At least one digit is required.
    fn fraction(&mut self) -> Option<(u64, u64)> {
        let mut numerator: u64 = 0;
        let mut denominator: u64 = 1;
        while let Some(c) = self.peek() {
            if let Some(d) = digit(c) {
                numerator = numerator.checked_mul(10)?.checked_add(d)?;
                denominator = denominator.checked_mul(10)?;
            } else if !c.is_whitespace() {
                break;
            }
            self.bump();
        }
        if denominator == 1 {
            None
        } else {
            Some((numerator, denominator))
        }
    }

    /// Reads a unit, which must be followed by whitespace, a digit (the start
    /// of the next span) or the end of the input.
    fn unit(&mut self) -> Option<&'a str> {
        let len = self
            .rest
            .find(|c: char| !is_unit_char(c))
            .unwrap_or(self.rest.len());
        let (unit, rest) = self.rest.split_at(len);
        self.rest = rest;

        match self.peek() {
            Some(c) if !c.is_whitespace() && digit(c).is_none() => None,
            _ => Some(unit),
        }
    }
}

fn is_unit_char(c: char) -> bool {
    c.is_ascii_alphabetic() || c == 'µ'
}

fn digit(c: char) -> Option<u64> {
    c.to_digit(10).map(u64::from)
}

#[cfg(test)]
mod test {
    use super::*;

    fn secs(secs: u64) -> Option<Duration> {
        Some(Duration::from_secs(secs))
    }

    fn nanos(nanos: u64) -> Option<Duration> {
        Some(Duration::from_nanos(nanos))
    }

    #[test]
    fn parses_every_unit_spelling() {
        for s in &["17nsec", "17nanos", "17ns"] {
            assert_eq!(parse(s), nanos(17), "{}", s);
        }
        for s in &["3usec", "3us", "3µs"] {
            assert_eq!(parse(s), nanos(3_000), "{}", s);
        }
        for s in &["3msec", "3millis", "3ms"] {
            assert_eq!(parse(s), nanos(3_000_000), "{}", s);
        }
        for s in &["3seconds", "3second", "3secs", "3sec", "3s"] {
            assert_eq!(parse(s), secs(3), "{}", s);
        }
        for s in &["3minutes", "3minute", "3min", "3mins", "3m"] {
            assert_eq!(parse(s), secs(180), "{}", s);
        }
        for s in &["3hours", "3hour", "3hr", "3hrs", "3h"] {
            assert_eq!(parse(s), secs(3 * 3_600), "{}", s);
        }
        for s in &["3days", "3day", "3d"] {
            assert_eq!(parse(s), secs(3 * 86_400), "{}", s);
        }
        for s in &["3weeks", "3week", "3wk", "3wks", "3w"] {
            assert_eq!(parse(s), secs(3 * 604_800), "{}", s);
        }
        for s in &["3months", "3month", "3M"] {
            assert_eq!(parse(s), secs(3 * 2_630_016), "{}", s);
        }
        for s in &["3years", "3year", "3yr", "3yrs", "3y"] {
            assert_eq!(parse(s), secs(3 * 31_557_600), "{}", s);
        }
    }

    #[test]
    fn parses_compound_durations() {
        assert_eq!(parse("1h 30m"), secs(5_400));
        assert_eq!(parse("2s 500ms"), Some(Duration::new(2, 500_000_000)));
        assert_eq!(parse("2h 37min"), secs(9_420));
        assert_eq!(parse("1hour 12min 5s"), secs(4_325));
        assert_eq!(parse("1h30m"), secs(5_400));
        assert_eq!(
            parse(
                "  1h 	 30m
"
            ),
            secs(5_400)
        );
        assert_eq!(parse("1 hour"), secs(3_600));
        assert_eq!(parse("1m 1m"), secs(120));
    }

    #[test]
    fn parses_zero() {
        assert_eq!(parse("0"), secs(0));
        assert_eq!(parse("0s"), secs(0));
        assert_eq!(parse("0h 0m"), secs(0));
    }

    #[test]
    fn parses_fractions() {
        assert_eq!(parse("4.2s"), Some(Duration::new(4, 200_000_000)));
        assert_eq!(parse("1.5h"), secs(5_400));
        assert_eq!(parse("0.5ms"), nanos(500_000));
        assert_eq!(parse("1.50s"), Some(Duration::new(1, 500_000_000)));
        assert_eq!(parse("1.000000001s"), Some(Duration::new(1, 1)));
    }

    #[test]
    fn rejects_malformed_input() {
        for s in &[
            "",
            " ",
            "1",
            "1 ",
            "h",
            "1.",
            ".5s",
            "1.5",
            "1x",
            "1 h m",
            "1h 2",
            "1h-",
            "-1h",
            "1h,2m",
            "1é",
            "1.0000000001s",
            "1.5ns",
            "1.1.1s",
            "1ss",
            "1H",
            "one hour",
        ] {
            assert_eq!(parse(s), None, "{:?}", s);
        }
    }

    #[test]
    fn rejects_overflow() {
        assert_eq!(parse("18446744073709551616s"), None);
        assert_eq!(parse("18446744073709551615y"), None);
        assert_eq!(parse("18446744073709551615s 1s"), None);
        assert_eq!(
            parse("18446744073709551615s"),
            secs(18_446_744_073_709_551_615)
        );
    }

    #[test]
    fn formats_spans_from_largest_to_smallest() {
        assert_eq!(format(Duration::from_secs(0)), "0s");
        assert_eq!(format(Duration::from_secs(1)), "1s");
        assert_eq!(format(Duration::from_secs(9_420)), "2h 37m");
        assert_eq!(format(Duration::from_secs(5_400)), "1h 30m");
        assert_eq!(format(Duration::from_millis(32)), "32ms");
        assert_eq!(format(Duration::new(2, 500_000_000)), "2s 500ms");
        assert_eq!(format(Duration::from_micros(1)), "1us");
        assert_eq!(format(Duration::from_nanos(1)), "1ns");
        assert_eq!(format(Duration::new(1, 1_001_001)), "1s 1ms 1us 1ns");
        assert_eq!(format(Duration::from_secs(86_400)), "1day");
        assert_eq!(format(Duration::from_secs(3 * 86_400)), "3days");
        assert_eq!(format(Duration::from_secs(2_630_016)), "1month");
        assert_eq!(format(Duration::from_secs(31_557_600)), "1year");
        assert_eq!(
            format(Duration::from_secs(2 * 31_557_600 + 2 * 2_630_016 + 3_661)),
            "2years 2months 1h 1m 1s"
        );
    }

    #[test]
    fn round_trips() {
        let samples = [
            Duration::from_secs(0),
            Duration::from_nanos(1),
            Duration::new(0, 999_999_999),
            Duration::new(59, 999_999_999),
            Duration::from_secs(3_599),
            Duration::from_secs(86_399),
            Duration::from_secs(31_557_599),
            Duration::from_secs(31_557_600),
            Duration::new(1_000_000_007, 123_456_789),
            Duration::new(u64::MAX, 999_999_999),
        ];
        for d in &samples {
            assert_eq!(parse(&format(*d)), Some(*d), "{:?}", d);
        }
    }
}
