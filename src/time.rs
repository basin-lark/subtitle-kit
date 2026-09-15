//! Timestamps as they appear in SRT subtitle files: `HH:MM:SS,mmm`.
//!
//! A `Timestamp` is stored as a single millisecond count rather than
//! separate hour/minute/second/millisecond fields. That makes shifting,
//! comparing, and sorting a matter of comparing one integer instead of
//! juggling carries across four fields.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp {
    millis: u32,
}

impl Timestamp {
    /// Builds a timestamp from hours/minutes/seconds/milliseconds.
    ///
    /// Values are not range-checked (minutes >= 60 will simply roll into
    /// hours) because the total millisecond count is the only thing that
    /// actually matters downstream.
    pub fn new(hours: u32, minutes: u32, seconds: u32, milliseconds: u32) -> Timestamp {
        let total = hours as u64 * 3_600_000
            + minutes as u64 * 60_000
            + seconds as u64 * 1_000
            + milliseconds as u64;
        Timestamp::from_millis(total.min(u32::MAX as u64) as u32)
    }

    pub fn from_millis(millis: u32) -> Timestamp {
        Timestamp { millis }
    }

    pub fn as_millis(&self) -> u32 {
        self.millis
    }

    pub fn hours(&self) -> u32 {
        self.millis / 3_600_000
    }

    pub fn minutes(&self) -> u32 {
        (self.millis / 60_000) % 60
    }

    pub fn seconds(&self) -> u32 {
        (self.millis / 1_000) % 60
    }

    pub fn subsecond_millis(&self) -> u32 {
        self.millis % 1_000
    }

    /// Returns a new timestamp offset by `offset_millis`, clamped to
    /// `[0, u32::MAX]` instead of wrapping or overflowing.
    pub fn shifted(&self, offset_millis: i64) -> Timestamp {
        let total = self.millis as i64 + offset_millis;
        let clamped = total.clamp(0, u32::MAX as i64);
        Timestamp::from_millis(clamped as u32)
    }

    /// Parses `HH:MM:SS,mmm`. A `.` is also accepted in place of the `,`
    /// since that is what WebVTT uses and stray files show up with it.
    pub fn parse(input: &str) -> Result<Timestamp, TimeParseError> {
        let trimmed = input.trim();
        let normalized = trimmed.replace('.', ",");
        let (hms, millis_part) = normalized
            .split_once(',')
            .ok_or_else(|| TimeParseError::InvalidFormat(trimmed.to_string()))?;

        let mut fields = hms.split(':');
        let hours = parse_field(fields.next(), trimmed)?;
        let minutes = parse_field(fields.next(), trimmed)?;
        let seconds = parse_field(fields.next(), trimmed)?;
        if fields.next().is_some() {
            return Err(TimeParseError::InvalidFormat(trimmed.to_string()));
        }
        if minutes >= 60 || seconds >= 60 {
            return Err(TimeParseError::OutOfRange(trimmed.to_string()));
        }

        let milliseconds: u32 = millis_part
            .parse()
            .map_err(|_| TimeParseError::InvalidFormat(trimmed.to_string()))?;

        Ok(Timestamp::new(hours, minutes, seconds, milliseconds))
    }

    pub fn format(&self) -> String {
        format!(
            "{:02}:{:02}:{:02},{:03}",
            self.hours(),
            self.minutes(),
            self.seconds(),
            self.subsecond_millis()
        )
    }
}

fn parse_field(field: Option<&str>, original: &str) -> Result<u32, TimeParseError> {
    field
        .ok_or_else(|| TimeParseError::InvalidFormat(original.to_string()))?
        .parse()
        .map_err(|_| TimeParseError::InvalidFormat(original.to_string()))
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.format())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeParseError {
    InvalidFormat(String),
    OutOfRange(String),
}

impl fmt::Display for TimeParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeParseError::InvalidFormat(s) => write!(f, "invalid timestamp format: {s:?}"),
            TimeParseError::OutOfRange(s) => {
                write!(f, "minutes or seconds out of range in timestamp: {s:?}")
            }
        }
    }
}

impl std::error::Error for TimeParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_format_and_parse() {
        let ts = Timestamp::new(1, 2, 3, 456);
        assert_eq!(ts.format(), "01:02:03,456");
        assert_eq!(Timestamp::parse("01:02:03,456").unwrap(), ts);
    }

    #[test]
    fn accepts_period_as_decimal_separator() {
        assert_eq!(
            Timestamp::parse("00:00:01.500").unwrap(),
            Timestamp::new(0, 0, 1, 500)
        );
    }

    #[test]
    fn rejects_seconds_out_of_range() {
        assert!(matches!(
            Timestamp::parse("00:00:60,000"),
            Err(TimeParseError::OutOfRange(_))
        ));
    }

    #[test]
    fn rejects_malformed_input() {
        assert!(matches!(
            Timestamp::parse("not a timestamp"),
            Err(TimeParseError::InvalidFormat(_))
        ));
    }

    #[test]
    fn shift_clamps_at_zero_instead_of_going_negative() {
        let ts = Timestamp::from_millis(500);
        assert_eq!(ts.shifted(-1000).as_millis(), 0);
    }

    #[test]
    fn shift_moves_forward_normally() {
        let ts = Timestamp::from_millis(1_000);
        assert_eq!(ts.shifted(250).as_millis(), 1_250);
    }
}
