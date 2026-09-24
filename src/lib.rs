//! A library for working with subtitle files as plain data.
//!
//! Every public function takes its input by value or reference and
//! returns a new value; nothing here opens a file, reads stdin, or
//! prints anything. That makes each function testable with a plain
//! string in and a plain value out, and it means this crate can be
//! embedded in a CLI, a server, or a GUI without dragging along
//! assumptions about where the data comes from.

pub mod srt;
pub mod time;
pub mod vtt;

pub use srt::{format_srt, parse_srt, renumber, shift_by, ParseError, Subtitle};
pub use time::{TimeParseError, Timestamp};
pub use vtt::{format_vtt, parse_vtt, Cue, VttParseError};
