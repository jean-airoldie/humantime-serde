#![doc(html_root_url = "https://docs.rs/humantime-serde/1.1")]
#![forbid(unsafe_code)]

//! Serde support for human-readable durations and timestamps, using the
//! format of the `humantime` crate (`1h 30m`, `2018-01-01T12:53:00Z`).
//!
//! Based on [this fork](https://github.com/tailhook/serde-humantime/tree/serde_wrapper).
//!
//! Currently `std::time::{Duration, SystemTime}` are supported.
//!
//! # Formats
//!
//! A `Duration` is written as a list of spans from years down to nanoseconds
//! (`1h 30m`, `2s 500ms`) and read from any concatenation of
//! `<number><unit>` spans, where the number may be fractional (`1.5h`).
//! Units are `nsec`/`ns`, `usec`/`us`/`µs`, `msec`/`ms`, `sec`/`s`,
//! `min`/`m`, `hours`/`hour`/`hr`/`h`, `days`/`day`/`d`, `weeks`/`week`/`w`,
//! `months`/`month`/`M` (30.44 days) and `years`/`year`/`yr`/`y`
//! (365.25 days), plus a few spelling variants.
//!
//! A `SystemTime` is written as a UTC RFC 3339 timestamp
//! (`2018-02-14T00:28:07Z`) and read from the same format, or from the looser
//! `2018-02-14 00:28:07` where the `T` can be a space and the `Z` is optional.
//! Only UTC is supported, and only times between 1970 and 9999.
//!
//! # Example
//! ```
//! use serde::{Serialize, Deserialize};
//! use std::time::{Duration, SystemTime};
//!
//! #[derive(Serialize, Deserialize)]
//! struct Foo {
//!     #[serde(with = "humantime_serde")]
//!     timeout: Duration,
//!     #[serde(default)]
//!     #[serde(with = "humantime_serde")]
//!     time: Option<SystemTime>,
//! }
//! ```
//!
//! Or use the `Serde` wrapper type:
//!
//! ```
//! use serde::{Serialize, Deserialize};
//! use humantime_serde::Serde;
//! use std::time::SystemTime;
//!
//! #[derive(Serialize, Deserialize)]
//! struct Foo {
//!     timeout: Vec<Serde<SystemTime>>,
//! }
//! ```

mod duration;
mod timestamp;

pub mod option;

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::time::{Duration, SystemTime};

use serde::{de, ser, Deserialize, Deserializer, Serialize, Serializer};

/// Deserializes a `Duration` or `SystemTime` from its human-readable form.
///
/// This function can be used with `serde_derive`'s `with` and
/// `deserialize_with` annotations.
pub fn deserialize<'a, T, D>(d: D) -> Result<T, D::Error>
where
    Serde<T>: Deserialize<'a>,
    D: Deserializer<'a>,
{
    Serde::deserialize(d).map(Serde::into_inner)
}

/// Serializes a `Duration` or `SystemTime` to its human-readable form.
///
/// This function can be used with `serde_derive`'s `with` and
/// `serialize_with` annotations.
pub fn serialize<T, S>(d: &T, s: S) -> Result<S::Ok, S::Error>
where
    for<'a> Serde<&'a T>: Serialize,
    S: Serializer,
{
    Serde::from(d).serialize(s)
}

/// A wrapper type which implements `Serialize` and `Deserialize` for
/// types involving `SystemTime` and `Duration`.
#[derive(Copy, Clone, Eq, Hash, PartialEq)]
pub struct Serde<T>(T);

impl<T> fmt::Debug for Serde<T>
where
    T: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.0.fmt(formatter)
    }
}

impl<T> Deref for Serde<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Serde<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Serde<T> {
    /// Consumes the `De`, returning the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> From<T> for Serde<T> {
    fn from(val: T) -> Serde<T> {
        Serde(val)
    }
}

impl<'de> Deserialize<'de> for Serde<Duration> {
    fn deserialize<D>(d: D) -> Result<Serde<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de2> de::Visitor<'de2> for V {
            type Value = Duration;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("a duration")
            }

            fn visit_str<E>(self, v: &str) -> Result<Duration, E>
            where
                E: de::Error,
            {
                duration::parse(v).ok_or_else(|| {
                    E::invalid_value(de::Unexpected::Str(v), &self)
                })
            }
        }

        d.deserialize_str(V).map(Serde)
    }
}

impl<'de> Deserialize<'de> for Serde<SystemTime> {
    fn deserialize<D>(d: D) -> Result<Serde<SystemTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;

        impl<'de2> de::Visitor<'de2> for V {
            type Value = SystemTime;

            fn expecting(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
                fmt.write_str("a timestamp")
            }

            fn visit_str<E>(self, v: &str) -> Result<SystemTime, E>
            where
                E: de::Error,
            {
                timestamp::parse(v).ok_or_else(|| {
                    E::invalid_value(de::Unexpected::Str(v), &self)
                })
            }
        }

        d.deserialize_str(V).map(Serde)
    }
}

impl<'de> Deserialize<'de> for Serde<Option<Duration>> {
    fn deserialize<D>(d: D) -> Result<Serde<Option<Duration>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<Serde<Duration>>::deserialize(d)? {
            Some(Serde(dur)) => Ok(Serde(Some(dur))),
            None => Ok(Serde(None)),
        }
    }
}

impl<'de> Deserialize<'de> for Serde<Option<SystemTime>> {
    fn deserialize<D>(d: D) -> Result<Serde<Option<SystemTime>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Option::<Serde<SystemTime>>::deserialize(d)? {
            Some(Serde(dur)) => Ok(Serde(Some(dur))),
            None => Ok(Serde(None)),
        }
    }
}

impl<'a> ser::Serialize for Serde<&'a Duration> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        duration::format(*self.0).serialize(serializer)
    }
}

impl ser::Serialize for Serde<Duration> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        duration::format(self.0).serialize(serializer)
    }
}

impl<'a> ser::Serialize for Serde<&'a SystemTime> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serialize_system_time(*self.0, serializer)
    }
}

impl ser::Serialize for Serde<SystemTime> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        serialize_system_time(self.0, serializer)
    }
}

impl<'a> ser::Serialize for Serde<&'a Option<Duration>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self.0 {
            Some(dur) => serializer.serialize_some(&Serde(dur)),
            None => serializer.serialize_none(),
        }
    }
}

impl ser::Serialize for Serde<Option<Duration>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        Serde(&self.0).serialize(serializer)
    }
}

impl<'a> ser::Serialize for Serde<&'a Option<SystemTime>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        match *self.0 {
            Some(tm) => serializer.serialize_some(&Serde(tm)),
            None => serializer.serialize_none(),
        }
    }
}

impl ser::Serialize for Serde<Option<SystemTime>> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        Serde(&self.0).serialize(serializer)
    }
}

fn serialize_system_time<S>(
    time: SystemTime,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: ser::Serializer,
{
    match timestamp::format(time) {
        Some(formatted) => formatted.serialize(serializer),
        None => Err(ser::Error::custom(
            "system time is outside the supported range of 1970 to 9999",
        )),
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn with() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super")]
            time: Duration,
        }

        let json = r#"{"time": "15 seconds"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, Duration::from_secs(15));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"15s"}"#);
    }

    #[test]
    fn with_option() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super", default)]
            time: Option<Duration>,
        }

        let json = r#"{"time": "15 seconds"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, Some(Duration::from_secs(15)));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"15s"}"#);

        let json = r#"{"time": null}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, None);
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":null}"#);

        let json = r#"{}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, None);
    }

    #[test]
    fn time() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super")]
            time: SystemTime,
        }

        let json = r#"{"time": "2018-05-11 18:28:30"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, UNIX_EPOCH + Duration::new(1526063310, 0));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"2018-05-11T18:28:30Z"}"#);
    }

    #[test]
    fn time_with_option() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super", default)]
            time: Option<SystemTime>,
        }

        let json = r#"{"time": "2018-05-11 18:28:30"}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, Some(UNIX_EPOCH + Duration::new(1526063310, 0)));
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":"2018-05-11T18:28:30Z"}"#);

        let json = r#"{"time": null}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, None);
        let reverse = serde_json::to_string(&foo).unwrap();
        assert_eq!(reverse, r#"{"time":null}"#);

        let json = r#"{}"#;
        let foo = serde_json::from_str::<Foo>(json).unwrap();
        assert_eq!(foo.time, None);
    }

    #[test]
    fn duration_formats_and_units() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "super")]
            time: Duration,
        }

        for (input, expected) in &[
            ("1h 30m", "1h 30m"),
            ("2s 500ms", "2s 500ms"),
            ("1hour", "1h"),
            ("1us", "1us"),
            ("90 minutes", "1h 30m"),
            ("1.5h", "1h 30m"),
            ("0", "0s"),
            ("1year 1month", "1year 1month"),
        ] {
            let json = format!(r#"{{"time":"{}"}}"#, input);
            let foo = serde_json::from_str::<Foo>(&json).unwrap();
            let reverse = serde_json::to_string(&foo).unwrap();
            assert_eq!(reverse, format!(r#"{{"time":"{}"}}"#, expected));
            let again = serde_json::from_str::<Foo>(&reverse).unwrap();
            assert_eq!(again.time, foo.time);
        }
    }

    #[test]
    fn invalid_values_are_rejected() {
        #[derive(Debug, Deserialize)]
        struct Dur {
            #[serde(with = "super")]
            _time: Duration,
        }

        #[derive(Debug, Deserialize)]
        struct Time {
            #[serde(with = "super")]
            _time: SystemTime,
        }

        let err = serde_json::from_str::<Dur>(r#"{"_time":"soon"}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected a duration"), "{}", err);

        let err = serde_json::from_str::<Time>(r#"{"_time":"soon"}"#)
            .unwrap_err()
            .to_string();
        assert!(err.contains("expected a timestamp"), "{}", err);
    }

    #[test]
    fn unrepresentable_system_time_is_a_serialization_error() {
        let before_epoch = UNIX_EPOCH - Duration::from_secs(1);
        let err = serde_json::to_string(&Serde::from(before_epoch))
            .unwrap_err()
            .to_string();
        assert!(err.contains("supported range"), "{}", err);
    }

    #[test]
    fn wrapper_and_collections() {
        let json = r#"["1h 30m","2s 500ms"]"#;
        let durations =
            serde_json::from_str::<Vec<Serde<Duration>>>(json).unwrap();
        assert_eq!(*durations[0], Duration::from_secs(5400));
        assert_eq!(*durations[1], Duration::new(2, 500_000_000));
        assert_eq!(serde_json::to_string(&durations).unwrap(), json);

        let json = r#"["2018-01-01T12:53:00Z","2018-01-01T12:53:00.5Z"]"#;
        let times =
            serde_json::from_str::<Vec<Serde<SystemTime>>>(json).unwrap();
        assert_eq!(
            serde_json::to_string(&times).unwrap(),
            r#"["2018-01-01T12:53:00Z","2018-01-01T12:53:00.500000000Z"]"#
        );
    }

    #[test]
    fn option_module() {
        #[derive(Serialize, Deserialize)]
        struct Foo {
            #[serde(with = "crate::option", default)]
            timeout: Option<Duration>,
            #[serde(with = "crate::option", default)]
            time: Option<SystemTime>,
        }

        let foo = serde_json::from_str::<Foo>(
            r#"{"timeout":"1m","time":"2018-05-11 18:28:30"}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_string(&foo).unwrap(),
            r#"{"timeout":"1m","time":"2018-05-11T18:28:30Z"}"#
        );

        let foo = serde_json::from_str::<Foo>("{}").unwrap();
        assert_eq!(
            serde_json::to_string(&foo).unwrap(),
            r#"{"timeout":null,"time":null}"#
        );
    }

    #[test]
    fn test_readme_deps() {
        version_sync::assert_markdown_deps_updated!("README.md");
    }

    #[test]
    fn test_html_root_url() {
        version_sync::assert_html_root_url_updated!("src/lib.rs");
    }
}
