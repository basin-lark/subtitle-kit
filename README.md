# subtitle-kit

A small Rust library for parsing, editing, and writing subtitle files
(`.srt` and `.vtt`), built with zero dependencies.

I keep re-writing the same throwaway script every time a downloaded
subtitle file is out of sync with the video: parse the file, shift every
timestamp by some offset, write it back out. Every version of that script
lives in a different shell history and none of them handle CRLF line
endings or WebVTT-style `.` decimal separators. This is that script,
turned into a library with tests, so the next version of the problem
doesn't need a rewrite.

## Design

Every public function is pure: it takes plain data in (a `&str`, a
`&[Subtitle]`) and returns plain data out (a `Vec<Subtitle>`, a
`String`). There is no file I/O and no global state anywhere in the
crate. Reading the `.srt` file off disk and writing the result back is
left to the caller — a CLI, a web handler, a test, whatever — which also
means every function can be exercised with a string literal and an
assertion, no fixtures or temp files required.

## Usage

```rust
use subtitle_kit::{format_srt, parse_srt, shift_by};

let input = "1\n00:00:01,000 --> 00:00:04,000\nHello there\n\n\
             2\n00:00:05,500 --> 00:00:07,000\nSecond line\n";

let subtitles = parse_srt(input).expect("valid srt");

// The audio track starts 750ms later than the subtitles think it does.
let corrected = shift_by(&subtitles, 750);

println!("{}", format_srt(&corrected));
// 1
// 00:00:01,750 --> 00:00:04,750
// Hello there
//
// 2
// 00:00:06,250 --> 00:00:07,750
// Second line
```

WebVTT works the same way, through `parse_vtt` and `format_vtt`. A `Cue`
differs from a `Subtitle` in that its identifier is optional text rather
than a required number, and it may carry a cue settings string (e.g.
`align:start position:10%`):

```rust
use subtitle_kit::{format_vtt, parse_vtt};

let input = "WEBVTT\n\nintro\n00:00:01.000 --> 00:00:04.000\nHello there\n";
let cues = parse_vtt(input).expect("valid vtt");
assert_eq!(cues[0].identifier.as_deref(), Some("intro"));
println!("{}", format_vtt(&cues));
```

Timestamps can also be built and formatted directly:

```rust
use subtitle_kit::Timestamp;

let ts = Timestamp::new(0, 1, 30, 250);
assert_eq!(ts.format(), "00:01:30,250");
assert_eq!(Timestamp::parse("00:01:30,250").unwrap(), ts);
```

## Status

Early. SRT parsing, formatting, shifting, and renumbering, and WebVTT
parsing and formatting, are covered by tests in `src/srt.rs`,
`src/vtt.rs`, and `src/time.rs`. Still missing: overlap detection,
reading-speed validation, merge/split operations, and framerate
conversion.
