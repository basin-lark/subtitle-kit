//! Parsing and formatting for the WebVTT subtitle format.
//!
//! WebVTT looks a lot like SRT — blocks separated by a blank line, each
//! with a timing line and one or more lines of text — but with its own
//! header, optional (and non-numeric) cue identifiers, `.` instead of `,`
//! in timestamps, and an optional cue settings string after the timing
//! line. It gets its own module rather than reusing `srt`'s block parser
//! because those differences run through every step, not just the edges.

use crate::time::{TimeParseError, Timestamp};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub identifier: Option<String>,
    pub start: Timestamp,
    pub end: Timestamp,
    pub settings: Option<String>,
    pub lines: Vec<String>,
}

/// Parses the full contents of a `.vtt` file into an ordered list of
/// cues. The file must start with a `WEBVTT` header block. `NOTE`,
/// `STYLE`, and `REGION` blocks are recognized and skipped rather than
/// treated as cues.
pub fn parse_vtt(input: &str) -> Result<Vec<Cue>, VttParseError> {
    let without_bom = input.strip_prefix('\u{feff}').unwrap_or(input);
    let normalized = without_bom.replace("\r\n", "\n").replace('\r', "\n");

    let mut blocks = normalized
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty());

    let header = blocks.next().ok_or(VttParseError::MissingHeader)?;
    let header_first_line = header.lines().next().unwrap_or("");
    if !header_first_line.starts_with("WEBVTT") {
        return Err(VttParseError::MissingHeader);
    }

    let mut cues = Vec::new();
    let mut cue_number = 0usize;
    for block in blocks {
        if block.starts_with("NOTE") || block.starts_with("STYLE") || block.starts_with("REGION")
        {
            continue;
        }
        cue_number += 1;
        cues.push(parse_cue_block(block, cue_number)?);
    }
    Ok(cues)
}

fn parse_cue_block(block: &str, cue_number: usize) -> Result<Cue, VttParseError> {
    let mut lines = block.lines();
    let first = lines.next().ok_or(VttParseError::EmptyCueBlock(cue_number))?;

    let (identifier, timing_line) = if first.contains("-->") {
        (None, first)
    } else {
        let timing = lines
            .next()
            .ok_or(VttParseError::MissingTimingLine(cue_number))?;
        (Some(first.to_string()), timing)
    };

    let (start, end, settings) = parse_timing_line(timing_line, cue_number)?;
    let text_lines: Vec<String> = lines.map(str::to_string).collect();

    Ok(Cue {
        identifier,
        start,
        end,
        settings,
        lines: text_lines,
    })
}

fn parse_timing_line(
    line: &str,
    cue_number: usize,
) -> Result<(Timestamp, Timestamp, Option<String>), VttParseError> {
    let mut sides = line.splitn(2, "-->");
    let start_str = sides
        .next()
        .map(str::trim)
        .ok_or_else(|| VttParseError::InvalidTimingLine(cue_number, line.to_string()))?;
    let rest = sides
        .next()
        .ok_or_else(|| VttParseError::InvalidTimingLine(cue_number, line.to_string()))?
        .trim();

    let mut parts = rest.splitn(2, |c: char| c.is_whitespace());
    let end_str = parts
        .next()
        .ok_or_else(|| VttParseError::InvalidTimingLine(cue_number, line.to_string()))?;
    let settings = parts
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let start =
        Timestamp::parse_vtt(start_str).map_err(|e| VttParseError::InvalidTimestamp(cue_number, e))?;
    let end =
        Timestamp::parse_vtt(end_str).map_err(|e| VttParseError::InvalidTimestamp(cue_number, e))?;
    Ok((start, end, settings))
}

/// Renders cues back into `.vtt` text, including the `WEBVTT` header.
pub fn format_vtt(cues: &[Cue]) -> String {
    let mut out = String::from("WEBVTT\n");
    for cue in cues {
        out.push('\n');
        if let Some(identifier) = &cue.identifier {
            out.push_str(identifier);
            out.push('\n');
        }
        out.push_str(&cue.start.format_vtt());
        out.push_str(" --> ");
        out.push_str(&cue.end.format_vtt());
        if let Some(settings) = &cue.settings {
            out.push(' ');
            out.push_str(settings);
        }
        out.push('\n');
        for line in &cue.lines {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VttParseError {
    MissingHeader,
    EmptyCueBlock(usize),
    MissingTimingLine(usize),
    InvalidTimingLine(usize, String),
    InvalidTimestamp(usize, TimeParseError),
}

impl fmt::Display for VttParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VttParseError::MissingHeader => {
                write!(f, "file does not start with a WEBVTT header")
            }
            VttParseError::EmptyCueBlock(n) => write!(f, "cue {n} is empty"),
            VttParseError::MissingTimingLine(n) => {
                write!(f, "cue {n} is missing its timing line")
            }
            VttParseError::InvalidTimingLine(n, s) => {
                write!(f, "cue {n} has an invalid timing line: {s:?}")
            }
            VttParseError::InvalidTimestamp(n, e) => {
                write!(f, "cue {n} has an invalid timestamp: {e}")
            }
        }
    }
}

impl std::error::Error for VttParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> &'static str {
        "WEBVTT\n\n\
         intro\n00:00:01.000 --> 00:00:04.000\nHello there\n\n\
         00:00:05.500 --> 00:00:07.000 align:start\nSecond line\nand a third\n"
    }

    #[test]
    fn parses_a_well_formed_file() {
        let cues = parse_vtt(sample()).unwrap();
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].identifier.as_deref(), Some("intro"));
        assert_eq!(cues[0].lines, vec!["Hello there".to_string()]);
        assert_eq!(cues[1].identifier, None);
        assert_eq!(cues[1].settings.as_deref(), Some("align:start"));
        assert_eq!(cues[1].lines.len(), 2);
    }

    #[test]
    fn format_and_parse_round_trip() {
        let cues = parse_vtt(sample()).unwrap();
        let rendered = format_vtt(&cues);
        let reparsed = parse_vtt(&rendered).unwrap();
        assert_eq!(cues, reparsed);
    }

    #[test]
    fn handles_crlf_line_endings() {
        let crlf = sample().replace('\n', "\r\n");
        let cues = parse_vtt(&crlf).unwrap();
        assert_eq!(cues.len(), 2);
    }

    #[test]
    fn strips_leading_byte_order_mark() {
        let with_bom = format!("\u{feff}{}", sample());
        let cues = parse_vtt(&with_bom).unwrap();
        assert_eq!(cues.len(), 2);
    }

    #[test]
    fn skips_note_blocks() {
        let with_note = "WEBVTT\n\nNOTE this is a comment\nspanning lines\n\n\
                          00:00:01.000 --> 00:00:02.000\nText\n";
        let cues = parse_vtt(with_note).unwrap();
        assert_eq!(cues.len(), 1);
    }

    #[test]
    fn rejects_missing_header() {
        let bad = "1\n00:00:01.000 --> 00:00:04.000\nText\n";
        assert!(matches!(parse_vtt(bad), Err(VttParseError::MissingHeader)));
    }

    #[test]
    fn rejects_malformed_timing_line() {
        let bad = "WEBVTT\n\ncue1\nnot a timing line\nText\n";
        assert!(matches!(
            parse_vtt(bad),
            Err(VttParseError::InvalidTimingLine(1, _))
        ));
    }
}
