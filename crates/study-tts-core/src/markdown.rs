//! Structural Markdown parsing into the source blocks a lesson is compiled
//! from.
//!
//! ADR-0001 §9.1: "Use a structural Markdown parser. Do not strip markup with
//! regular expressions because fenced code, nested lists, inline code, links,
//! and punctuation carry meaning." That section enumerates the blocks a parser
//! emits, and [`SourceBlockKind`] is that list — nine kinds, closed, so a
//! structure this build does not know is refused rather than skipped.
//!
//! §9.2 bounds what may happen next: "Version 1 does not use an LLM." This
//! module retains source order and block references, keeps code apart from
//! prose, and reports what it cannot transform. It writes nothing, resolves
//! nothing, and invents no text.
//!
//! # What a block reference is
//!
//! Every block carries a `block-NNN` identifier assigned in document order.
//! That spelling is the one `fixtures/lessons/e0-s0-two-segment.json` and
//! ADR-0001 §8.3's audit example already use, so a compiled lesson's
//! `source_refs` reads the same as a hand-authored one's.
//!
//! The identifier is **positional**, which §9.2's "retain source order and
//! block references" asks for and which has a consequence worth stating:
//! inserting a block renumbers every block after it. A reference therefore
//! traces a claim to a block *in one compilation*, not across edits of the
//! source. `AuthoredLesson::source.content_hash` is what names the document a
//! given set of references belongs to, which is why a compiler that emits
//! references must emit that hash too.
//!
//! # What this module deliberately does not do
//!
//! No normalization of any kind — numbers, acronyms, identifiers, URLs, case
//! conventions. ADR-0001 §9.3 and `DELIVERY-PLAN.md` §E3-S2 own that. A
//! pronunciation directive is *extracted and recorded* here and applied
//! nowhere: applying one is the lexicon's job, which is E3-S2 task 6.

use std::{cmp::Reverse, collections::BTreeMap, ops::Range};

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::{
    AuthoredLesson, DeliveryStyle, LessonSegment, LessonSource, ReviewStatus, SegmentRole,
    SourceContentHash, SpeakerDeclaration,
};

/// How many source blocks one document may hold.
///
/// A refusal threshold rather than a contract, in the shape
/// [`crate::MAX_LESSON_JSON_BYTES`] already takes: a document that produces
/// more blocks than a reviewer could read has gone wrong somewhere, and
/// discovering that after allocating for all of them costs what the ceiling
/// exists to refuse.
pub const MAX_SOURCE_BLOCKS: usize = 10_000;

/// One structural block of a source document, in ADR-0001 §9.1's vocabulary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBlock {
    /// The `block-NNN` reference a lesson segment cites.
    pub id: String,
    /// What kind of block this is.
    pub kind: SourceBlockKind,
    /// The block's text with markup removed, or the code verbatim.
    pub text: String,
    /// Byte range in the source document, for locating a refusal without
    /// quoting what it refused.
    pub span: Range<usize>,
}

/// The block kinds ADR-0001 §9.1 says a structural parser emits.
///
/// Closed, and that is the point: §9.1 lists nine, and a tenth structure the
/// parser hands this build is [`CompileError::UnsupportedStructure`] rather
/// than a block quietly dropped. The enumeration is transcribed from §9.1's
/// bullet list — heading, paragraph, list item, fenced code, inline-code span,
/// quotation, link, table, pronunciation directive — and the document names
/// this type in return.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceBlockKind {
    /// A heading, with the level it was written at.
    ///
    /// §9.2 requires headings be converted "into section metadata rather than
    /// reading 'heading level two'". A heading is recorded here and emits no
    /// spoken segment, which is the narrowest reading that satisfies it: the
    /// lesson schema at `3.1` has no section field, so the alternative would
    /// move a published contract. `E3-S1-INTERFACE-CHANGE-001` records that
    /// reading and the field it would otherwise need.
    Heading {
        /// Heading depth, 1 through 6.
        level: u8,
    },
    /// Prose. The only kind that becomes a spoken segment.
    Paragraph,
    /// One item of an ordered or unordered list.
    ListItem,
    /// A fenced code block, with its info string when it declared one.
    ///
    /// Kept whole and apart from prose, because §9.2 requires preserving
    /// "code separately from conceptual prose" — a code block folded into a
    /// paragraph is read aloud as prose, which is the failure
    /// `t1_e3_fenced_code_is_never_merged_into_prose` exists to catch.
    FencedCode {
        /// The fence's info string, empty when none was written.
        info: String,
    },
    /// An inline-code span inside prose.
    InlineCode,
    /// A block quotation.
    Quotation,
    /// A link, carrying both halves §9.1 names.
    Link {
        /// Where the link points.
        destination: String,
    },
    /// A table, linearized by the policy `E3-S1-INTERFACE-CHANGE-001` states.
    Table,
    /// An explicit pronunciation directive the author wrote.
    ///
    /// §9.1 calls this a source block, so it is one. The syntax is a fenced
    /// block whose info string is `pronounce`, chosen because it is valid
    /// CommonMark — every other tool renders it as a code block rather than
    /// mangling it — and because the fence's info string is something the
    /// parser already hands us. `E3-S1-INTERFACE-CHANGE-001` records the
    /// choice and the two alternatives it was taken over.
    PronunciationDirective,
}

/// The info string that makes a fenced block a pronunciation directive.
///
/// Two-sided: `E3-S1-INTERFACE-CHANGE-001` §The pronunciation-directive
/// syntax names this constant, and this constant names that record.
pub const PRONUNCIATION_FENCE: &str = "pronounce";

/// Why a source document could not be parsed into blocks.
///
/// One variant per refusal, so a test can assert the exact failure rather than
/// `.is_err()` — the shape [`crate::LessonError`] already takes. **No variant
/// carries source text.** `docs/governance/RIGHTS-DATA-ARTIFACT-POLICY.md`
/// §Storage and access excludes source text from logs, and a refusal is
/// printed by default, so a diagnostic locates a problem by block reference
/// and byte offset and leaves the reading of it to the author's own editor.
/// That policy's §Enforcement table names this module in return, against
/// `t2_e3_arbitrary_unicode_markdown_never_panics`, which is the test that
/// asserts a refusal reports an offset rather than the text it refused.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CompileError {
    /// The document holds a structure this build cannot transform safely.
    #[error(
        "source document holds an unsupported {structure} at byte {offset}; ADR-0001 §9.2 emits \
         no text it cannot derive, so the author must remove or rewrite it rather than have it \
         silently dropped"
    )]
    UnsupportedStructure {
        /// What the parser reported, named rather than quoted.
        structure: &'static str,
        /// Where it starts in the source document.
        offset: usize,
    },

    /// A table cell holds nothing to linearize.
    ///
    /// This is the observable form of a ragged row, and the reason there is no
    /// `RaggedTableRow` variant: `pulldown-cmark` pads a short row to the
    /// header's width before this build sees it, so a row with one cell under
    /// two columns arrives as two cells, the second empty. Detecting the
    /// original raggedness would mean reading the source line, which §9.1
    /// forbids. Refusing the empty cell catches it at the point that matters —
    /// a cell with no value linearizes to "Column:" followed by nothing, and
    /// §9.2 emits no text it cannot derive.
    ///
    /// There is also no `TableWithoutHeader` variant, for a plainer reason: a
    /// pipe table's first row *is* its header in CommonMark, and a delimiter
    /// line with nothing above it is a paragraph rather than a table. The
    /// refusal was unreachable, which
    /// `t1_e3_table_cell_with_no_value_is_refused` established before this
    /// variant replaced both.
    #[error(
        "source document holds a table at byte {offset} whose column `{column}` has an empty \
         cell; a cell with no value cannot be read aloud, and supplying one would be inventing \
         text the author did not write"
    )]
    EmptyTableCell {
        /// Where the table starts.
        offset: usize,
        /// The column header whose cell is empty.
        column: String,
    },

    /// The document holds more blocks than this build will read.
    #[error(
        "source document holds more than {max} source blocks; a document this large is not \
         reviewable, and the author must split it"
    )]
    TooManySourceBlocks {
        /// The ceiling that refused it.
        max: usize,
    },

    /// The document opens with no heading to take a title from.
    #[error(
        "source document holds no heading, so there is no title to compile; ADR-0001 §9.2 \
         derives and never invents, and the lesson schema requires a title"
    )]
    NoTitleHeading,

    /// More than one speaker is declared and none was chosen for the segments.
    ///
    /// A defect in the request rather than an I/O one, and it is here for that
    /// reason: the first version of `study-tts lesson compile` reported it as
    /// `IoError::ReadFile` on the source path, which told an operator their
    /// document could not be read when it read fine. "Which declared speaker
    /// do compiled segments use" is a question about the request; the command
    /// line is only where it is asked.
    #[error(
        "{count} speakers are declared, so the one compiled segments use is ambiguous; name it \
         with `--voice <name>`"
    )]
    SpeakerNotChosen {
        /// How many speakers the request declared.
        count: usize,
    },

    /// One speaker name is bound to a voice profile more than once.
    ///
    /// The same invariant [`crate::LessonError::DuplicateSpeaker`] enforces on
    /// a lesson document, enforced one layer earlier because a compilation
    /// declares its speakers before any document exists to be refused.
    /// Keeping the last binding is what a `BTreeMap` collected from an
    /// iterator does by default, and it is what this refuses: which voice a
    /// speaker renders with would depend on argument order rather than on a
    /// decision somebody made. A voice profile reaches every synthesis key
    /// under ADR-0001 §12.5, so the collapse would be silent and durable.
    #[error(
        "speaker `{speaker}` is bound more than once; leave one binding, because a repeated one \
         lets the voice depend on argument order rather than on a decision"
    )]
    SpeakerDeclaredTwice {
        /// The speaker named more than once.
        speaker: String,
    },

    /// The source document is not UTF-8.
    ///
    /// A compile refusal rather than an I/O one, and the distinction is the
    /// operator's: the file read perfectly well, and reporting it as
    /// `IoError::ReadFile` sent them to check permissions and paths for a
    /// document whose bytes are the problem. Markdown is a format this project
    /// accepts from an author, so what it must be is this module's rule to
    /// state — the third refusal in this story moved off a borrowed
    /// `IoError`, after [`CompileError::SpeakerNotChosen`] and
    /// [`CompileError::SpeakerDeclaredTwice`].
    #[error(
        "source document is not valid UTF-8 at byte {offset}; the author must save it as UTF-8, \
         which is the only encoding a lesson's text can be compiled from"
    )]
    SourceNotUtf8 {
        /// Where the first invalid byte sequence begins.
        offset: usize,
    },

    /// The document holds nothing a voice could read.
    #[error(
        "source document holds no prose, list item, quotation, or table, so the compiled lesson \
         would have no segment to speak"
    )]
    NoProseToSpeak,
}

/// Parses a source document into the blocks ADR-0001 §9.1 enumerates.
///
/// Source order is preserved and every block is numbered in it, which is what
/// makes a reference stable across a recompilation of unchanged bytes.
///
/// # Errors
///
/// [`CompileError::UnsupportedStructure`] for a structure outside §9.1's nine
/// kinds — raw HTML, an image, a footnote definition, a task-list marker, a
/// thematic break — because §9.2 forbids emitting what cannot be derived and
/// dropping it silently is worse than refusing.
/// [`CompileError::EmptyTableCell`] where a table cannot be linearized without
/// supplying a value the author did not write.
/// [`CompileError::TooManySourceBlocks`] past [`MAX_SOURCE_BLOCKS`].
pub fn parse_source(document: &str) -> Result<Vec<SourceBlock>, CompileError> {
    // Only the extensions §9.1 names. Tables are in because §9.1 lists them;
    // footnotes, strikethrough, task lists, and the rest stay off, so the
    // parser reports them as the text they are rather than as structures this
    // build would have to decide about.
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);

    let mut blocks = Vec::new();
    let mut state = BlockBuilder::default();

    for (event, span) in Parser::new_ext(document, options).into_offset_iter() {
        state.apply(&event, span, &mut blocks)?;
        if blocks.len() > MAX_SOURCE_BLOCKS {
            return Err(CompileError::TooManySourceBlocks {
                max: MAX_SOURCE_BLOCKS,
            });
        }
    }

    // Numbered by where each block *starts*, not by when it closed. A block is
    // emitted at its end tag, so a container closes after the link or
    // inline-code span inside it — which numbered the child first and left the
    // sentence a listener hears as the higher reference. ADR-0001 §9.2 requires
    // compilation to "retain source order", and a reviewer tracing
    // `block-002` must land on the second thing in the document rather than on
    // the second thing to finish.
    //
    // The identifiers the fold assigned are overwritten here deliberately: the
    // builder cannot know a block's final position while events are still
    // arriving, and threading one through would mean buffering the whole
    // document before emitting anything.
    //
    // The end is the tiebreak, descending, and it is not optional: a paragraph
    // whose only content is a link shares that link's start byte, so sorting on
    // the start alone fell back to fold order and put the child first again —
    // the same defect, in the one case the first fix missed. A container always
    // spans further than what it holds, so the longer block is the outer one.
    blocks.sort_by_key(|block| (block.span.start, Reverse(block.span.end)));
    for (index, block) in blocks.iter_mut().enumerate() {
        block.id = block_reference(index);
    }
    Ok(blocks)
}

/// The reference for the block at `index`, counting from zero.
///
/// Zero-padded to three digits because that is the spelling the committed
/// fixtures carry; a fourth digit appears past `block-999` rather than
/// truncating, since a reference that collided would merge two blocks'
/// provenance.
fn block_reference(index: usize) -> String {
    format!("block-{:03}", index + 1)
}

/// Accumulates events into blocks, because one block is many events.
///
/// `pulldown-cmark` reports a block as a `Start`, its contents, and an `End`,
/// so a block is a fold rather than a single event. Holding the open block
/// here is what keeps a fenced code block's text out of the paragraph before
/// it.
#[derive(Default)]
struct BlockBuilder {
    /// The block being accumulated, if any.
    open: Option<OpenBlock>,
    /// Header cells of the current table, which name the cells beneath them.
    table_header: Vec<String>,
    /// Completed cells of the current table row.
    table_cells: Vec<String>,
    /// The cell currently being read, which is why a table's text events do
    /// not reach the open block: a cell is accumulated, then pushed whole.
    table_cell: Option<String>,
    /// The link being read: its destination, where it starts, and the label
    /// accumulated so far.
    ///
    /// ADR-0001 §9.1 names the block "link label and destination", so both
    /// halves are recorded. The label is accumulated here *and* appended to
    /// the prose holding it, because a link's label is part of the sentence it
    /// sits in — dropping it from the paragraph would silently delete words a
    /// listener is supposed to hear.
    link: Option<OpenLink>,
}

/// A link whose label is still arriving.
struct OpenLink {
    destination: String,
    label: String,
    span: Range<usize>,
}

/// A block whose events are still arriving.
struct OpenBlock {
    kind: SourceBlockKind,
    text: String,
    span: Range<usize>,
    /// The end tag that closes this block.
    ///
    /// A container emits its children's tags before its own: a block quote
    /// wraps a paragraph, and a loose list item does too. Without this, the
    /// inner paragraph's `End` closed the outer block and the quotation was
    /// reported as prose — which is what
    /// `t1_e3_each_supported_block_type_is_classified` caught. Closing only on
    /// the tag that opened keeps the outer kind, which is the one §9.1 names.
    closes_on: TagEnd,
}

impl BlockBuilder {
    /// Folds one event into the block being built, emitting finished blocks.
    fn apply(
        &mut self,
        event: &Event<'_>,
        span: Range<usize>,
        blocks: &mut Vec<SourceBlock>,
    ) -> Result<(), CompileError> {
        match event {
            Event::Start(tag) => self.start(tag, span),
            Event::End(tag) => self.end(*tag, blocks),
            Event::Text(text) | Event::Code(text) => {
                // Inside a table cell the text belongs to the cell, not to the
                // table block: the linearization pairs each cell with its
                // column header, which needs them kept apart until the row is
                // complete. Outside any block there is nothing to attach to,
                // and nothing is lost, because CommonMark wraps every run of
                // prose in a paragraph.
                if let Some(cell) = self.table_cell.as_mut() {
                    cell.push_str(text);
                } else if let Some(open) = self.open.as_mut() {
                    open.text.push_str(text);
                }
                // A label belongs to its link as well as to the prose above.
                if let Some(link) = self.link.as_mut() {
                    link.label.push_str(text);
                }
                if matches!(event, Event::Code(_)) {
                    // An inline-code span is its own block under §9.1 *and*
                    // stays part of the prose holding it, so it is recorded
                    // here rather than replacing the paragraph's text.
                    //
                    // The identifier is left empty and assigned by
                    // `parse_source`'s numbering pass: a block's reference
                    // depends on its position among every other block, which
                    // is not known until the fold has finished. Computing one
                    // here allocated a `String` per block that that pass then
                    // overwrote.
                    blocks.push(SourceBlock {
                        id: String::new(),
                        kind: SourceBlockKind::InlineCode,
                        text: (*text).to_string(),
                        span,
                    });
                }
                Ok(())
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(open) = self.open.as_mut() {
                    open.text.push(' ');
                }
                Ok(())
            }
            // Every remaining event is a structure outside §9.1's nine kinds.
            // Named rather than matched with a catch-all that would let the
            // next `pulldown-cmark` release add one silently.
            Event::Html(_) => Err(CompileError::UnsupportedStructure {
                structure: "HTML block",
                offset: span.start,
            }),
            Event::InlineHtml(_) => Err(CompileError::UnsupportedStructure {
                structure: "inline HTML",
                offset: span.start,
            }),
            Event::FootnoteReference(_) => Err(CompileError::UnsupportedStructure {
                structure: "footnote reference",
                offset: span.start,
            }),
            Event::TaskListMarker(_) => Err(CompileError::UnsupportedStructure {
                structure: "task-list marker",
                offset: span.start,
            }),
            Event::Rule => Err(CompileError::UnsupportedStructure {
                structure: "thematic break",
                offset: span.start,
            }),
            Event::InlineMath(_) | Event::DisplayMath(_) => {
                Err(CompileError::UnsupportedStructure {
                    structure: "math span",
                    offset: span.start,
                })
            }
        }
    }

    /// Opens a block, or records a table structure.
    ///
    /// Emits nothing: every block is pushed when its end tag arrives, which is
    /// what lets a container keep its own kind and a link carry its label.
    fn start(&mut self, tag: &Tag<'_>, span: Range<usize>) -> Result<(), CompileError> {
        match tag {
            Tag::Heading { level, .. } => {
                self.open(
                    SourceBlockKind::Heading {
                        level: *level as u8,
                    },
                    span,
                    TagEnd::Heading(*level),
                );
                Ok(())
            }
            Tag::Paragraph => {
                self.open(SourceBlockKind::Paragraph, span, TagEnd::Paragraph);
                Ok(())
            }
            Tag::Item => {
                self.open(SourceBlockKind::ListItem, span, TagEnd::Item);
                Ok(())
            }
            Tag::BlockQuote(_) => {
                self.open(SourceBlockKind::Quotation, span, TagEnd::BlockQuote(None));
                Ok(())
            }
            Tag::CodeBlock(kind) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let block = if info.trim() == PRONUNCIATION_FENCE {
                    SourceBlockKind::PronunciationDirective
                } else {
                    SourceBlockKind::FencedCode { info }
                };
                self.open(block, span, TagEnd::CodeBlock);
                Ok(())
            }
            Tag::Link { dest_url, .. } => {
                // Held open rather than pushed here: §9.1 names the block
                // "link label and destination", and the label is the text
                // between this tag and its end. The first version pushed the
                // block immediately with an empty `text`, which recorded half
                // of what the ADR says a link block records.
                self.link = Some(OpenLink {
                    destination: dest_url.to_string(),
                    label: String::new(),
                    span,
                });
                Ok(())
            }
            Tag::Table(_) => {
                self.table_header.clear();
                self.open(SourceBlockKind::Table, span, TagEnd::Table);
                Ok(())
            }
            Tag::TableHead | Tag::TableRow => {
                self.table_cells.clear();
                Ok(())
            }
            Tag::TableCell => {
                self.table_cell = Some(String::new());
                Ok(())
            }
            Tag::List(_) | Tag::Emphasis | Tag::Strong => Ok(()),
            Tag::Image { .. } => Err(CompileError::UnsupportedStructure {
                structure: "image",
                offset: span.start,
            }),
            Tag::FootnoteDefinition(_) => Err(CompileError::UnsupportedStructure {
                structure: "footnote definition",
                offset: span.start,
            }),
            Tag::HtmlBlock => Err(CompileError::UnsupportedStructure {
                structure: "HTML block",
                offset: span.start,
            }),
            Tag::Strikethrough
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition => Err(CompileError::UnsupportedStructure {
                structure: "unsupported inline or block decoration",
                offset: span.start,
            }),
        }
    }

    /// Closes a block, emitting it in source order.
    fn end(&mut self, tag: TagEnd, blocks: &mut Vec<SourceBlock>) -> Result<(), CompileError> {
        match tag {
            TagEnd::Heading(_)
            | TagEnd::Paragraph
            | TagEnd::Item
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock => {
                self.close(tag, blocks);
                Ok(())
            }
            TagEnd::TableCell => {
                if let Some(cell) = self.table_cell.take() {
                    self.table_cells.push(cell);
                }
                Ok(())
            }
            TagEnd::TableHead => {
                self.table_header = std::mem::take(&mut self.table_cells);
                let offset = self.open.as_ref().map_or(0, |open| open.span.start);
                if let Some(position) = self
                    .table_header
                    .iter()
                    .position(|column| column.trim().is_empty())
                {
                    // An unnamed column cannot name the cells beneath it, and
                    // numbering it would invent the name a listener hears.
                    return Err(CompileError::EmptyTableCell {
                        offset,
                        column: format!("column {}", position + 1),
                    });
                }
                Ok(())
            }
            TagEnd::TableRow => {
                let offset = self.open.as_ref().map_or(0, |open| open.span.start);
                // A padded cell is how a ragged row reaches this build; see
                // `CompileError::EmptyTableCell`.
                for (column, cell) in self.table_header.iter().zip(&self.table_cells) {
                    if cell.trim().is_empty() {
                        return Err(CompileError::EmptyTableCell {
                            offset,
                            column: column.clone(),
                        });
                    }
                }
                self.append_table_row();
                Ok(())
            }
            TagEnd::Table => {
                self.close(tag, blocks);
                Ok(())
            }
            TagEnd::Link => {
                if let Some(link) = self.link.take() {
                    // Identifier assigned by `parse_source`'s numbering pass;
                    // see the note at the inline-code site.
                    blocks.push(SourceBlock {
                        id: String::new(),
                        kind: SourceBlockKind::Link {
                            destination: link.destination,
                        },
                        text: link.label,
                        span: link.span,
                    });
                }
                Ok(())
            }
            TagEnd::List(_) | TagEnd::Emphasis | TagEnd::Strong => Ok(()),
            _ => Ok(()),
        }
    }

    /// Opens a block, unless a container is already open.
    ///
    /// The outermost block wins: a paragraph inside a quotation is the
    /// quotation's text, and a paragraph inside a list item is the item's.
    /// Its text still accumulates, because the open block is the one every
    /// text event appends to.
    fn open(&mut self, kind: SourceBlockKind, span: Range<usize>, closes_on: TagEnd) {
        if self.open.is_some() {
            return;
        }
        self.open = Some(OpenBlock {
            kind,
            text: String::new(),
            span,
            closes_on,
        });
    }

    /// Emits the open block when `tag` is the one that opened it.
    ///
    /// The emitted block carries no identifier; `parse_source` assigns every
    /// one once the document is complete, because a reference is a position
    /// among all blocks rather than a property of this one.
    fn close(&mut self, tag: TagEnd, blocks: &mut Vec<SourceBlock>) {
        if self.open.as_ref().is_none_or(|open| open.closes_on != tag) {
            return;
        }
        if let Some(open) = self.open.take() {
            blocks.push(SourceBlock {
                id: String::new(),
                kind: open.kind,
                text: open.text.trim().to_owned(),
                span: open.span,
            });
        }
    }

    /// Appends one linearized table row to the open table block.
    ///
    /// The policy `E3-S1-INTERFACE-CHANGE-001` §The table-linearization policy
    /// states: row-major, each cell named by its column header, one sentence
    /// per row. A header is required and a ragged row refuses, because either
    /// one would need a column name or a value the author did not write.
    fn append_table_row(&mut self) {
        let Some(open) = self.open.as_mut() else {
            return;
        };
        let row: Vec<String> = self
            .table_header
            .iter()
            .zip(&self.table_cells)
            .map(|(header, cell)| format!("{header}: {cell}"))
            .collect();
        if !open.text.is_empty() {
            open.text.push(' ');
        }
        open.text.push_str(&row.join(", "));
        open.text.push('.');
    }
}

/// What a compilation is given that the source document does not carry.
///
/// ADR-0001 §7.4 sets the precedence a configuration value resolves through:
/// command-line option, lesson profile, project configuration, user
/// configuration, compiled safe default. **Levels 1 and 5 are implemented and
/// 2 through 4 are not** — nothing in this workspace reads a configuration
/// file, and no `DELIVERY-PLAN.md` story owns the hierarchy, so E3-S1 takes
/// what it needs from the command line and documented defaults rather than
/// setting a file format it does not own. `E3-S1-INTERFACE-CHANGE-001` records
/// that, and the follow-up issue carries levels 2 through 4.
///
/// Every field here is a value the lesson schema requires and Markdown cannot
/// supply. `speakers` is the load-bearing one: it binds a voice profile, which
/// ADR-0001 §12.5 makes a synthesis-key input, so a compiler that defaulted it
/// would be choosing a voice.
#[derive(Debug)]
pub struct CompileRequest {
    /// Stable identity of the lesson, which also names its output directory.
    pub lesson_id: String,
    /// BCP 47 language tag; validated by the lesson gate, not here.
    pub language: String,
    /// Voice binding for every speaker the lesson gives lines to.
    pub speakers: BTreeMap<String, SpeakerDeclaration>,
    /// Which declared speaker compiled segments are assigned to.
    pub speaker: String,
    /// Role every compiled segment carries.
    ///
    /// One value for the whole document: the role vocabulary is seventeen
    /// closed values and no Markdown structure implies any of them, so a
    /// compiler that guessed per block would be inventing review metadata.
    /// An author assigns real roles by editing the compiled lesson, which is
    /// what `review_status` being `draft` leaves room for.
    pub role: SegmentRole,
    /// Delivery style every compiled segment carries, on the same terms.
    pub style: DeliveryStyle,
    /// Silence written after each compiled segment.
    pub pause_after_ms: u32,
}

/// Compiles a source document into a lesson an author then reviews.
///
/// Deterministic throughout, which ADR-0001 §9.2 requires: the same bytes and
/// the same request produce the same document, byte for byte, because every
/// identifier is positional and every map is ordered.
///
/// One segment per prose block — paragraph, list item, quotation, and a
/// linearized table. A heading is recorded as a source block and emits no
/// segment, and neither does a fenced code block: §9.2 asks that code be
/// preserved "separately from conceptual prose", and whether code is read
/// literally or explained is ADR-0001 §9.3's question, which E3-S2 owns.
///
/// `spoken_text` equals `display_text` at this stage. Nothing is normalized
/// here — §9.3 and E3-S2 own that — and emitting them equal rather than
/// omitting one keeps the document valid against §8.2 while leaving the
/// difference for the story that creates it.
///
/// # Errors
///
/// Whatever [`parse_source`] reports, plus [`CompileError::NoTitleHeading`]
/// when the document opens with no heading to take a title from, and
/// [`CompileError::NoProseToSpeak`] when it holds nothing a voice could read.
///
/// # Panics
///
/// Never for any document, and the argument is the one `plan.rs` makes for its
/// own panic rather than a disclosure. The single `expect` here converts
/// `blake3::Hash::to_hex` into a [`SourceContentHash`], whose `TryFrom<String>`
/// accepts exactly 64 lowercase hexadecimal characters —
/// [`crate::BLAKE3_HEX_PATTERN`] is the spelling it checks. `to_hex` returns
/// that for every possible input, including an empty one, because a BLAKE3
/// digest is 32 bytes whatever it hashed. For this to start panicking,
/// `SourceContentHash`'s own constructor would have to narrow, and the digest
/// newtype tests are what would fail first.
pub fn compile_lesson(
    document: &str,
    request: CompileRequest,
) -> Result<AuthoredLesson, CompileError> {
    let blocks = parse_source(document)?;

    // The title is derived rather than configured: the first heading is the
    // document's own, and §9.2 permits deriving while forbidding inventing.
    let title = blocks
        .iter()
        .find(|block| matches!(block.kind, SourceBlockKind::Heading { .. }))
        .map(|block| block.text.clone())
        .ok_or(CompileError::NoTitleHeading)?;

    let segments: Vec<LessonSegment> = blocks
        .iter()
        .filter(|block| block.kind.is_spoken())
        .enumerate()
        .map(|(index, block)| LessonSegment {
            id: format!("seg-{:04}", index + 1),
            // Cloned per segment because every segment names the same
            // speaker; the request's other fields move into the lesson below.
            speaker: request.speaker.clone(),
            role: request.role,
            source_refs: vec![block.id.clone()],
            display_text: block.text.clone(),
            spoken_text: block.text.clone(),
            style: request.style,
            pause_after_ms: request.pause_after_ms,
            // Never `approved`. A compiler cannot have performed the review
            // ADR-0001 §8.2 requires before a production build, and §9.2's own
            // phrase is "reviewable draft segments".
            review_status: ReviewStatus::Draft,
            editorial: false,
        })
        .collect();

    if segments.is_empty() {
        return Err(CompileError::NoProseToSpeak);
    }

    let mut lesson = AuthoredLesson::new(
        request.lesson_id,
        title,
        request.language,
        request.speakers,
        segments,
    );

    // The document the block references index. Without it `block-007` names a
    // position in a document nobody can identify, which is the other half of
    // "preserve stable source block references through compilation".
    lesson.source = Some(LessonSource {
        content_hash: SourceContentHash::try_from(
            blake3::hash(document.as_bytes()).to_hex().to_string(),
        )
        .expect("a BLAKE3 hex digest is a valid source content hash"),
        references: Vec::new(),
    });
    Ok(lesson)
}

impl SourceBlockKind {
    /// Whether a block of this kind becomes a segment a voice reads.
    ///
    /// Exhaustive, so a tenth kind is a compile error here rather than a block
    /// that silently stops being spoken — or silently starts.
    const fn is_spoken(&self) -> bool {
        match self {
            Self::Paragraph | Self::ListItem | Self::Quotation | Self::Table => true,
            // Recorded, not spoken. A heading is section metadata (§9.2), code
            // is preserved apart from prose and read by E3-S2's rules, a link
            // destination and an inline-code span belong to the prose holding
            // them, and a directive tells the normalizer how to say something
            // rather than being something to say.
            Self::Heading { .. }
            | Self::FencedCode { .. }
            | Self::InlineCode
            | Self::Link { .. }
            | Self::PronunciationDirective => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{
        CompileError, CompileRequest, MAX_SOURCE_BLOCKS, PRONUNCIATION_FENCE, SourceBlockKind,
        compile_lesson, parse_source,
    };
    use crate::{DeliveryStyle, ReviewStatus, SegmentRole, SpeakerDeclaration};

    /// Every kind ADR-0001 §9.1 enumerates is told apart from the others.
    ///
    /// The expectations are written out rather than derived from the
    /// classifier: `clean-code` §Tests requires a table a reviewer reads
    /// against the controlling document, because a `match` copied from the
    /// implementation agrees with any policy including a wrong one. Read the
    /// right column against §9.1's bullet list.
    ///
    /// The wrong implementation this rejects: a text-accumulating fold that
    /// reports a list item as a paragraph, because both carry prose.
    #[test]
    fn t1_e3_each_supported_block_type_is_classified() {
        let document = concat!(
            "# Title\n",
            "\n",
            "A paragraph.\n",
            "\n",
            "- An item\n",
            "\n",
            "> A quotation\n",
            "\n",
            "```rust\n",
            "let x = 1;\n",
            "```\n",
            "\n",
            "Prose with `inline_code` in it.\n",
            "\n",
            "A [label](https://example.invalid/page) here.\n",
            "\n",
            "| Source | Spoken |\n",
            "|---|---|\n",
            "| `a` | letter a |\n",
            "\n",
            "```pronounce\n",
            "charCodeAt = char code at\n",
            "```\n",
        );

        let blocks = parse_source(document).expect("every structure here is supported");
        let kinds: Vec<&SourceBlockKind> = blocks.iter().map(|block| &block.kind).collect();

        for expected in [
            SourceBlockKind::Heading { level: 1 },
            SourceBlockKind::Paragraph,
            SourceBlockKind::ListItem,
            SourceBlockKind::Quotation,
            SourceBlockKind::FencedCode {
                info: "rust".to_owned(),
            },
            SourceBlockKind::InlineCode,
            SourceBlockKind::Link {
                destination: "https://example.invalid/page".to_owned(),
            },
            SourceBlockKind::Table,
            SourceBlockKind::PronunciationDirective,
        ] {
            assert!(
                kinds.contains(&&expected),
                "§9.1 names {expected:?}; got {kinds:?}"
            );
        }
    }

    /// Code is its own block, never folded into the prose around it.
    ///
    /// ADR-0001 §9.2 requires preserving "code separately from conceptual
    /// prose". The wrong implementation this rejects is the one the event
    /// model invites: appending every text event to the open block regardless
    /// of the tag enclosing it, which reads a code block aloud as a sentence.
    #[test]
    fn t1_e3_fenced_code_is_never_merged_into_prose() {
        const CODE: &str = "let unmistakable_token = 1;";
        let document = format!("Before.\n\n```rust\n{CODE}\n```\n\nAfter.\n");

        let blocks = parse_source(&document).expect("the document is supported");

        let prose: Vec<&str> = blocks
            .iter()
            .filter(|block| block.kind == SourceBlockKind::Paragraph)
            .map(|block| block.text.as_str())
            .collect();
        assert_eq!(prose, vec!["Before.", "After."], "prose is two blocks");
        for block in &prose {
            assert!(
                !block.contains("unmistakable_token"),
                "code reached a prose block: {block}"
            );
        }
        let code: Vec<&str> = blocks
            .iter()
            .filter(|block| matches!(block.kind, SourceBlockKind::FencedCode { .. }))
            .map(|block| block.text.as_str())
            .collect();
        assert_eq!(code.len(), 1, "the code block is one block");
        assert!(
            code[0].contains("unmistakable_token"),
            "kept verbatim: {code:?}"
        );
    }

    /// An unsupported structure refuses; it is never silently skipped.
    ///
    /// The wrong implementation this rejects is a `_ => continue` arm, which
    /// makes every structure this build does not know vanish and reports a
    /// successful compilation of a document it only partly read.
    #[test]
    fn t1_e3_unsupported_structure_is_reported_not_dropped() {
        let document = "Before.\n\n<div>raw html</div>\n\nAfter.\n";

        let error = parse_source(document).expect_err("raw HTML cannot be transformed safely");

        assert!(
            matches!(
                error,
                CompileError::UnsupportedStructure {
                    structure: "HTML block",
                    ..
                }
            ),
            "raw HTML is refused by name: {error}"
        );
    }

    /// A pronunciation directive is recorded and never becomes prose.
    #[test]
    fn t1_e3_a_pronunciation_directive_is_extracted_not_spoken() {
        let document = format!("Prose.\n\n```{PRONUNCIATION_FENCE}\nkubectl = cube control\n```\n");

        let blocks = parse_source(&document).expect("a directive is supported");

        let directives: Vec<&str> = blocks
            .iter()
            .filter(|block| block.kind == SourceBlockKind::PronunciationDirective)
            .map(|block| block.text.as_str())
            .collect();
        assert_eq!(directives.len(), 1, "the directive is recorded: {blocks:?}");
        assert!(directives[0].contains("cube control"));
        for block in &blocks {
            assert!(
                block.kind != SourceBlockKind::Paragraph || !block.text.contains("cube control"),
                "a directive must not become prose: {block:?}"
            );
        }
    }

    /// References are `block-NNN` in document order, the spelling the
    /// committed fixtures carry.
    #[test]
    fn t1_e3_block_references_are_numbered_in_document_order() {
        let blocks = parse_source("One.\n\nTwo.\n\nThree.\n").expect("prose is supported");

        let ids: Vec<&str> = blocks.iter().map(|block| block.id.as_str()).collect();
        assert_eq!(ids, vec!["block-001", "block-002", "block-003"]);
    }

    /// No document panics, and nothing it emits is invalid UTF-8.
    ///
    /// ADR-0001 §17.3's first property: "arbitrary valid Unicode never panics
    /// and remains valid UTF-8". A seeded loop rather than `proptest`, which
    /// is not in this workspace — `rust-testing` §Not available here says a
    /// seeded loop covers a property at T2 and that adding a framework is a
    /// dependency decision rather than a test-writing one.
    ///
    /// The alphabet is hostile on purpose. A generator producing well-formed
    /// Markdown would prove the parser handles what an author writes, which is
    /// not the claim: the claim is that nothing in the input space reaches a
    /// panic. Combining marks, bidirectional overrides, zero-width joiners,
    /// lone carriage returns, NUL, unterminated fences, ragged tables, and
    /// deeply nested markers are each here because each is a shape a real
    /// editor can produce.
    ///
    /// The seed is fixed and written down, so a failure is reproducible rather
    /// than a story about a run nobody else has.
    #[test]
    fn t2_e3_arbitrary_unicode_markdown_never_panics() {
        const SEED: u64 = 0x5EED_E351;
        /// Well inside T2's two-minute budget; the parse is microseconds.
        const DOCUMENTS: usize = 4_000;
        /// Each piece is a shape that has broken a Markdown parser somewhere.
        ///
        /// The last four exist to reach the refusal path. Without them the
        /// generator produced 4 000 successful parses and no refusal, so the
        /// construction of a `CompileError` — and the rule that it must not
        /// quote its input — went untested while the suite reported success.
        const PIECES: [&str; 28] = [
            "#",
            "##",
            "######",
            "#\u{0}",
            ">",
            ">>>",
            "- ",
            "1. ",
            "`",
            "```",
            "```rust",
            "```pronounce",
            "|",
            "|---|",
            "[",
            "](",
            "\u{301}",
            "\u{200d}",
            "\u{202e}",
            "\r",
            "\n",
            "\t",
            "a",
            "\u{10FFFF}",
            "<div>",
            "![alt](x)",
            "\n***\n",
            "\n[^1]: note\n",
        ];

        // A linear congruential generator: the property needs reproducible
        // bytes rather than cryptographic ones, and this needs no dependency.
        let mut state = SEED;
        let mut next = move || {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            usize::try_from(state >> 16).unwrap_or(0)
        };

        let mut parsed = 0_usize;
        let mut refused = 0_usize;
        let mut blocks_seen = 0_usize;
        for document_index in 0..DOCUMENTS {
            let length = next() % 24 + 1;
            let mut document = String::new();
            for _ in 0..length {
                document.push_str(PIECES[next() % PIECES.len()]);
            }

            // The assertion is that this returns at all. A panic is the
            // failure; which way it returns is the parser's business.
            match parse_source(&document) {
                Ok(blocks) => {
                    parsed += 1;
                    blocks_seen += blocks.len();
                    for block in blocks {
                        assert!(
                            block.span.start <= document.len() && block.span.end <= document.len(),
                            "document {document_index} (seed {SEED:#x}) emitted a span outside \
                             the document: {:?} of {} bytes",
                            block.span,
                            document.len()
                        );
                    }
                }
                Err(error) => {
                    refused += 1;
                    // A refusal must not quote the document it refused, which
                    // is RIGHTS-DATA-ARTIFACT-POLICY.md §Storage and access.
                    let message = error.to_string();
                    assert!(
                        !message.contains(&document),
                        "document {document_index} (seed {SEED:#x}) was quoted in its own \
                         refusal: {message}"
                    );
                }
            }
        }

        // Without this the test passes for a generator whose every document
        // refuses at the first event, or parses to nothing — a property suite
        // that exercised no code and reported success. Both outcomes must
        // occur, and the parse must have produced blocks.
        assert!(
            parsed > 0 && refused > 0 && blocks_seen > 0,
            "the generator must reach both outcomes and emit blocks: {parsed} parsed, \
             {refused} refused, {blocks_seen} blocks (seed {SEED:#x})"
        );
    }

    /// A cell with no value is refused, never padded into a sentence.
    ///
    /// This is the test that found two of this module's error variants
    /// unreachable. `| only-one-cell |` under two columns does **not** arrive
    /// as a ragged row: `pulldown-cmark` pads it to the header's width, and
    /// the first implementation linearized the padding into "Spoken: ." — a
    /// value the author never wrote, read aloud as a column name followed by
    /// silence. A headerless pipe table is likewise not a table but a
    /// paragraph, so a `TableWithoutHeader` refusal could never fire either.
    ///
    /// Both are now one refusal at the point the parser lets this build
    /// observe: an empty cell.
    #[test]
    fn t1_e3_table_cell_with_no_value_is_refused() {
        let document = "| Source | Spoken |\n|---|---|\n| only-one-cell |\n";

        match parse_source(document) {
            Err(CompileError::EmptyTableCell { column, .. }) => {
                assert_eq!(column, "Spoken", "the refusal names the unfilled column");
            }
            other => panic!("a padded cell must be refused, got {other:?}"),
        }
    }

    /// The linearization pairs every cell with its own column header.
    ///
    /// Read against `E3-S1-INTERFACE-CHANGE-001` §The table-linearization
    /// policy rather than against the implementation: the expected string is
    /// written out, so a reviewer can disagree with the policy by reading this
    /// line.
    #[test]
    fn t1_e3_a_table_linearizes_by_column_header() {
        let document = concat!(
            "| Source | Context | Spoken result |\n",
            "|---|---|---|\n",
            "| charCodeAt(0) | API memorization | char code at, with index zero |\n",
            "| O(n log n) | complexity | order n log n |\n",
        );

        let blocks = parse_source(document).expect("a well-formed table is supported");
        let table = blocks
            .iter()
            .find(|block| block.kind == SourceBlockKind::Table)
            .expect("the table is one block");

        assert_eq!(
            table.text,
            "Source: charCodeAt(0), Context: API memorization, \
             Spoken result: char code at, with index zero. \
             Source: O(n log n), Context: complexity, Spoken result: order n log n.",
            "the policy is row-major, each cell named by its column"
        );
    }

    /// A request naming one speaker, for the compile tests.
    fn request() -> CompileRequest {
        CompileRequest {
            lesson_id: "e3-s1-compiled".to_owned(),
            language: "en".to_owned(),
            speakers: BTreeMap::from([(
                "narrator".to_owned(),
                SpeakerDeclaration {
                    voice_profile: "synthetic-test-voice-v1".to_owned(),
                },
            )]),
            speaker: "narrator".to_owned(),
            role: SegmentRole::Explanation,
            style: DeliveryStyle::CalmExplanatory,
            pause_after_ms: 75,
        }
    }

    /// No compiled segment claims a review a human did not perform.
    ///
    /// ADR-0001 §8.2: "no segment marked unreviewed can enter a production
    /// build." The wrong implementation this rejects is the one that copies
    /// `scaffold_lesson`, which writes `approved` as a starting value for a
    /// single segment a human then edits — defensible there, and not for
    /// twenty segments a compiler produced unread.
    #[test]
    fn t1_e3_a_compiled_segment_is_never_marked_approved() {
        let lesson =
            compile_lesson("# Title\n\nOne.\n\nTwo.\n", request()).expect("prose compiles");

        assert!(!lesson.segments.is_empty(), "there are segments to check");
        for segment in &lesson.segments {
            assert_eq!(
                segment.review_status,
                ReviewStatus::Draft,
                "segment {} claimed a review nobody performed",
                segment.id
            );
        }
    }

    /// Every segment's speaker is one the request declared.
    ///
    /// The wrong implementation this rejects: emitting a `speakers` map with
    /// an invented default binding, which chooses a voice — and a voice
    /// profile is a synthesis-key input under ADR-0001 §12.5.
    #[test]
    fn t1_e3_a_compiled_lesson_names_every_declared_speaker() {
        let lesson = compile_lesson("# Title\n\nOne.\n", request()).expect("prose compiles");

        for segment in &lesson.segments {
            assert!(
                lesson.speakers.contains_key(&segment.speaker),
                "segment {} names undeclared speaker `{}`",
                segment.id,
                segment.speaker
            );
        }
        assert_eq!(lesson.speakers.len(), 1, "no speaker was invented");
    }

    /// A heading becomes the title and is never read aloud.
    ///
    /// ADR-0001 §9.2: headings "convert into section metadata rather than
    /// reading 'heading level two'". The lesson schema at `3.1` carries no
    /// section field, so the narrowest reading that needs no contract move is
    /// this one — the heading is recorded as a source block, supplies the
    /// title, and emits no segment. `E3-S1-INTERFACE-CHANGE-001` records it.
    #[test]
    fn t1_e3_a_heading_emits_no_spoken_segment() {
        let lesson =
            compile_lesson("# The Heading\n\nProse.\n", request()).expect("prose compiles");

        assert_eq!(lesson.title, "The Heading", "the heading is the title");
        assert_eq!(lesson.segments.len(), 1, "only the prose is spoken");
        for segment in &lesson.segments {
            assert!(
                !segment.spoken_text.contains("The Heading"),
                "the heading reached spoken text: {}",
                segment.spoken_text
            );
        }
    }

    /// Two compilations of one document are byte-identical, and the document
    /// they index is named.
    ///
    /// ADR-0001 §9.2 requires retaining "source order and block references".
    /// The assertion is on the serialized bytes rather than on the
    /// identifiers, because an unordered collection anywhere in the compiler
    /// would move the output while leaving the ids equal — this workspace
    /// holds zero `HashMap` in crate sources for that reason, and the compiler
    /// has to keep the line. `canonical_bytes` is deliberately not used: it
    /// takes a hand-built `CanonicalValue` for an identity, and what matters
    /// here is the document a later build reads.
    ///
    /// The second assertion is the other half of task 2: `source.content_hash`
    /// names the document the references index, without which `block-001` is a
    /// position in a document nobody can identify.
    #[test]
    fn t2_e3_source_references_are_stable_across_recompilation() {
        const DOCUMENT: &str = concat!(
            "# Caches\n\nA cache stores reusable work.\n\n",
            "- It is keyed by its inputs.\n\n> Reuse is only safe when the key is complete.\n",
        );

        let first = compile_lesson(DOCUMENT, request()).expect("compiles");
        let second = compile_lesson(DOCUMENT, request()).expect("compiles again");

        assert_eq!(
            serde_json::to_vec(&first).expect("a compiled lesson serializes"),
            serde_json::to_vec(&second).expect("a compiled lesson serializes"),
            "recompiling unchanged bytes must produce the same document"
        );

        let references: Vec<&str> = first
            .segments
            .iter()
            .flat_map(|segment| segment.source_refs.iter().map(String::as_str))
            .collect();
        assert_eq!(
            references,
            vec!["block-002", "block-003", "block-004"],
            "every segment cites the block it came from, in document order"
        );

        let recorded = first.source.as_ref().expect("the source document is named");
        assert_eq!(
            recorded.content_hash.as_str(),
            blake3::hash(DOCUMENT.as_bytes()).to_hex().as_str(),
            "the references index the document this hash names"
        );
    }

    /// A document past the block ceiling is refused, not accumulated.
    ///
    /// The file ceiling does not cover this: `MAX_LESSON_JSON_BYTES` admits
    /// 16 MiB, which is roughly 5.6 million `"a\n\n"` blocks, so this is the
    /// bound that bites first and it needs a check of its own. It had none
    /// until the review that found it.
    #[test]
    fn t1_e3_a_document_past_the_block_ceiling_is_refused() {
        let document = "a\n\n".repeat(MAX_SOURCE_BLOCKS + 10);

        match parse_source(&document) {
            Err(CompileError::TooManySourceBlocks { max }) => {
                assert_eq!(max, MAX_SOURCE_BLOCKS, "the refusal names the ceiling");
            }
            other => panic!("a document past the ceiling must be refused, got {other:?}"),
        }
    }

    /// A link records both halves ADR-0001 §9.1 names.
    ///
    /// §9.1's block is "link label and destination". The first implementation
    /// pushed the block when the link opened, with `text: String::new()`, so
    /// the destination was recorded and the label never was — and the test
    /// that existed asserted only the destination, agreeing with the narrower
    /// behavior. Both halves are asserted here, and the label stays part of
    /// the prose that holds it, because dropping it from the paragraph would
    /// delete words a listener is supposed to hear.
    #[test]
    fn t1_e3_a_link_records_its_label_and_its_destination() {
        let blocks = parse_source("See [the cache docs](https://example.invalid/cache) first.\n")
            .expect("a link is supported");

        let link = blocks
            .iter()
            .find(|block| matches!(block.kind, SourceBlockKind::Link { .. }))
            .expect("the link is one block");
        let SourceBlockKind::Link { destination } = &link.kind else {
            unreachable!("the block was matched as a link above");
        };
        assert_eq!(destination, "https://example.invalid/cache");
        assert_eq!(link.text, "the cache docs", "the label is recorded");

        let prose = blocks
            .iter()
            .find(|block| block.kind == SourceBlockKind::Paragraph)
            .expect("the sentence is a paragraph");
        assert_eq!(
            prose.text, "See the cache docs first.",
            "the label stays in the sentence a listener hears"
        );
    }

    /// A container is numbered before the blocks it holds.
    ///
    /// ADR-0001 §9.2 requires compilation to "retain source order and block
    /// references", and this module's header claims the identifiers follow
    /// document order. Both were false until the numbering moved off close
    /// order: a block is emitted at its end tag, so a paragraph holding a link
    /// closed *after* the link and took the higher reference — the sentence a
    /// listener hears was `block-003` while its own link was `block-002`.
    ///
    /// The wrong implementation this rejects is that one, which every other
    /// test in this module passed because no fixture nested anything.
    #[test]
    fn t1_e3_a_container_is_numbered_before_the_blocks_it_holds() {
        let blocks = parse_source("See [the docs](https://example.invalid/d) first.\n")
            .expect("a paragraph holding a link is supported");

        let paragraph = blocks
            .iter()
            .find(|block| block.kind == SourceBlockKind::Paragraph)
            .expect("the sentence is a paragraph");
        let link = blocks
            .iter()
            .find(|block| matches!(block.kind, SourceBlockKind::Link { .. }))
            .expect("the link is its own block");

        assert_eq!(
            paragraph.id, "block-001",
            "the sentence starts the document"
        );
        assert_eq!(link.id, "block-002", "the link inside it follows");
        assert!(
            paragraph.span.start < link.span.start,
            "numbering follows where a block starts: {:?} then {:?}",
            paragraph.span,
            link.span
        );
    }

    /// An inline-code span is numbered after the prose that holds it.
    ///
    /// The same defect as the link above, by the other route: a code span is
    /// pushed from the text event rather than an end tag, so it landed before
    /// its own paragraph.
    #[test]
    fn t1_e3_inline_code_is_numbered_after_its_paragraph() {
        let blocks = parse_source("Prose with `inline_code` in it.\n")
            .expect("a paragraph holding inline code is supported");

        let ids: Vec<(&str, &SourceBlockKind)> = blocks
            .iter()
            .map(|block| (block.id.as_str(), &block.kind))
            .collect();
        assert_eq!(
            ids,
            vec![
                ("block-001", &SourceBlockKind::Paragraph),
                ("block-002", &SourceBlockKind::InlineCode),
            ],
            "the paragraph starts the document and the span inside it follows"
        );
    }

    /// A container sharing its child's start byte is still numbered first.
    ///
    /// `t1_e3_a_container_is_numbered_before_the_blocks_it_holds` cannot catch
    /// this: its fixture is `"See [the docs](…) first."`, where `See ` puts the
    /// link at byte 4 and the two starts differ. A paragraph whose *only*
    /// content is a link shares its start exactly, so ordering on the start
    /// alone fell back to the order the fold produced — which emits a child at
    /// its end tag, before its container closes.
    ///
    /// The wrong implementation this rejects is therefore the previous fix,
    /// not the original defect: a start-only sort passes the other test and
    /// fails this one.
    #[test]
    fn t1_e3_a_container_sharing_a_start_byte_is_still_numbered_first() {
        for document in [
            "[only a link](https://example.invalid/p)\n",
            "`only_code`\n",
        ] {
            let blocks = parse_source(document).expect("the document is supported");

            let first = blocks.first().expect("there is a block");
            assert_eq!(
                (first.id.as_str(), &first.kind),
                ("block-001", &SourceBlockKind::Paragraph),
                "the paragraph is outermost in {document:?}: {blocks:?}"
            );
            assert_eq!(
                blocks.get(1).map(|block| block.id.as_str()),
                Some("block-002"),
                "what it holds follows it in {document:?}"
            );
            assert_eq!(
                first.span.start, blocks[1].span.start,
                "this fixture is only a test of the tiebreak if the starts coincide"
            );
        }
    }
}
