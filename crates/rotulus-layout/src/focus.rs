//! What keyboard focus steps through.
//!
//! The view draws a transcript, not widgets, so Tab has nothing of GTK's
//! to move between. These are the things a click does something to — a
//! speaker, a link, a load-more row, an image — in reading order, so that
//! the keyboard reaches each of them, in the order they are read, and
//! nothing else.

use std::ops::Range;

use crate::buffer::ChatBuffer;
use crate::message::{Block, MessageFlags, MessageId, MessageKind};
use crate::wrap::LineSource;

/// One thing in the scrollback that does something when activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusItem {
    /// A speaker's name in the gutter, and their avatar with it.
    Speaker { message: MessageId },
    /// A link: the text it lives in and its byte range there.
    Link {
        message: MessageId,
        source: LineSource,
        range: Range<usize>,
    },
    /// A load-more row, all of it.
    LoadMore { message: MessageId },
    /// An inline image, or the placeholder standing in for one.
    Image { message: MessageId, block: usize },
}

impl FocusItem {
    /// The row the item is on.
    pub fn message(&self) -> MessageId {
        match self {
            FocusItem::Speaker { message }
            | FocusItem::Link { message, .. }
            | FocusItem::LoadMore { message }
            | FocusItem::Image { message, .. } => *message,
        }
    }
}

impl ChatBuffer {
    /// A row's items, in reading order: the speaker, then each block's
    /// links or image in turn. A load-more row is one item, whatever it
    /// says.
    pub fn focus_items(&self, row: usize) -> Vec<FocusItem> {
        let (Some(message), Some(msg)) = (self.id_at(row), self.message_at(row)) else {
            return Vec::new();
        };
        if let MessageKind::LoadMore(_) = msg.kind {
            return vec![FocusItem::LoadMore { message }];
        }
        let mut out = Vec::new();
        // The gutter a grouped row doesn't draw can't be clicked either.
        let shown = !msg.flags.contains(MessageFlags::GROUPED)
            && (msg.gutter.as_ref().is_some_and(|g| !g.text.is_empty())
                || self.params().avatar_size > 0);
        if shown && msg.speaker.as_ref().is_some_and(|s| s.key != 0) {
            out.push(FocusItem::Speaker { message });
        }
        for (bi, b) in msg.blocks.iter().enumerate() {
            let parsed = match b {
                Block::Text(p) | Block::Quote { content: p, .. } => p,
                Block::Image { .. } => {
                    out.push(FocusItem::Image { message, block: bi });
                    continue;
                }
                Block::Code { .. } => continue,
            };
            let mut links: Vec<_> = parsed.links.iter().map(|l| l.range.clone()).collect();
            links.sort_by_key(|r| r.start);
            out.extend(links.into_iter().map(|range| FocusItem::Link {
                message,
                source: LineSource::Block(bi),
                range,
            }));
        }
        out
    }

    /// The item after `from` (or before it, going back) among the rows
    /// `rows` spans, inclusive — what is on screen — crossing rows as
    /// needed. Without `from`, or when it isn't in those rows, the first
    /// item there (or the last, going back). `None` past either end, so
    /// focus leaves rather than walking the whole scrollback.
    pub fn step_focus(
        &self,
        from: Option<&FocusItem>,
        forward: bool,
        rows: (usize, usize),
    ) -> Option<FocusItem> {
        let (first, last) = (rows.0, rows.1.min(self.len().checked_sub(1)?));
        let shown = |r: usize| r >= first && r <= last;
        let mut row = if forward { first } else { last };
        if let Some(f) = from {
            if let Some(r) = self.row_of(f.message()).filter(|r| shown(*r)) {
                let items = self.focus_items(r);
                if let Some(at) = items.iter().position(|i| i == f) {
                    let next = if forward {
                        items.get(at + 1)
                    } else {
                        at.checked_sub(1).and_then(|i| items.get(i))
                    };
                    if let Some(next) = next {
                        return Some(next.clone());
                    }
                    row = if forward { r + 1 } else { r.checked_sub(1)? };
                }
            }
        }
        while shown(row) {
            let items = self.focus_items(row);
            let pick = if forward {
                items.into_iter().next()
            } else {
                items.into_iter().next_back()
            };
            if pick.is_some() {
                return pick;
            }
            row = if forward {
                row + 1
            } else {
                row.checked_sub(1)?
            };
        }
        None
    }
}
