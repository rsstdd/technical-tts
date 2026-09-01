//! The written timeline, and the three text documents derived from it.
//!
//! Every boundary here is a frame count taken from what `assembly` actually
//! wrote, never a duration recomputed from a declared millisecond value.
//! ADR-0001 §§13.2 and 17.12 require caption boundaries to come from the
//! assembled sample boundaries, and a second derivation from `pause_after_ms`
//! would disagree with the write loop whenever rounding differs.
//!
//! `docs/architecture/WALKING-SKELETON.md` step 11 names this module and
//! [`captions`] as the path that derives the three documents, and names the
//! caption projection; this comment is the other end of that mirror.
//!
//! [`chapters`] carries those frames exactly. WebVTT cannot, so [`captions`]
//! writes a floored projection through [`timestamp`], whose comment is where
//! the authorizing record and its terms are stated.
//!
//! What these documents say for an unchanged timeline is part of what a
//! package *is*, so it is versioned rather than assumed:
//! [`TEXT_RENDERER_VERSION`] carries that identity into `manifest.json`, and
//! `manifest::validate_package` refuses to reuse a package another renderer
//! wrote.
//!
//! The documents this module renders are plain text rather than a format any
//! parser here owns, so their escaping is the whole of their safety: reviewed
//! `display_text` is authored content, and a caption or chapter record that let
//! it terminate its own cue or key would let a lesson rewrite the file around
//! it.

use std::fmt::Write as _;

use study_tts_core::{CANONICAL_SAMPLE_RATE, PlannedSegment};

/// Identity of the rules that turn one [`Timeline`] into the three documents.
///
/// Selected-package reuse compares the plan hash and the tool stack, and
/// neither reaches this module: FFmpeg never sees the transcript, the
/// captions, or the chapters, so a package written by a different renderer
/// over the same plan and the same tools is indistinguishable from a current
/// one without this. `manifest` records it and `manifest::validate_package`
/// requires it to match before a package may be reused.
///
/// Bump it whenever the bytes any of the three documents would hold for an
/// unchanged timeline change — a cue or chapter boundary, an escaping rule, a
/// line's layout. The concrete case is the rollback `ADR-0001-D010` describes:
/// replacing [`timestamp`] with an exact-frame representation changes every
/// caption in `transcript.vtt` while the plan and the tools stand still, and
/// without a bump here the old captions would be reused rather than rewritten.
///
/// `1.0-skeleton`, not `1.0`: E2-S3 adds loudness normalization and E2-S4 the
/// run report, and either may move these documents again.
pub(crate) const TEXT_RENDERER_VERSION: &str = "1.0-skeleton-text-renderer";

const MILLISECONDS_PER_SECOND: u64 = 1_000;

const SECONDS_PER_MINUTE: u64 = 60;
const MINUTES_PER_HOUR: u64 = 60;

/// One segment as it was written into the master, in exact frames.
///
/// Positions only. Segment identity stays where it already is — the plan and
/// the validated cache artifact — so this carries no second copy of a digest
/// that could disagree with the one the entry recorded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WrittenSegment {
    /// First frame of this segment's speech within the master.
    pub start_frame: u64,
    /// Frames of speech written, as counted by the write loop.
    pub audio_frames: u64,
    /// Frames of silence written after the speech.
    pub pause_frames: u64,
}

impl WrittenSegment {
    /// First frame after this segment's speech.
    ///
    /// Unchecked addition: both operands were summed into the checked
    /// `total_frames` that `assembly::assemble` compares against the plan, so a
    /// pair that could wrap here would have refused the whole master first.
    fn speech_end_frame(&self) -> u64 {
        self.start_frame + self.audio_frames
    }

    /// First frame after this segment's speech and its trailing silence.
    ///
    /// Unchecked on the same terms as [`WrittenSegment::speech_end_frame`].
    fn chapter_end_frame(&self) -> u64 {
        self.speech_end_frame() + self.pause_frames
    }
}

/// What one assembly wrote, in the order it wrote it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Timeline {
    /// Each segment's exact position in the master.
    pub segments: Vec<WrittenSegment>,
    /// Frames in the finished master, speech and silence together.
    pub total_frames: u64,
}

/// Renders the readable speaker-labelled transcript.
///
/// One line per segment, per ADR-0001 §13.5's "readable speaker-labelled
/// transcript". Line breaks inside a segment are collapsed so a reader can
/// count segments by counting lines.
pub(crate) fn transcript(plan_segments: &[PlannedSegment]) -> String {
    let mut document = String::new();
    for segment in plan_segments {
        // Cannot fail: `String`'s `fmt::Write` is infallible, and the arm
        // exists only because the trait is shared with fallible writers.
        let _ = writeln!(
            document,
            "{}: {}",
            single_line(&segment.speaker),
            single_line(&segment.display_text)
        );
    }
    document
}

/// Renders segment-level WebVTT captions from the written timeline.
///
/// One speech-only cue per segment: the trailing silence belongs to the
/// chapter, not to the caption, and a cue held open across it would caption
/// silence.
///
/// Cue times come from [`timestamp`], which cannot carry a frame boundary
/// exactly and states under what record it may floor one. The exact frames
/// stay in `manifest.json`.
pub(crate) fn captions(plan_segments: &[PlannedSegment], timeline: &Timeline) -> String {
    let mut document = String::from("WEBVTT\n");
    for (index, (segment, written)) in plan_segments.iter().zip(&timeline.segments).enumerate() {
        let _ = write!(
            document,
            "\n{}\n{} --> {}\n{}\n",
            index + 1,
            timestamp(written.start_frame),
            timestamp(written.speech_end_frame()),
            escape_cue_text(&segment.display_text)
        );
    }
    document
}

/// Renders FFMETADATA chapters covering the whole master without gaps.
///
/// Each chapter spans its segment's speech *and* its trailing silence, so the
/// chapters are contiguous and the last one ends at the final frame. A chapter
/// that stopped at the speech would leave every pause outside any chapter, and
/// a player seeking into one would land nowhere.
///
/// `TIMEBASE` is stated in frames so the boundaries need no conversion at all:
/// the numbers written here are the assembled sample boundaries themselves.
pub(crate) fn chapters(plan_segments: &[PlannedSegment], timeline: &Timeline) -> String {
    let mut document = String::from(";FFMETADATA1\n");
    for (segment, written) in plan_segments.iter().zip(&timeline.segments) {
        let _ = write!(
            document,
            "\n[CHAPTER]\nTIMEBASE=1/{}\nSTART={}\nEND={}\ntitle={}\n",
            CANONICAL_SAMPLE_RATE,
            written.start_frame,
            written.chapter_end_frame(),
            escape_metadata_value(&segment.display_text)
        );
    }
    document
}

/// Renders one frame position as a `HH:MM:SS.mmm` WebVTT timestamp.
///
/// The projection `ADR-0001-D010` requests, which names this function in
/// return: floored, never rounded, so a cue cannot begin after the sample it
/// describes. The largest error it can introduce is 23/24 ms.
///
/// ADR-0001 §13.5 calls these captions "sample-exact" and §17.12 requires the
/// boundaries to equal the assembled sample boundaries. This function does not
/// satisfy §17.12 and D010 is what says it need not: WebVTT timestamps are
/// milliseconds and a 24 kHz frame is 1/24 of one, so no implementation could.
/// Keeping the exact frame in `manifest.json` is the compensating control that
/// record rests on, not compliance. It carries no expiry, because the
/// constraint is the output format §13.5 names rather than a value awaiting
/// calibration.
///
/// Divided before it is scaled, so the multiplication only ever runs on a
/// remainder below one second. Multiplying the frame count by a thousand first
/// would overflow a `u64` for a long enough master, which is exactly the
/// arithmetic a timeline module must not get wrong.
fn timestamp(frames: u64) -> String {
    let rate = u64::from(CANONICAL_SAMPLE_RATE);
    let seconds = frames / rate;
    let milliseconds = (frames % rate) * MILLISECONDS_PER_SECOND / rate;
    format!(
        "{:02}:{:02}:{:02}.{milliseconds:03}",
        seconds / (SECONDS_PER_MINUTE * MINUTES_PER_HOUR),
        (seconds / SECONDS_PER_MINUTE) % MINUTES_PER_HOUR,
        seconds % SECONDS_PER_MINUTE,
    )
}

/// Collapses carriage returns and line feeds in authored text to one space.
///
/// Every document this module writes is line-oriented, so a newline inside a
/// value is the one character that can move text out of the record it belongs
/// to. Collapsing is preferred to escaping because all three documents stay
/// readable to a human that way.
fn single_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escapes reviewed text for a WebVTT cue payload.
///
/// `&`, `<`, and `>` are WebVTT's own markup characters; escaping `>` is also
/// what keeps a `-->` inside authored text from reading as a second cue timing
/// line. The blank line that would otherwise end the cue early is removed by
/// [`single_line`] before any of that.
fn escape_cue_text(text: &str) -> String {
    single_line(text)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Escapes reviewed text for an FFMETADATA value.
///
/// FFmpeg's metadata format treats `=`, `;`, `#`, and `\` as special anywhere
/// in a line and requires each to be backslash-escaped. The backslash is
/// replaced first, or the escapes added for the other three would themselves be
/// escaped a second time.
fn escape_metadata_value(text: &str) -> String {
    single_line(text)
        .replace('\\', "\\\\")
        .replace('=', "\\=")
        .replace(';', "\\;")
        .replace('#', "\\#")
}

#[cfg(test)]
mod tests {
    use super::*;
    use study_tts_core::{BASE_TAKE, CacheKey, DeliveryStyle};

    fn planned_segment(label: &str, display_text: &str) -> PlannedSegment {
        PlannedSegment {
            id: format!("segment-{label}"),
            speaker: "nadia".to_owned(),
            voice_profile: "nadia-v1".to_owned(),
            display_text: display_text.to_owned(),
            spoken_text: "Spoken text.".to_owned(),
            style: DeliveryStyle::Calm,
            pause_after_ms: 0,
            take: BASE_TAKE,
            cache_key: label
                .repeat(CacheKey::LENGTH)
                .parse()
                .expect("a repeated hexadecimal digit is a cache key"),
        }
    }

    #[test]
    fn t1_e1_transcript_preserves_non_line_break_whitespace() {
        let segments = [planned_segment(
            "0",
            "First  term\tkept.\r\nSecond line.\rThird line.",
        )];

        assert_eq!(
            transcript(&segments),
            "nadia: First  term\tkept. Second line. Third line.\n"
        );
    }

    #[test]
    fn t1_e1_chapters_cover_written_speech_and_silence() {
        let segments = [
            planned_segment("0", "First\n=part"),
            planned_segment("1", "Second; #part"),
        ];
        let timeline = Timeline {
            segments: vec![
                WrittenSegment {
                    start_frame: 0,
                    audio_frames: 32,
                    pause_frames: 8,
                },
                WrittenSegment {
                    start_frame: 40,
                    audio_frames: 60,
                    pause_frames: 4,
                },
            ],
            total_frames: 104,
        };

        assert_eq!(
            chapters(&segments, &timeline),
            concat!(
                ";FFMETADATA1\n",
                "\n[CHAPTER]\nTIMEBASE=1/24000\nSTART=0\nEND=40\ntitle=First \\=part\n",
                "\n[CHAPTER]\nTIMEBASE=1/24000\nSTART=40\nEND=104\n",
                "title=Second\\; \\#part\n",
            )
        );
    }

    /// Frame positions and the WebVTT timestamps they must render as.
    ///
    /// The last four rows are the ones that matter. 24,001 frames is one
    /// second and one frame — 1/24 of a millisecond past the second — so a
    /// sub-millisecond frame must not advance the millisecond. 24,023 is the
    /// row that separates this function from a rounding one: it is 1,000.958
    /// ms, which floors to `.000` and would round to `.001`. The last two
    /// cross a minute and an hour, which is where a formatter that forgot to
    /// carry goes wrong.
    #[test]
    fn t1_e1_frame_positions_render_as_floored_webvtt_timestamps() {
        const CASES: [(u64, &str); 7] = [
            (0, "00:00:00.000"),
            (2_400, "00:00:00.100"),
            (4_680, "00:00:00.195"),
            (24_001, "00:00:01.000"),
            (24_023, "00:00:01.000"),
            (24_000 * 61, "00:01:01.000"),
            (24_000 * 3_600, "01:00:00.000"),
        ];

        for (frames, expected) in CASES {
            assert_eq!(timestamp(frames), expected, "{frames} frames");
        }
    }

    /// The conversion must not overflow for any frame count a `u64` can hold.
    ///
    /// Multiplying the frame count by a thousand before dividing would wrap
    /// well below this, and a wrapped timestamp is a caption that silently
    /// points somewhere else in the audio.
    #[test]
    fn t1_e1_the_widest_frame_position_still_renders() {
        let rendered = timestamp(u64::MAX);

        // `u64::MAX` is 768,614,336,404,564 whole seconds plus 15,615 frames.
        // 15,615 frames is 650.625 ms, which projects to 650 under D010.
        assert_eq!(rendered, "213503982334:36:04.650");
    }

    /// Reviewed text cannot terminate a cue or introduce a second timing line.
    #[test]
    fn t1_e1_cue_text_cannot_escape_its_own_cue() {
        const CASES: [(&str, &str); 4] = [
            ("plain text", "plain text"),
            ("a & b", "a &amp; b"),
            ("<v Nadia>", "&lt;v Nadia&gt;"),
            (
                "first\n\nsecond 00:00:01.000 --> 00:00:02.000",
                "first second 00:00:01.000 --&gt; 00:00:02.000",
            ),
        ];

        for (authored, expected) in CASES {
            assert_eq!(escape_cue_text(authored), expected, "`{authored}`");
        }
    }

    /// Reviewed text cannot open a key, a section, or a comment in FFMETADATA.
    #[test]
    fn t1_e1_chapter_titles_cannot_escape_their_own_record() {
        const CASES: [(&str, &str); 5] = [
            ("plain title", "plain title"),
            ("a=b", "a\\=b"),
            ("a;b", "a\\;b"),
            ("a#b", "a\\#b"),
            // The backslash is escaped first, so its own escape is not escaped
            // a second time by the rules that follow it.
            ("a\\=b", "a\\\\\\=b"),
        ];

        for (authored, expected) in CASES {
            assert_eq!(escape_metadata_value(authored), expected, "`{authored}`");
        }
    }
}
