//! What an assistive technology reads.
//!
//! A screen reader sees the view through `GtkAccessibleText`: one string,
//! addressed by character offset, plus a caret and a selection. The
//! string here is the transcript as a person would read it aloud — each
//! row on its own line, as timestamp, nick and message — which is not the
//! same thing as what is painted: a grouped row still names its speaker,
//! because "who said this" is the first thing a listener needs and the
//! visual cue that stands in for it (the row above) is not available to
//! them.
//!
//! The model is built the first time something asks for it, which on a
//! desktop without a screen reader is never, and from then on kept in step
//! with appends so that announcing a new message is cheap. Anything else
//! that changes the buffer — an insert above, a removal, a trim — drops it
//! to be rebuilt on the next query.
//!
//! Plain Rust over the layout engine's buffer, so all of it is tested
//! without a display; the GTK glue is at the bottom, behind the `v4_14`
//! feature, since the interface arrived in GTK 4.14.

// Without the GTK glue the model has no caller outside its tests; it is
// still built, so those tests run in every configuration.
#![cfg_attr(not(feature = "v4_14"), allow(dead_code))]

use rotulus_layout::{Block, Caret, ChatBuffer, LineSource, Linkifier, MessageId};

/// Where one of a row's texts sits in the row's line.
#[derive(Debug, Clone)]
struct Part {
    source: LineSource,
    /// Character offset of the part's first character, relative to the
    /// row.
    start: u32,
    /// Its length in characters, anything inserted included.
    len: u32,
    /// Text read out that the source doesn't have, as (the source byte
    /// it follows, characters inserted): a disguised link's address.
    extra: Vec<(usize, u32)>,
    /// Whether the characters are the source's own, so a caret maps
    /// through them one for one. A speaker's name standing in for the
    /// gutter isn't.
    verbatim: bool,
}

/// One row's place in the transcript.
#[derive(Debug, Clone)]
struct RowSpan {
    id: MessageId,
    /// Character offset of the row's first character.
    start: u32,
    /// Byte offset of the same, into `TextModel::text`.
    byte_start: usize,
    /// The row's texts in order. Those it doesn't have are absent.
    parts: Vec<Part>,
}

/// The transcript as one accessible string.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextModel {
    text: String,
    rows: Vec<RowSpan>,
    chars: u32,
}

/// What reading a row aloud depends on besides the row.
pub(crate) struct RowFormat {
    /// Formats a row's timestamp, or declines to.
    pub(crate) stamp: Box<dyn Fn(i64) -> Option<String>>,
    /// Decides which links are disguised, as the view does for a click.
    pub(crate) linkifier: Linkifier,
}

fn char_len(s: &str) -> u32 {
    s.chars().count() as u32
}

impl TextModel {
    pub(crate) fn build(buf: &ChatBuffer, fmt: &RowFormat) -> TextModel {
        let mut m = TextModel::default();
        for row in 0..buf.len() {
            if let Some(id) = buf.id_at(row) {
                m.insert_row(buf, m.rows.len(), id, fmt);
            }
        }
        m
    }

    /// Append the row `id` names. Returns the character range inserted,
    /// including the newline that separates it from its neighbour.
    pub(crate) fn push(&mut self, buf: &ChatBuffer, id: MessageId, fmt: &RowFormat) -> (u32, u32) {
        self.insert_row(buf, self.rows.len(), id, fmt)
    }

    /// The row `id` names, as read aloud, and where its parts start.
    fn row_text(msg: &rotulus_layout::Message, fmt: &RowFormat) -> (String, Vec<Part>) {
        let mut row = String::new();
        let mut parts = Vec::new();
        if let Some(ts) = (fmt.stamp)(msg.timestamp) {
            row.push_str(ts.trim_end());
            row.push(' ');
        }
        // The speaker's name rather than the gutter's decoration of it,
        // which a screen reader spells out: "less than al greater than".
        if let Some(sp) = msg.speaker.as_ref().filter(|s| !s.nick.is_empty()) {
            parts.push(Part {
                source: LineSource::Gutter,
                start: char_len(&row),
                len: char_len(&sp.nick),
                extra: Vec::new(),
                verbatim: false,
            });
            row.push_str(&sp.nick);
            row.push_str(": ");
        } else if let Some(g) = msg.gutter.as_ref().filter(|g| !g.text.is_empty()) {
            parts.push(Part {
                source: LineSource::Gutter,
                start: char_len(&row),
                len: char_len(&g.text),
                extra: Vec::new(),
                verbatim: true,
            });
            row.push_str(&g.text);
            row.push(' ');
        }
        for (bi, b) in msg.blocks.iter().enumerate() {
            if bi > 0 {
                row.push('\n');
            }
            let start = char_len(&row);
            let mut extra = Vec::new();
            match b {
                Block::Text(p) | Block::Quote { content: p, .. } => {
                    // A link whose label isn't its address reads as both,
                    // since the listener can't hover it to find out where
                    // it goes.
                    let mut links: Vec<_> = p.links.iter().collect();
                    links.sort_by_key(|l| l.range.start);
                    let mut at = 0;
                    for l in links {
                        let Some(shown) = p.text.get(l.range.clone()) else {
                            continue;
                        };
                        if l.range.start < at || fmt.linkifier.normalize(shown) == l.href {
                            continue;
                        }
                        row.push_str(&p.text[at..l.range.end]);
                        let note = format!(" ({})", l.href);
                        extra.push((l.range.end, char_len(&note)));
                        row.push_str(&note);
                        at = l.range.end;
                    }
                    row.push_str(&p.text[at..]);
                }
                Block::Code { text, .. } => row.push_str(text),
                Block::Image { alt, .. } => row.push_str(alt),
            }
            parts.push(Part {
                source: LineSource::Block(bi),
                start,
                len: char_len(&row) - start,
                extra,
                verbatim: true,
            });
        }
        (row, parts)
    }

    /// Move every row from `from` on by `chars` characters and `bytes`
    /// bytes (negative to pull them back).
    fn shift(&mut self, from: usize, chars: i64, bytes: i64) {
        for r in &mut self.rows[from..] {
            r.start = (i64::from(r.start) + chars) as u32;
            r.byte_start = (r.byte_start as i64 + bytes) as usize;
        }
    }

    /// Insert the row `id` names at model position `index` — the same
    /// position it has in the buffer. Returns the character range
    /// inserted, separator included.
    pub(crate) fn insert_row(
        &mut self,
        buf: &ChatBuffer,
        index: usize,
        id: MessageId,
        fmt: &RowFormat,
    ) -> (u32, u32) {
        let index = index.min(self.rows.len());
        let Some(msg) = buf.message(id) else {
            return (self.chars, self.chars);
        };
        let (row, parts) = Self::row_text(msg, fmt);
        let (at_char, at_byte, text, start, byte_start) = if self.rows.is_empty() {
            (0, 0, row, 0, 0)
        } else if index == self.rows.len() {
            // At the end: a separator, then the row.
            let (c, b) = (self.chars, self.text.len());
            (c, b, format!("\n{row}"), c + 1, b + 1)
        } else {
            // Before a row: the row, then a separator.
            let (c, b) = (self.rows[index].start, self.rows[index].byte_start);
            (c, b, format!("{row}\n"), c, b)
        };
        let added = text.chars().count() as u32;
        self.text.insert_str(at_byte, &text);
        self.chars += added;
        self.shift(index, i64::from(added), text.len() as i64);
        self.rows.insert(
            index,
            RowSpan {
                id,
                start,
                byte_start,
                parts,
            },
        );
        (at_char, at_char + added)
    }

    /// Remove the row at model position `index`. Returns the character
    /// range removed, separator included.
    pub(crate) fn remove_row(&mut self, index: usize) -> Option<(u32, u32)> {
        if index >= self.rows.len() {
            return None;
        }
        let r = self.rows.remove(index);
        let (from_char, from_byte, to_char, to_byte) = if let Some(next) = self.rows.get(index) {
            // A row with a successor takes its trailing separator.
            (r.start, r.byte_start, next.start, next.byte_start)
        } else if index > 0 {
            // The last of several takes the separator before it.
            (r.start - 1, r.byte_start - 1, self.chars, self.text.len())
        } else {
            (0, 0, self.chars, self.text.len())
        };
        self.text.replace_range(from_byte..to_byte, "");
        let (chars, bytes) = (to_char - from_char, to_byte - from_byte);
        self.chars -= chars;
        self.shift(index, -i64::from(chars), -(bytes as i64));
        Some((from_char, to_char))
    }

    /// Model position of the row `id` names.
    pub(crate) fn index_of(&self, id: MessageId) -> Option<usize> {
        self.rows.iter().position(|r| r.id == id)
    }

    /// Positions of rows the buffer no longer has, looking at no more than
    /// `limit` of them from the front — where a trim takes rows from.
    pub(crate) fn gone(&self, buf: &ChatBuffer, limit: usize) -> Vec<usize> {
        let mut out = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            if out.len() == limit {
                break;
            }
            if buf.row_of(r.id).is_none() {
                out.push(i);
            }
        }
        out
    }

    pub(crate) fn char_len(&self) -> u32 {
        self.chars
    }

    #[cfg(test)]
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    /// Byte offset of a character offset, clamped to the end.
    fn byte_of(&self, offset: u32) -> usize {
        if offset >= self.chars {
            return self.text.len();
        }
        // Rows are sorted by start; find the one holding the offset and
        // walk characters from there rather than from the beginning.
        let i = self
            .rows
            .partition_point(|r| r.start <= offset)
            .saturating_sub(1);
        let (mut chars, bytes) = match self.rows.get(i) {
            Some(r) => (r.start, r.byte_start),
            None => (0, 0),
        };
        for (b, _) in self.text[bytes..].char_indices() {
            if chars == offset {
                return bytes + b;
            }
            chars += 1;
        }
        self.text.len()
    }

    /// The characters `start..end`, clamped.
    pub(crate) fn contents(&self, start: u32, end: u32) -> &str {
        let (a, b) = (self.byte_of(start.min(end)), self.byte_of(end.max(start)));
        &self.text[a..b]
    }

    /// The unit of `granularity` holding `offset`, as (start, end, text).
    ///
    /// A unit runs from its start to the next one's, taking the
    /// whitespace or newline after it along, as GTK's own text widgets
    /// report them: a reader that steps by asking again at `end` then
    /// always moves on.
    pub(crate) fn contents_at(&self, offset: u32, granularity: Granularity) -> (u32, u32, &str) {
        let offset = offset.min(self.chars);
        let (start, end) = match granularity {
            Granularity::Character => (offset, (offset + 1).min(self.chars)),
            Granularity::Word => {
                self.unit(offset, |prev, c| prev.is_whitespace() && !c.is_whitespace())
            }
            // Lines and sentences are rows' lines here: chat is not
            // written in paragraphs of sentences, and a message's own
            // punctuation is too unreliable to split on.
            Granularity::Line | Granularity::Sentence => self.unit(offset, |prev, _| prev == '\n'),
            Granularity::Paragraph => {
                let i = self
                    .rows
                    .partition_point(|r| r.start <= offset)
                    .saturating_sub(1);
                match self.rows.get(i) {
                    Some(r) => (
                        r.start,
                        self.rows.get(i + 1).map_or(self.chars, |n| n.start),
                    ),
                    None => (0, 0),
                }
            }
        };
        (start, end, self.contents(start, end))
    }

    /// The unit around `offset`: from the last boundary at or before it
    /// (or the start) to the first after it (or the end), where a
    /// boundary falls between two characters that `starts` accepts.
    fn unit(&self, offset: u32, starts: impl Fn(char, char) -> bool) -> (u32, u32) {
        let (before, after) = self.text.split_at(self.byte_of(offset));
        let mut start = 0;
        let (mut next, mut at) = (after.chars().next(), offset);
        for prev in before.chars().rev() {
            if next.is_some_and(|c| starts(prev, c)) {
                start = at;
                break;
            }
            (next, at) = (Some(prev), at - 1);
        }
        let mut end = self.chars;
        let mut rest = after.chars();
        if let Some(mut prev) = rest.next() {
            for (i, c) in rest.enumerate() {
                if starts(prev, c) {
                    end = offset + 1 + i as u32;
                    break;
                }
                prev = c;
            }
        }
        (start, end)
    }

    /// The character offset of a caret, if its row is in the model.
    pub(crate) fn offset_of(&self, caret: &Caret, buf: &ChatBuffer) -> Option<u32> {
        let row = self.rows.iter().find(|r| r.id == caret.message)?;
        let part = row.parts.iter().find(|p| p.source == caret.source)?;
        let within = if part.verbatim {
            let text = buf.source_text(buf.row_of(caret.message)?, caret.source)?;
            let upto = text.get(..caret.offset.min(text.len()))?;
            let inserted: u32 = part
                .extra
                .iter()
                .filter(|(after, _)| *after < caret.offset)
                .map(|(_, n)| n)
                .sum();
            char_len(upto) + inserted
        } else {
            0
        };
        Some(row.start + part.start + within.min(part.len))
    }

    /// The caret at a character offset: `offset_of` run backwards. A
    /// character no source has — a timestamp, a separator, a link's
    /// address — gives the nearest caret before it.
    #[cfg_attr(not(feature = "v4_16"), allow(dead_code))]
    pub(crate) fn caret_at(&self, offset: u32, buf: &ChatBuffer) -> Option<Caret> {
        let i = self
            .rows
            .partition_point(|r| r.start <= offset)
            .checked_sub(1)?;
        let row = &self.rows[i];
        let rel = offset - row.start;
        let pi = row
            .parts
            .partition_point(|p| p.start <= rel)
            .saturating_sub(1);
        let part = row.parts.get(pi)?;
        let within = rel.saturating_sub(part.start).min(part.len);
        let mut byte = 0;
        if part.verbatim {
            let text = buf.source_text(buf.row_of(row.id)?, part.source)?;
            let mut extra = part.extra.iter().peekable();
            let mut inserted = 0;
            let ends = text.char_indices().map(|(b, _)| b).chain([text.len()]);
            for (n, b) in ends.enumerate() {
                while let Some((_, k)) = extra.next_if(|(after, _)| *after < b) {
                    inserted += k;
                }
                if n as u32 + inserted > within {
                    break;
                }
                byte = b;
            }
        }
        Some(Caret {
            message: row.id,
            source: part.source,
            offset: byte,
        })
    }

    /// Where the newest row starts: the caret's place when nothing else
    /// has put it anywhere, since the end of a log is what is being read.
    pub(crate) fn last_row_start(&self) -> u32 {
        self.rows.last().map_or(0, |r| r.start)
    }
}

/// `GtkAccessibleTextGranularity`, without depending on GTK 4.14 to name
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Granularity {
    Character,
    Word,
    Sentence,
    Line,
    Paragraph,
}

#[cfg(feature = "v4_14")]
mod glue {
    use super::*;
    use crate::view::RotulusView;
    use gtk4::prelude::*;
    use gtk4::subclass::prelude::*;

    impl From<gtk4::AccessibleTextGranularity> for Granularity {
        fn from(g: gtk4::AccessibleTextGranularity) -> Self {
            match g {
                gtk4::AccessibleTextGranularity::Character => Granularity::Character,
                gtk4::AccessibleTextGranularity::Word => Granularity::Word,
                gtk4::AccessibleTextGranularity::Sentence => Granularity::Sentence,
                gtk4::AccessibleTextGranularity::Paragraph => Granularity::Paragraph,
                _ => Granularity::Line,
            }
        }
    }

    impl AccessibleTextImpl for crate::view::imp::RotulusView {
        fn contents(&self, start: u32, end: u32) -> Option<glib::Bytes> {
            let obj = self.obj();
            obj.with_a11y(|m, _| Some(glib::Bytes::from(m.contents(start, end).as_bytes())))
        }

        fn contents_at(
            &self,
            offset: u32,
            granularity: gtk4::AccessibleTextGranularity,
        ) -> Option<(u32, u32, glib::Bytes)> {
            let obj = self.obj();
            obj.with_a11y(|m, _| {
                let (a, b, text) = m.contents_at(offset, granularity.into());
                Some((a, b, glib::Bytes::from(text.as_bytes())))
            })
        }

        fn caret_position(&self) -> u32 {
            let obj = self.obj();
            let at = obj.a11y_range().map(|s| s.focus);
            obj.with_a11y(|m, buf| {
                Some(match at {
                    Some(c) => m.offset_of(&c, buf).unwrap_or(m.last_row_start()),
                    None => m.last_row_start(),
                })
            })
            .unwrap_or(0)
        }

        fn selection(&self) -> Vec<gtk4::AccessibleTextRange> {
            let obj = self.obj();
            let Some(sel) = obj.a11y_range().filter(|s| !s.is_empty()) else {
                return Vec::new();
            };
            obj.with_a11y(|m, buf| {
                let a = m.offset_of(&sel.anchor, buf)?;
                let b = m.offset_of(&sel.focus, buf)?;
                let (lo, hi) = (a.min(b), a.max(b));
                Some(vec![gtk4::AccessibleTextRange::new(
                    lo as usize,
                    (hi - lo) as usize,
                )])
            })
            .unwrap_or_default()
        }

        fn attributes(
            &self,
            _offset: u32,
        ) -> Vec<(gtk4::AccessibleTextRange, glib::GString, glib::GString)> {
            Vec::new()
        }

        fn default_attributes(&self) -> Vec<(glib::GString, glib::GString)> {
            Vec::new()
        }

        #[cfg(feature = "v4_16")]
        fn extents(&self, start: u32, end: u32) -> Option<gtk4::graphene::Rect> {
            let obj = self.obj();
            let (a, b) = obj.with_a11y(|m, buf| {
                let last = end.saturating_sub(1).max(start);
                Some((m.caret_at(start, buf)?, m.caret_at(last, buf)?))
            })?;
            Some(obj.caret_rect(&a)?.union(&obj.caret_rect(&b)?))
        }

        #[cfg(feature = "v4_16")]
        fn offset(&self, point: &gtk4::graphene::Point) -> Option<u32> {
            let obj = self.obj();
            let caret = obj.char_at(f64::from(point.x()), f64::from(point.y()))?;
            obj.with_a11y(|m, buf| m.offset_of(&caret, buf))
        }
    }

    impl RotulusView {
        /// Run `f` over the accessible text, building it first if nothing
        /// has asked before.
        pub(crate) fn with_a11y<T>(
            &self,
            f: impl FnOnce(&TextModel, &ChatBuffer) -> Option<T>,
        ) -> Option<T> {
            let imp = self.imp_ref();
            let buf = imp.buffer.borrow();
            let mut slot = imp.a11y.borrow_mut();
            if slot.is_none() {
                *slot = Some(TextModel::build(&buf, &self.a11y_format()));
            }
            f(slot.as_ref()?, &buf)
        }

        /// Run `f` over the accessible text if something has asked for
        /// it, and report the ranges it changed. Nobody asking means
        /// nobody to tell.
        fn a11y_edit(
            &self,
            f: impl FnOnce(&mut TextModel, &ChatBuffer, &RowFormat) -> Vec<(bool, u32, u32)>,
        ) {
            let imp = self.imp_ref();
            let changes = {
                let mut slot = imp.a11y.borrow_mut();
                let Some(m) = slot.as_mut() else {
                    return;
                };
                f(m, &imp.buffer.borrow(), &self.a11y_format())
            };
            for (inserted, a, b) in changes {
                let change = if inserted {
                    gtk4::AccessibleTextContentChange::Insert
                } else {
                    gtk4::AccessibleTextContentChange::Remove
                };
                self.update_contents(change, a, b);
            }
        }

        /// A row was appended, and `trimmed` of the oldest went to make
        /// room for it.
        pub(crate) fn a11y_appended(&self, id: MessageId, trimmed: usize) {
            self.a11y_edit(|m, buf, fmt| {
                let mut changes = Vec::new();
                // Highest first, so each position is still right when it
                // is removed.
                for i in m.gone(buf, trimmed).into_iter().rev() {
                    if let Some((a, b)) = m.remove_row(i) {
                        changes.push((false, a, b));
                    }
                }
                let (a, b) = m.push(buf, id, fmt);
                changes.push((true, a, b));
                changes
            });
        }

        /// A row went in above the end — a page of history.
        pub(crate) fn a11y_inserted(&self, id: MessageId) {
            self.a11y_edit(|m, buf, fmt| {
                let Some(at) = buf.row_of(id) else {
                    return Vec::new();
                };
                let (a, b) = m.insert_row(buf, at, id, fmt);
                vec![(true, a, b)]
            });
        }

        /// A row was removed from the buffer.
        pub(crate) fn a11y_removed(&self, id: MessageId) {
            self.a11y_edit(|m, _, _| {
                let Some(i) = m.index_of(id) else {
                    return Vec::new();
                };
                m.remove_row(i)
                    .map(|(a, b)| vec![(false, a, b)])
                    .unwrap_or_default()
            });
        }

        /// A row's content was replaced in place.
        pub(crate) fn a11y_replaced(&self, id: MessageId) {
            self.a11y_edit(|m, buf, fmt| {
                let Some(i) = m.index_of(id) else {
                    return Vec::new();
                };
                let mut changes = Vec::new();
                if let Some((a, b)) = m.remove_row(i) {
                    changes.push((false, a, b));
                }
                let (a, b) = m.insert_row(buf, i, id, fmt);
                changes.push((true, a, b));
                changes
            });
        }

        pub(crate) fn a11y_reset(&self) {
            let old = self.imp_ref().a11y.borrow_mut().take();
            let Some(old) = old else {
                return;
            };
            // Everything that was there went; everything there now arrived.
            // Coarse, and only for what changes every row at once — a
            // clear, a new timestamp format — since a listener told the
            // truth coarsely is better off than one told something finer
            // that is wrong.
            self.update_contents(gtk4::AccessibleTextContentChange::Remove, 0, old.char_len());
            let len = self.with_a11y(|m, _| Some(m.char_len())).unwrap_or(0);
            if len > 0 {
                self.update_contents(gtk4::AccessibleTextContentChange::Insert, 0, len);
            }
        }

        pub(crate) fn a11y_selection_changed(&self) {
            if self.imp_ref().a11y.borrow().is_some() {
                self.update_caret_position();
                self.update_selection_bound();
            }
        }

        /// What the accessible caret and selection report: the selection
        /// when there is one, else what last moved the caret without
        /// selecting, such as the current search match.
        fn a11y_range(&self) -> Option<rotulus_layout::Selection> {
            let imp = self.imp_ref();
            let sel = *imp.selection.borrow();
            sel.or(imp.a11y_caret.get())
        }

        /// Put the accessible caret on `range` without selecting it.
        pub(crate) fn a11y_move_caret(&self, range: Option<rotulus_layout::Selection>) {
            self.imp_ref().a11y_caret.set(range);
            self.a11y_selection_changed();
        }

        pub(crate) fn a11y_announce(&self, message: &str) {
            self.announce(message, gtk4::AccessibleAnnouncementPriority::Medium);
        }
    }
}

#[cfg(not(feature = "v4_14"))]
impl crate::view::RotulusView {
    pub(crate) fn a11y_move_caret(&self, _range: Option<rotulus_layout::Selection>) {}
    pub(crate) fn a11y_announce(&self, _message: &str) {}
    pub(crate) fn a11y_appended(&self, _id: MessageId, _trimmed: usize) {}
    pub(crate) fn a11y_inserted(&self, _id: MessageId) {}
    pub(crate) fn a11y_removed(&self, _id: MessageId) {}
    pub(crate) fn a11y_replaced(&self, _id: MessageId) {}
    pub(crate) fn a11y_reset(&self) {}
    pub(crate) fn a11y_selection_changed(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use rotulus_layout::{
        Block, FixedMeasure, LayoutParams, Message, MessageFlags, MessageKind, ParsedText, Speaker,
    };

    fn said(nick: &str, body: &str) -> Message {
        Message {
            kind: MessageKind::Live,
            timestamp: 1_000,
            speaker: Some(Speaker::new(1, nick)),
            gutter: Some(ParsedText::plain(format!("<{nick}>"))),
            blocks: vec![Block::Text(ParsedText::plain(body))].into(),
            flags: MessageFlags::NONE,
        }
    }

    fn plain() -> RowFormat {
        RowFormat {
            stamp: Box::new(|_| None),
            linkifier: Linkifier::default(),
        }
    }

    fn buffer(rows: &[Message]) -> ChatBuffer {
        let m = FixedMeasure::new(8);
        let mut b = ChatBuffer::new(LayoutParams::default());
        for r in rows {
            b.append(r.clone(), &m);
        }
        b
    }

    #[test]
    fn rows_read_as_lines_of_nick_and_message() {
        let b = buffer(&[said("al", "hello"), said("bo", "hi there")]);
        let m = TextModel::build(&b, &plain());
        assert_eq!(m.text(), "al: hello\nbo: hi there");
        assert_eq!(m.char_len(), m.text().chars().count() as u32);
    }

    #[test]
    fn a_row_with_no_speaker_reads_its_gutter() {
        let mut row = said("al", "joined");
        row.speaker = None;
        row.gutter = Some(ParsedText::plain("-->"));
        let m = TextModel::build(&buffer(&[row]), &plain());
        assert_eq!(m.text(), "--> joined");
    }

    #[test]
    fn a_grouped_row_still_names_its_speaker() {
        let m = FixedMeasure::new(8);
        let mut b = ChatBuffer::new(LayoutParams::default());
        b.set_group_gap_secs(300, &m);
        b.append(said("al", "one"), &m);
        b.append(said("al", "two"), &m);
        assert!(b
            .message_at(1)
            .unwrap()
            .flags
            .contains(MessageFlags::GROUPED));
        let t = TextModel::build(&b, &plain());
        assert_eq!(t.text(), "al: one\nal: two");
    }

    #[test]
    fn timestamps_lead_each_row_when_shown() {
        let b = buffer(&[said("al", "hello")]);
        let fmt = RowFormat {
            stamp: Box::new(|_| Some("[12:00] ".to_string())),
            linkifier: Linkifier::default(),
        };
        let m = TextModel::build(&b, &fmt);
        assert_eq!(m.text(), "[12:00] al: hello");
    }

    #[test]
    fn contents_are_addressed_in_characters() {
        let b = buffer(&[said("é", "ñandú")]);
        let m = TextModel::build(&b, &plain());
        assert_eq!(m.text(), "é: ñandú");
        assert_eq!(m.contents(3, 8), "ñandú");
        assert_eq!(
            m.contents(3, u32::MAX),
            "ñandú",
            "an end past the text clamps"
        );
        assert_eq!(
            m.contents(8, 3),
            "ñandú",
            "a reversed range is the same range"
        );
    }

    #[test]
    fn granularities_find_their_unit_and_what_follows_it() {
        let b = buffer(&[said("al", "one two"), said("bo", "three")]);
        let m = TextModel::build(&b, &plain());
        assert_eq!(m.text(), "al: one two\nbo: three");
        assert_eq!(m.contents_at(5, Granularity::Word), (4, 8, "one "));
        assert_eq!(
            m.contents_at(7, Granularity::Word),
            (4, 8, "one "),
            "the space after a word belongs to it"
        );
        assert_eq!(m.contents_at(5, Granularity::Character).2, "n");
        assert_eq!(
            m.contents_at(5, Granularity::Line),
            (0, 12, "al: one two\n")
        );
        assert_eq!(m.contents_at(16, Granularity::Line), (12, 21, "bo: three"));
        assert_eq!(m.contents_at(5, Granularity::Paragraph).2, "al: one two\n");
        assert_eq!(m.contents_at(16, Granularity::Paragraph).2, "bo: three");
    }

    #[test]
    fn stepping_by_unit_walks_the_whole_text() {
        let b = buffer(&[said("al", "  one  two "), said("bo", "three\n")]);
        let m = TextModel::build(&b, &plain());
        for g in [
            Granularity::Character,
            Granularity::Word,
            Granularity::Line,
            Granularity::Paragraph,
        ] {
            let mut at = 0;
            let mut read = String::new();
            while at < m.char_len() {
                let (start, end, text) = m.contents_at(at, g);
                assert!(start <= at && at < end, "{g:?} at {at} gave {start}..{end}");
                read.push_str(text);
                at = end;
            }
            assert_eq!(read, m.text(), "stepping by {g:?} reads everything once");
        }
    }

    #[test]
    fn a_disguised_link_reads_with_its_address() {
        let lk = Linkifier::default();
        let mut body = ParsedText::plain("see docs or https://x.org ok");
        body.add_link(4..8, "https://example.com/docs");
        crate::links::autolink(&mut body, &lk);
        let mut row = said("al", "");
        row.blocks = vec![Block::Text(body)].into();
        let b = buffer(&[row]);
        let m = TextModel::build(&b, &plain());
        assert_eq!(
            m.text(),
            "al: see docs (https://example.com/docs) or https://x.org ok",
            "only the link whose label hides its address gets it read out"
        );
        let id = b.id_at(0).unwrap();
        let at = |offset| Caret {
            message: id,
            source: LineSource::Block(0),
            offset,
        };
        assert_eq!(m.offset_of(&at("see docs or ".len()), &b), Some(43));
        assert_eq!(&m.text()[43..45], "ht");
    }

    #[test]
    fn a_link_overlapping_the_last_one_is_left_alone() {
        let mut body = ParsedText::plain("abcdef");
        body.links = vec![
            rotulus_layout::Link {
                href: "https://one.example".into(),
                range: 0..4,
            },
            rotulus_layout::Link {
                href: "https://two.example".into(),
                range: 2..6,
            },
        ];
        let mut row = said("al", "");
        row.blocks = vec![Block::Text(body)].into();
        let m = TextModel::build(&buffer(&[row]), &plain());
        assert_eq!(m.text(), "al: abcd (https://one.example)ef");
    }

    #[test]
    fn carets_and_offsets_map_both_ways() {
        let mut b = buffer(&[said("al", "one"), said("bo", "ñandú")]);
        let mut row = said("cy", "x");
        let mut body = ParsedText::plain("a b c");
        body.add_link(2..3, "https://b.example");
        row.blocks = vec![Block::Text(body), Block::text("é")].into();
        b.append(row, &FixedMeasure::new(8));
        let m = TextModel::build(&b, &plain());
        for r in 0..b.len() {
            let id = b.id_at(r).unwrap();
            for bi in 0..b.message_at(r).unwrap().blocks.len() {
                let source = LineSource::Block(bi);
                let text = b.source_text(r, source).unwrap();
                for (offset, _) in text.char_indices().chain([(text.len(), ' ')]) {
                    let c = Caret {
                        message: id,
                        source,
                        offset,
                    };
                    let o = m.offset_of(&c, &b).unwrap();
                    assert_eq!(m.caret_at(o, &b), Some(c), "row {r} block {bi} at {offset}");
                }
            }
        }
        // A character no source has — the speaker's separator — gives
        // the caret before it.
        let id = b.id_at(0).unwrap();
        assert_eq!(
            m.caret_at(3, &b),
            Some(Caret {
                message: id,
                source: LineSource::Gutter,
                offset: 0
            })
        );
    }

    #[test]
    fn an_append_extends_the_text_and_reports_where() {
        let mut b = buffer(&[said("al", "one")]);
        let mut m = TextModel::build(&b, &plain());
        let id = b.append(said("bo", "two"), &FixedMeasure::new(8));
        let (start, end) = m.push(&b, id, &plain());
        assert_eq!(m.text(), "al: one\nbo: two");
        assert_eq!(
            (start, end),
            (7, m.char_len()),
            "the insert starts at the old end, newline included"
        );
        assert_eq!(
            TextModel::build(&b, &plain()).text(),
            m.text(),
            "incremental matches a rebuild"
        );
    }

    #[test]
    fn inserts_and_removes_match_a_rebuild_and_report_their_ranges() {
        let m8 = FixedMeasure::new(8);
        let mut b = buffer(&[said("al", "one"), said("bo", "two"), said("cy", "three")]);
        let mut m = TextModel::build(&b, &plain());

        // Above the end, the way a page of history arrives.
        let id = b.insert_before(b.id_at(1), said("dee", "older"), &m8);
        let (a, z) = m.insert_row(&b, b.row_of(id).unwrap(), id, &plain());
        assert_eq!(m.text(), TextModel::build(&b, &plain()).text());
        assert_eq!(
            &m.text()[a as usize..z as usize],
            "dee: older\n",
            "the row and its separator"
        );

        // From the middle, the front and the end.
        for pick in [2usize, 0, 1] {
            let gone = b.id_at(pick).unwrap();
            let i = m.index_of(gone).unwrap();
            let before = m.text().to_string();
            b.remove(gone, &m8);
            let (a, z) = m.remove_row(i).unwrap();
            assert_eq!(m.text(), TextModel::build(&b, &plain()).text());
            assert_eq!(
                z - a,
                (before.chars().count() - m.text().chars().count()) as u32
            );
        }
        let last = b.id_at(0).unwrap();
        b.remove(last, &m8);
        m.remove_row(0).unwrap();
        assert_eq!(m.text(), "");
    }

    #[test]
    fn a_trim_at_the_cap_finds_the_rows_that_went() {
        let m8 = FixedMeasure::new(8);
        let mut b = buffer(&[said("al", "one"), said("bo", "two"), said("cy", "three")]);
        b.set_max_rows(3, &m8);
        let mut m = TextModel::build(&b, &plain());
        let id = b.append(said("dee", "four"), &m8);
        assert_eq!(b.len(), 3, "the append trimmed one");
        let gone = m.gone(&b, 1);
        assert_eq!(gone, [0]);
        m.remove_row(0).unwrap();
        m.push(&b, id, &plain());
        assert_eq!(m.text(), TextModel::build(&b, &plain()).text());
    }

    #[test]
    fn a_caret_maps_to_its_character_offset() {
        let b = buffer(&[said("al", "one"), said("bo", "ñandú")]);
        let m = TextModel::build(&b, &plain());
        let id = b.id_at(1).unwrap();
        let caret = Caret {
            message: id,
            source: LineSource::Block(0),
            offset: "ña".len(),
        };
        // "al: one\nbo: ñandú": row 1 starts at 8, its body at 12.
        assert_eq!(m.offset_of(&caret, &b), Some(14));
        let gutter = Caret {
            message: id,
            source: LineSource::Gutter,
            offset: 3,
        };
        assert_eq!(
            m.offset_of(&gutter, &b),
            Some(8),
            "the gutter's characters aren't read, so it maps to the name"
        );
    }
}
