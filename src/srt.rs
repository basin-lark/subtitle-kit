//! Parsing and formatting for the SRT subtitle format.
//!
//! Every function here takes its input as a plain string and returns a
//! plain value; nothing touches the filesystem. Reading the file and
//! writing the result back out is left to the caller.

use crate::time::{TimeParseError, Timestamp};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subtitle {
    pub index: u32,
    pub start: Timestamp,
    pub end: Timestamp,
    pub lines: Vec<String>,
}

/// Parses the full contents of an `.srt` file into an ordered list of
/// subtitles. Blocks are separated by a blank line; each block is a
/// numeric index, a timing line, and one or more lines of text.
pub fn parse_srt(input: &str) -> Result<Vec<Subtitle>, ParseError> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .map(parse_block)
        .collect()
}

fn parse_block(block: &str) -> Result<Subtitle, ParseError> {
    let mut lines = block.lines();

    let index_line = lines.next().ok_or(ParseError::EmptyBlock)?;
    let index: u32 = index_line
        .trim()
        .parse()
        .map_err(|_| ParseError::InvalidIndex(index_line.to_string()))?;

    let timing_line = lines
        .next()
        .ok_or_else(|| ParseError::MissingTimingLine(index))?;
    let (start, end) = parse_timing_line(timing_line, index)?;

    let text_lines: Vec<String> = lines.map(str::to_string).collect();

    Ok(Subtitle {
        index,
        start,
        end,
        lines: text_lines,
    })
}

fn parse_timing_line(line: &str, index: u32) -> Result<(Timestamp, Timestamp), ParseError> {
    let mut sides = line.splitn(2, "-->");
    let start_str = sides
        .next()
        .map(str::trim)
        .ok_or_else(|| ParseError::InvalidTimingLine(index, line.to_string()))?;
    let end_str = sides
        .next()
        .ok_or_else(|| ParseError::InvalidTimingLine(index, line.to_string()))?
        .trim()
        // The end timestamp may be followed by rendering hints
        // (e.g. `X1:... Y1:...`); only the first token is the timestamp.
        .split_whitespace()
        .next()
        .ok_or_else(|| ParseError::InvalidTimingLine(index, line.to_string()))?;

    let start = Timestamp::parse(start_str).map_err(|e| ParseError::InvalidTimestamp(index, e))?;
    let end = Timestamp::parse(end_str).map_err(|e| ParseError::InvalidTimestamp(index, e))?;
    Ok((start, end))
}

/// Renders subtitles back into `.srt` text. Indices are written as given
/// on each `Subtitle`; call [`renumber`] first if they need to be sequential.
pub fn format_srt(subtitles: &[Subtitle]) -> String {
    let mut out = String::new();
    for (position, subtitle) in subtitles.iter().enumerate() {
        if position > 0 {
            out.push('\n');
        }
        out.push_str(&subtitle.index.to_string());
        out.push('\n');
        out.push_str(&subtitle.start.format());
        out.push_str(" --> ");
        out.push_str(&subtitle.end.format());
        out.push('\n');
        for line in &subtitle.lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Returns a copy of `subtitles` with every start and end time moved by
/// `offset_millis` (negative moves earlier). Times are clamped at zero
/// rather than going negative.
pub fn shift_by(subtitles: &[Subtitle], offset_millis: i64) -> Vec<Subtitle> {
    subtitles
        .iter()
        .map(|subtitle| Subtitle {
            index: subtitle.index,
            start: subtitle.start.shifted(offset_millis),
            end: subtitle.end.shifted(offset_millis),
            lines: subtitle.lines.clone(),
        })
        .collect()
}

/// Returns a copy of `subtitles` with indices replaced by 1, 2, 3, ...
/// in their current order. Useful after inserting, removing, or
/// reordering entries.
pub fn renumber(subtitles: &[Subtitle]) -> Vec<Subtitle> {
    subtitles
        .iter()
        .enumerate()
        .map(|(position, subtitle)| Subtitle {
            index: position as u32 + 1,
            start: subtitle.start,
            end: subtitle.end,
            lines: subtitle.lines.clone(),
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    EmptyBlock,
    InvalidIndex(String),
    MissingTimingLine(u32),
    InvalidTimingLine(u32, String),
    InvalidTimestamp(u32, TimeParseError),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::EmptyBlock => write!(f, "found an empty subtitle block"),
            ParseError::InvalidIndex(s) => write!(f, "invalid subtitle index: {s:?}"),
            ParseError::MissingTimingLine(index) => {
                write!(f, "subtitle {index} is missing its timing line")
            }
            ParseError::InvalidTimingLine(index, s) => {
                write!(f, "subtitle {index} has an invalid timing line: {s:?}")
            }
            ParseError::InvalidTimestamp(index, e) => {
                write!(f, "subtitle {index} has an invalid timestamp: {e}")
            }
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        "1\n00:00:01,000 --> 00:00:04,000\nHello there\n\n2\n00:00:05,500 --> 00:00:07,000\nSecond line\nand a third\n"
    }

    #[test]
    fn parses_a_well_formed_file() {
        let subtitles = parse_srt(sample()).unwrap();
        assert_eq!(subtitles.len(), 2);
        assert_eq!(subtitles[0].index, 1);
        assert_eq!(subtitles[0].lines, vec!["Hello there".to_string()]);
        assert_eq!(subtitles[1].lines.len(), 2);
    }

    #[test]
    fn format_and_parse_round_trip() {
        let subtitles = parse_srt(sample()).unwrap();
        let rendered = format_srt(&subtitles);
        let reparsed = parse_srt(&rendered).unwrap();
        assert_eq!(subtitles, reparsed);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let crlf = sample().replace('\n', "\r\n");
        let subtitles = parse_srt(&crlf).unwrap();
        assert_eq!(subtitles.len(), 2);
    }

    #[test]
    fn rejects_non_numeric_index() {
        let bad = "one\n00:00:01,000 --> 00:00:04,000\nText\n";
        assert!(matches!(parse_srt(bad), Err(ParseError::InvalidIndex(_))));
    }

    #[test]
    fn shift_by_moves_all_entries() {
        let subtitles = parse_srt(sample()).unwrap();
        let shifted = shift_by(&subtitles, 1_000);
        assert_eq!(shifted[0].start.as_millis(), 2_000);
        assert_eq!(shifted[0].end.as_millis(), 5_000);
    }

    #[test]
    fn shift_by_clamps_at_zero() {
        let subtitles = parse_srt(sample()).unwrap();
        let shifted = shift_by(&subtitles, -10_000);
        assert_eq!(shifted[0].start.as_millis(), 0);
    }

    #[test]
    fn renumber_produces_sequential_indices() {
        let mut subtitles = parse_srt(sample()).unwrap();
        subtitles[0].index = 41;
        subtitles[1].index = 99;
        let renumbered = renumber(&subtitles);
        assert_eq!(renumbered[0].index, 1);
        assert_eq!(renumbered[1].index, 2);
    }
}
