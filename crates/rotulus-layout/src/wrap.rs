//! Line breaking and per-message layout.
//!
//! Produces a [`LayoutCache`] for one message at one width: its total
//! pixel height and the line boxes needed for hit-testing. This is where
//! both layouts live: two columns, with the timestamp and nick gutter on
//! the left and the body indented past it, and one, with the nick at the
//! start of the body's first line.
//!
//! Measurement goes through [`TextMeasure`], one call per *run*, never
//! per character. xtext's `find_next_wrap` (xtext.c:3685) measured
//! character by character through a full Pango layout round trip each
//! time; that is the single hottest thing in its append path and the
//! reason a resize of a large buffer visibly hitches.

use crate::measure::TextMeasure;
use crate::message::{Block, Message, MessageFlags, Reaction, Reply};
use crate::span::{ParsedText, Span, Style};
use std::ops::Range;

/// Identifies the conditions a [`LayoutCache`] was computed under.
///
/// A cache is valid only for the generation it was built at; anything
/// that changes rendered geometry bumps a field. Critically, bumping a
/// generation does **not** trigger recomputation — caches are rebuilt
/// lazily when a row is next laid out, so a resize costs O(visible).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct LayoutGeneration {
    /// Content width in pixels.
    pub width: u32,
    /// Bumped on font change.
    pub font: u32,
    /// Bumped on theme change (palette resolution can alter nothing
    /// geometric, but a theme may carry a font).
    pub theme: u32,
    /// Zoom level in per-mille, so the generation stays `Eq`-comparable.
    /// See docs/design.md "Layout, the height index, and scroll anchoring".
    pub zoom_permille: u32,
}

/// What a [`LineBox`] draws text from.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineSource {
    /// The left column — the pre-rendered gutter, or the speaker's nick.
    /// Always the message's first line box when present.
    Gutter,
    /// Body block `n`.
    Block(usize),
}

/// One visual line within a message.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LineBox {
    /// Y offset from the top of the message.
    pub y: u32,
    pub height: u32,
    /// Byte range within the owning source's text.
    pub range: Range<usize>,
    /// Where this line's text comes from.
    pub source: LineSource,
    /// X offset — the indent column, or 0 for full-width content.
    pub x: u32,
    /// Measured width of the line's text, in px.
    ///
    /// Recorded here rather than re-measured by the view because the
    /// wrap pass already knows it — it is the number the break decision
    /// was made on. Anything that wants to draw a box around laid-out
    /// text (a fenced code block, today) gets the same extent the
    /// wrapper used, so the box cannot disagree with the wrap.
    pub width: u32,
}

/// What a laid-out message knows about itself.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LayoutCache {
    pub generation: LayoutGeneration,
    pub height: u32,
    pub lines: Vec<LineBox>,
    /// Width the gutter/nick column actually needed, before clamping to
    /// [`LayoutParams::max_indent`].
    pub natural_indent: u32,
    /// Where to paint the speaker's avatar, when there is one to paint.
    ///
    /// Only ever set on a *group head* — a continuation row shows
    /// neither the nick nor the icon, which is the whole point of
    /// grouping. The view resolves the actual image from the key at
    /// draw time, because avatars animate and a cached frame would
    /// freeze.
    pub avatar: Option<AvatarBox>,
    /// The reply header, when the row has one. Boxed, as the layout
    /// itself is, so a row without one pays a pointer.
    pub reply: Option<Box<ReplyBox>>,
    /// The reaction chips under the body, in the order given; boxed for
    /// the same reason.
    pub chips: Option<Box<[ChipBox]>>,
}

/// Where a reaction chip goes, in row-local coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ChipBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// Where the key's text starts, and the count's after it.
    pub key_x: u32,
    pub count_x: u32,
}

impl LayoutCache {
    /// Where the timestamp goes: the top of the row's lines, below the gap
    /// a reply leaves above them.
    pub fn stamp_y(&self) -> u32 {
        self.reply.as_ref().map_or(0, |r| reply_gap(r.height))
    }
}

/// The space above a row that replies, for a line `line_h` tall.
pub(crate) fn reply_gap(line_h: u32) -> u32 {
    (line_h / 4).max(1)
}

/// Where a row's reply header goes, in row-local coordinates: a bar, the
/// quoted nick, then the excerpt, on one line.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReplyBox {
    pub x: u32,
    pub y: u32,
    /// As far as anything is drawn, so a click lands where something is.
    pub width: u32,
    pub height: u32,
    /// The quoted nick; `None` when the original is unavailable.
    pub nick: Option<ReplyText>,
    /// The excerpt, or the unavailable text.
    pub text: ReplyText,
}

/// One text of a reply header, cut to fit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ReplyText {
    pub x: u32,
    /// What is drawn, the ellipsis included.
    pub width: u32,
    /// How many bytes of the text are drawn.
    pub len: usize,
    /// Whether [`ELLIPSIS`] follows them.
    pub ellipsis: bool,
}

/// What a cut reply text ends with.
pub const ELLIPSIS: &str = "\u{2026}";

/// How many characters of a reply's text are kept: more than a line
/// shows, few enough that shaping it costs little.
pub const REPLY_TEXT_CAP: usize = 300;

/// Pixels between a chip's edge and its text, above and below; and
/// between the body and the chips, between rows of chips, and under the
/// last, so the next row's header doesn't touch them.
const CHIP_PAD_Y: u32 = 2;
pub(crate) const CHIP_GAP_Y: u32 = 2;

/// A square avatar slot in the gutter, in row-local coordinates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct AvatarBox {
    pub x: u32,
    pub y: u32,
    pub size: u32,
    pub key: u64,
}

/// Geometry knobs, supplied by the view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LayoutParams {
    /// Total content width available.
    pub width: u32,
    /// Two-column mode: body indented past a nick/timestamp gutter.
    pub indent: bool,
    /// Cap on the gutter width. Below it the gutter grows to fit the
    /// widest nick seen, which is what `gtk_xtext_fix_indent` did.
    pub max_indent: u32,
    /// Current gutter width, shared across the buffer so columns line
    /// up between rows.
    pub indent_width: u32,
    /// Width reserved at the left of the gutter for the timestamp
    /// column, or 0 when timestamps are off.
    ///
    /// The layout engine never formats a timestamp — it can't, having no
    /// locale or time formatting — it only reserves the space the view
    /// says it needs, so the gutter is wide enough for stamp + nick.
    pub stamp_width: u32,
    /// Gap between the right edge of the gutter and the body column.
    pub gutter_gap: u32,
    /// Left padding inside a quote block, per nesting level.
    pub quote_indent: u32,
    /// Vertical padding above and below an image or code block.
    pub block_padding: u32,
    pub word_wrap: bool,
    /// Edge length of the avatar slot in the gutter, or 0 for no
    /// avatars. Driven by the Settings toggle.
    pub avatar_size: u32,
}

impl Default for LayoutParams {
    fn default() -> Self {
        LayoutParams {
            width: 640,
            indent: true,
            max_indent: 256,
            indent_width: 0,
            stamp_width: 0,
            gutter_gap: 6,
            quote_indent: 12,
            block_padding: 2,
            word_wrap: true,
            avatar_size: 0,
        }
    }
}

/// Lay one message out.
pub fn layout_message(
    msg: &Message,
    params: &LayoutParams,
    generation: LayoutGeneration,
    measure: &dyn TextMeasure,
) -> LayoutCache {
    layout_row(msg, None, &[], params, generation, measure)
}

/// Lay one row out: its message, under the reply header when it has one
/// and over its reactions.
pub fn layout_row(
    msg: &Message,
    reply: Option<&Reply>,
    reactions: &[Reaction],
    params: &LayoutParams,
    generation: LayoutGeneration,
    measure: &dyn TextMeasure,
) -> LayoutCache {
    let metrics = measure.metrics();
    let line_h = metrics.line_height.max(1);
    let mut lines = Vec::new();

    // The gutter is only as wide as this message needs; the buffer
    // reconciles the maximum across rows and re-lays out when it grows.
    //
    // A pre-rendered gutter (see Message::gutter) wins over the speaker's
    // bare nick, because it carries the styling the application chose.
    // A grouped row is a continuation of the one above: same speaker,
    // close in time. Its gutter is suppressed so a burst of messages
    // reads as one block under one name instead of repeating the nick
    // (and, once avatars land, the icon) on every line. The body keeps
    // its indent, so the column stays straight.
    //
    // The flag is set by ChatBuffer, which is the only thing that can
    // see a message's neighbours. It still contributes its *natural*
    // gutter width below, so hiding a nick never narrows the shared
    // column and shifts every other row sideways.
    let grouped = msg.flags.contains(crate::message::MessageFlags::GROUPED);

    // An avatar is only drawn on a group head, but *every* row with a
    // speaker reserves its width — otherwise the shared gutter narrows
    // the moment a run forms and every column in the buffer jumps.
    let avatar_slot = if params.indent && params.avatar_size > 0 {
        match &msg.speaker {
            Some(s) if s.key != 0 => params.avatar_size + params.gutter_gap,
            _ => 0,
        }
    } else {
        0
    };

    let natural_indent = if params.indent {
        let gutter_text = match (&msg.gutter, &msg.speaker) {
            (Some(g), _) if !g.text.is_empty() => {
                measure_styled(g, 0..g.text.len(), measure) + metrics.space_width
            }
            (_, Some(s)) => measure.run_width(&s.nick, Style::default()) + metrics.space_width * 2,
            _ => 0,
        };
        // The gutter holds the timestamp *and* the nick, side by side,
        // so it has to be wide enough for both — reserving only the nick
        // width is what makes a stamp overlap it.
        params.stamp_width + avatar_slot + gutter_text
    } else {
        0
    };

    let body_x = if params.indent {
        params.indent_width
    } else {
        0
    };
    let body_width = params.width.saturating_sub(body_x).max(line_h);

    // A row that replies starts a little apart from the row above, so
    // its quote can't be read as the end of that row's message.
    let top = if reply.is_some() {
        reply_gap(line_h)
    } else {
        0
    };
    let mut y = top;

    // The gutter is one unwrapped line, always first, carrying the x it
    // is actually drawn at.
    //
    // Right-aligned against the body column, the way xtext aligns its
    // left text (`ent->indent = buf->indent - left_width - space_width`,
    // xtext.c). Computing it *here* rather than in the view matters: the
    // view previously derived it from `params().indent_width`, which is
    // only ever set on a local copy inside `ensure_layout` and so read
    // back as 0 — the gutter drew hard left while bodies moved right as
    // the column grew, and hit-testing used the same wrong x, which is
    // why nicks could not be selected. One source of truth removes both
    // bugs at once.
    // The avatar sits at the left of the gutter, just past the stamp,
    // with the nick to its right. Group heads only.
    let avatar = if avatar_slot > 0 && !grouped {
        msg.speaker.as_ref().map(|s| AvatarBox {
            x: params.stamp_width,
            y: top,
            size: params.avatar_size,
            key: s.key,
        })
    } else {
        None
    };

    // Where the first body line starts, in single-column mode: past the
    // timestamp and the gutter, which share its line instead of having a
    // column of their own. Zero in two-column mode, where both live in
    // the gutter column.
    let mut lead = 0u32;
    let mut gutter_end = 0u32;
    let gutter = msg
        .gutter
        .as_ref()
        .filter(|g| !g.text.is_empty() && !grouped);
    if let Some(g) = gutter {
        let gw = measure_styled(g, 0..g.text.len(), measure);
        let gx = if params.indent {
            params
                .indent_width
                .saturating_sub(gw + params.gutter_gap)
                .max(params.stamp_width)
        } else {
            params.stamp_width
        };
        lines.push(LineBox {
            y: top,
            height: line_h,
            range: 0..g.text.len(),
            source: LineSource::Gutter,
            x: gx,
            width: gw,
        });
        gutter_end = gx + gw + params.gutter_gap;
        if !params.indent {
            lead = gx + gw + metrics.space_width;
        }
    } else if !params.indent {
        lead = params.stamp_width;
    }
    // Only a text body can start beside the gutter. Anything else — a
    // code block, a quote, an image — starts on the line below it, since
    // it has a box or an indent of its own that the lead would break. So
    // does a body with less than half the width left beside a long nick:
    // one word per line down the right edge is worse than a line break.
    //
    // Either way the first line is left to the stamp and the gutter, which
    // the view draws there; a lead is only ever nonzero because one of
    // them is.
    let beside = lead;
    if lead > 0 && (!matches!(msg.blocks.first(), Some(Block::Text(_))) || lead > body_width / 2) {
        y = top + line_h;
        lead = 0;
    }

    // The reply header goes on the row's first line, beside the nick: in
    // the body column, or in one column where the body would have begun.
    // That line is the row's own, so the quote reads as the start of this
    // message rather than the end of the one above; the body starts on
    // the line after it.
    //
    // A nick too long for its column runs into the body's, and the header
    // starts past it. In one column, a nick that leaves the header too
    // little of its line puts the header on the next, from the edge, as
    // the body would wrap.
    let mut reply = reply.map(|r| {
        let (x, hy) = if params.indent {
            (body_x.max(gutter_end), top)
        } else if params.width.saturating_sub(beside) < 4 * line_h {
            (0, top + line_h)
        } else {
            (beside, top)
        };
        let mut b = layout_reply(r, x, params.width.saturating_sub(x), line_h, measure);
        b.y = hy;
        Box::new(b)
    });
    if let Some(r) = &reply {
        y = r.y + line_h;
        lead = 0;
    }

    for (bi, block) in msg.blocks.iter().enumerate() {
        match block {
            Block::Text(p) => {
                let first = if bi == 0 { lead } else { 0 };
                let n = wrap_text(
                    p,
                    body_width,
                    first,
                    params.word_wrap,
                    measure,
                    |range, lw, dx| {
                        lines.push(LineBox {
                            y,
                            height: line_h,
                            range,
                            source: LineSource::Block(bi),
                            x: body_x + dx,
                            width: lw,
                        });
                        y += line_h;
                    },
                );
                // An empty block still occupies a line, so a blank
                // message doesn't collapse to zero height and become
                // unclickable.
                if n == 0 {
                    lines.push(LineBox {
                        y,
                        height: line_h,
                        range: 0..0,
                        source: LineSource::Block(bi),
                        x: body_x + first,
                        width: 0,
                    });
                    y += line_h;
                }
            }
            Block::Code { text, .. } => {
                y += params.block_padding;
                // Code never wraps on words — breaking a token would
                // change what it says. Overflow clips.
                let mut start = 0usize;
                // Code blocks carry no spans, so they draw — and so must
                // measure — in the default style.
                let plain = Style::default();
                for (i, _) in text.match_indices('\n') {
                    lines.push(LineBox {
                        y,
                        height: line_h,
                        range: start..i,
                        source: LineSource::Block(bi),
                        x: body_x,
                        width: measure.run_width(&text[start..i], plain),
                    });
                    y += line_h;
                    start = i + 1;
                }
                lines.push(LineBox {
                    y,
                    height: line_h,
                    range: start..text.len(),
                    source: LineSource::Block(bi),
                    x: body_x,
                    width: measure.run_width(&text[start..], plain),
                });
                y += line_h + params.block_padding;
            }
            Block::Quote { content, depth } => {
                let qx = body_x + params.quote_indent * u32::from(*depth).max(1);
                let qw = params.width.saturating_sub(qx).max(line_h);
                let n = wrap_text(content, qw, 0, params.word_wrap, measure, |range, lw, _| {
                    lines.push(LineBox {
                        y,
                        height: line_h,
                        range,
                        source: LineSource::Block(bi),
                        x: qx,
                        width: lw,
                    });
                    y += line_h;
                });
                // An empty quote still gets a line box, for the same
                // reason an empty text block does: a row with no line
                // boxes occupies vertical space that nothing can hit-test
                // or select, which reads as a dead patch in the buffer.
                if n == 0 {
                    lines.push(LineBox {
                        y,
                        height: line_h,
                        range: 0..0,
                        source: LineSource::Block(bi),
                        x: qx,
                        width: 0,
                    });
                    y += line_h;
                }
            }
            Block::Image { size, alt, .. } => match size {
                // The decode has landed: a real pixel height, not
                // `ceil(h / fontsize)` blank text rows with a texture
                // painted across them (xtext.c:4345). Selection,
                // hit-testing and the marker line all just work.
                Some(sz) => {
                    y += params.block_padding;
                    let (iw, h) = measure.image_size((sz.width, sz.height), body_width);
                    lines.push(LineBox {
                        y,
                        height: h,
                        range: 0..alt.len(),
                        source: LineSource::Block(bi),
                        x: body_x,
                        width: iw,
                    });
                    y += h + params.block_padding;
                }
                // Not decoded yet — measure as the placeholder text, so
                // the row doesn't jump when the bytes arrive any more
                // than it has to.
                None => {
                    let p = ParsedText::plain(alt.clone());
                    let n = wrap_text(
                        &p,
                        body_width,
                        0,
                        params.word_wrap,
                        measure,
                        |range, lw, _| {
                            lines.push(LineBox {
                                y,
                                height: line_h,
                                range,
                                source: LineSource::Block(bi),
                                x: body_x,
                                width: lw,
                            });
                            y += line_h;
                        },
                    );
                    // Same rule: an image whose alt text is empty must
                    // still be clickable — that is how the user opens
                    // the media dialog before the decode lands.
                    if n == 0 {
                        lines.push(LineBox {
                            y,
                            height: line_h,
                            range: 0..0,
                            source: LineSource::Block(bi),
                            x: body_x,
                            width: 0,
                        });
                        y += line_h;
                    }
                }
            },
        }
    }

    // At least a line, past whatever is already placed: a header alone
    // may sit on the second.
    if lines.is_empty() {
        y = y.max(top + line_h);
    }

    // Under the body, in its column, wrapping as words do; under the
    // row's first line at least, which a nick has with no body beside it.
    let mut chips = (!reactions.is_empty()).then(|| {
        y = y.max(top + line_h);
        let (boxes, h) = layout_chips(reactions, body_x, y, body_width, line_h, measure);
        y += h;
        boxes.into_boxed_slice()
    });

    // Centre the text against the icon when the icon is the taller of
    // the two.
    //
    // Without this the text sits on the icon's top edge, which reads as
    // misalignment rather than as a design: the eye pairs a 32px icon
    // with the line beside it, and "beside" means centred. Shifting
    // every line box (rather than only the first) keeps a two-line
    // message centred as a block, which is what looks right when the
    // message is still shorter than the icon.
    //
    // Done here, on the finished line boxes, so hit-testing and painting
    // move together — the alternative, offsetting only at draw time, is
    // the class of bug where you can see text you cannot select.
    let text_h = (y - top).max(line_h);
    let row_h = text_h.max(avatar.map(|a| a.size).unwrap_or(0));
    if row_h > text_h {
        let shift = (row_h - text_h) / 2;
        for l in &mut lines {
            l.y += shift;
        }
        for c in chips.iter_mut().flat_map(|c| c.iter_mut()) {
            c.y += shift;
        }
        if let Some(r) = &mut reply {
            r.y += shift;
        }
    }

    LayoutCache {
        generation,
        // A head row must be at least as tall as its avatar, or the
        // icon overflows into the row below. This is the case the
        // "re-estimate on regroup" fix in ChatBuffer exists for: a head
        // and its continuations now genuinely differ in height.
        height: top + row_h,
        lines,
        natural_indent: natural_indent.min(params.max_indent),
        avatar,
        reply,
        chips,
    }
}

/// Chips for `reactions` from `(x, y)` in `width`, each starting a new
/// line when it doesn't fit beside the last, and the height they take,
/// the gaps above and below them included. A chip wider than the line overflows it
/// rather than being cut: its key is the reaction.
fn layout_chips(
    reactions: &[Reaction],
    x: u32,
    y: u32,
    width: u32,
    line_h: u32,
    measure: &dyn TextMeasure,
) -> (Vec<ChipBox>, u32) {
    let space = measure.metrics().space_width;
    let height = line_h + 2 * CHIP_PAD_Y;
    let top = y + CHIP_GAP_Y;
    let (mut cx, mut cy) = (x, top);
    let mut out = Vec::with_capacity(reactions.len());
    for r in reactions {
        let key_w = measure.run_width(&r.key, Style::default());
        let count_w = match r.count {
            0 => 0,
            n => space + measure.run_width(&n.to_string(), Style::default()),
        };
        let w = space + key_w + count_w + space;
        if cx > x && cx + w > x + width {
            (cx, cy) = (x, cy + height + CHIP_GAP_Y);
        }
        out.push(ChipBox {
            x: cx,
            y: cy,
            width: w,
            height,
            key_x: cx + space,
            count_x: cx + space + key_w + space,
        });
        cx += w + space / 2;
    }
    (out, cy + height + CHIP_GAP_Y - y)
}

/// The header for `r` at `x`, cut to `width`: two spaces' width with the
/// bar in it, the nick, a space, then the text.
fn layout_reply(r: &Reply, x: u32, width: u32, line_h: u32, measure: &dyn TextMeasure) -> ReplyBox {
    let space = measure.metrics().space_width;
    let style = |attrs| Style {
        attrs,
        ..Style::default()
    };
    let mut at = 2 * space;
    let (nick, text) = match r {
        Reply::Quote { nick, excerpt, .. } => {
            // At most about half, so some of what they said shows.
            let n = fit_ellipsized(
                nick,
                style(Reply::NICK_ATTRS),
                x + at,
                width.saturating_sub(at).min(width / 2),
                measure,
            );
            at += n.width + space;
            (Some(n), excerpt)
        }
        Reply::Unavailable(t) => (None, t),
    };
    let text = fit_ellipsized(
        text,
        style(r.text_attrs()),
        x + at,
        width.saturating_sub(at),
        measure,
    );
    let end = if text.width > 0 {
        text.x + text.width
    } else {
        nick.map_or(text.x, |n| n.x + n.width)
    };
    ReplyBox {
        x,
        y: 0,
        // At least a pixel, so focus on it has somewhere to be.
        width: (end - x).min(width).max(1),
        height: line_h,
        nick,
        text,
    }
}

/// As much of `text` as fits in `max` at `x`, with an ellipsis when that
/// isn't all of it, or nothing when not even the ellipsis fits.
fn fit_ellipsized(
    text: &str,
    style: Style,
    x: u32,
    max: u32,
    measure: &dyn TextMeasure,
) -> ReplyText {
    let full = measure.run_width(text, style);
    if full <= max {
        return ReplyText {
            x,
            width: full,
            len: text.len(),
            ellipsis: false,
        };
    }
    let e = measure.run_width(ELLIPSIS, style);
    let Some(budget) = max.checked_sub(e) else {
        return ReplyText {
            x,
            width: 0,
            len: 0,
            ellipsis: false,
        };
    };
    // fit_prefix makes progress past the budget rather than none, which
    // here would push the ellipsis past the edge.
    let (mut len, mut w) = measure.fit_prefix(text, style, budget);
    while w > budget && len > 0 {
        len = text[..len].char_indices().last().map_or(0, |(i, _)| i);
        w = measure.run_width(&text[..len], style);
    }
    // No space between the last word and the ellipsis.
    let kept = text[..len].trim_end();
    let w = if kept.len() == len {
        w
    } else {
        measure.run_width(kept, style)
    };
    ReplyText {
        x,
        width: w + e,
        len: kept.len(),
        ellipsis: true,
    }
}

/// Break `p` into visual lines, calling `emit` per line with the line's
/// range, its width, and how far right of the block's x it starts.
/// Returns the count.
///
/// The first line starts `first_indent` in and is that much narrower;
/// every later line has the full width. That is how a single-column row
/// puts its body beside the nick rather than under it.
fn wrap_text(
    p: &ParsedText,
    width: u32,
    first_indent: u32,
    word_wrap: bool,
    measure: &dyn TextMeasure,
    mut emit: impl FnMut(Range<usize>, u32, u32),
) -> usize {
    if p.text.is_empty() {
        return 0;
    }
    let mut count = 0usize;
    let mut line_start = 0usize;
    let first_width = width.saturating_sub(first_indent).max(1);
    let avail = |count: usize| if count == 0 { first_width } else { width };
    let dx = |count: usize| if count == 0 { first_indent } else { 0 };

    while line_start < p.text.len() {
        // A hard newline always breaks.
        let hard = p.text[line_start..]
            .find('\n')
            .map(|i| line_start + i)
            .unwrap_or(p.text.len());
        let segment = &p.text[line_start..hard];

        if segment.is_empty() {
            emit(line_start..line_start, 0, dx(count));
            count += 1;
            line_start = hard + 1;
            continue;
        }

        let mut seg_start = line_start;
        loop {
            let rest = &p.text[seg_start..hard];
            if rest.is_empty() {
                break;
            }
            let w = measure_styled(p, seg_start..hard, measure);
            let width = avail(count);
            if w <= width {
                emit(seg_start..hard, w, dx(count));
                count += 1;
                break;
            }
            // Doesn't fit: find the break point, honouring every style
            // change between here and the boundary.
            let (fit_abs, fit_w) = fit_styled_prefix(p, seg_start..hard, width, measure);
            let mut brk = fit_abs.max(seg_start + 1);
            if word_wrap {
                if let Some(sp) = p.text[seg_start..brk].rfind([' ', '\t']) {
                    if sp > 0 {
                        brk = seg_start + sp;
                    }
                }
            }
            while brk < p.text.len() && !p.text.is_char_boundary(brk) {
                brk += 1;
            }
            if brk <= seg_start {
                brk = next_boundary(&p.text, seg_start);
            }
            // Reuse the prefix measurement when word-wrap didn't move
            // the break; re-measure only the shorter line we actually
            // emit. The common case costs nothing extra.
            let lw = if brk == fit_abs {
                fit_w
            } else {
                measure_styled(p, seg_start..brk, measure)
            };
            emit(seg_start..brk, lw, dx(count));
            count += 1;
            // Swallow the space we broke on.
            seg_start = brk;
            while seg_start < hard && p.text.as_bytes()[seg_start] == b' ' {
                seg_start += 1;
            }
        }
        line_start = hard + 1;
    }
    count
}

/// Walk `range` as maximal runs of uniform style, without allocating.
///
/// Gaps between spans are default-styled. This is the one place that
/// knows how to walk a `ParsedText`'s style structure, so measuring and
/// break-point search can't drift apart.
///
/// Callback rather than a returned `Vec` because both callers sit on the
/// wrapping hot path — `measure_styled` runs per candidate line and
/// `fit_styled_prefix` per overflowing line, so an allocation each would
/// be a per-line malloc during layout. `f` returning `Some` stops the
/// walk early, which is what lets the prefix search bail at the run that
/// straddles the width boundary.
fn walk_style_runs<R>(
    p: &ParsedText,
    range: Range<usize>,
    mut f: impl FnMut(Range<usize>, Style) -> Option<R>,
) -> Option<R> {
    let mut cursor = range.start;
    for s in &p.spans {
        if s.range.end <= cursor || s.range.start >= range.end {
            continue;
        }
        if s.range.start > cursor {
            if let Some(r) = f(cursor..s.range.start, Style::default()) {
                return Some(r);
            }
            cursor = s.range.start;
        }
        let end = s.range.end.min(range.end);
        if end > cursor {
            if let Some(r) = f(cursor..end, s.style) {
                return Some(r);
            }
            cursor = end;
        }
    }
    if cursor < range.end {
        return f(cursor..range.end, Style::default());
    }
    None
}

/// Total width of a byte range, measured one style run at a time.
///
/// The per-run granularity is the point: a 200-character message with
/// two bold words costs three measure calls, not two hundred.
fn measure_styled(p: &ParsedText, range: Range<usize>, measure: &dyn TextMeasure) -> u32 {
    let mut total = 0u32;
    walk_style_runs::<()>(p, range, |r, style| {
        total += measure.run_width(&p.text[r], style);
        None
    });
    total
}

/// Largest prefix of `range` fitting in `max_width`, honouring every
/// style change inside it.
///
/// The naive version — measure the whole rest with the style in effect
/// at its *start* — is wrong as soon as a run changes style partway
/// through, which `**bold**` and `` `code` `` both do. Bold and
/// monospace are wider than the base font, so a break point chosen from
/// the start style under-measures and the drawn line overflows its box.
/// Walking runs and only calling the measurer's own prefix search on the
/// single run that straddles the boundary is both correct and no more
/// expensive.
fn fit_styled_prefix(
    p: &ParsedText,
    range: Range<usize>,
    max_width: u32,
    measure: &dyn TextMeasure,
) -> (usize, u32) {
    let mut total = 0u32;
    let mut cursor = range.start;
    let straddled = walk_style_runs(p, range, |r, style| {
        let seg = &p.text[r.clone()];
        let w = measure.run_width(seg, style);
        if total + w <= max_width {
            total += w;
            cursor = r.end;
            return None;
        }
        // This run crosses the boundary — only it needs the measurer's
        // own prefix search.
        let (fit, fw) = measure.fit_prefix(seg, style, max_width.saturating_sub(total));
        Some((r.start + fit, total + fw))
    });
    straddled.unwrap_or((cursor, total))
}

fn next_boundary(s: &str, from: usize) -> usize {
    let mut i = from + 1;
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i.min(s.len())
}

/// Rough height for a row that has never been laid out.
///
/// Feeds the estimated heights in [`crate::index`]. Deliberately crude —
/// it only has to keep the scrollbar plausible until the row is actually
/// measured, and being fast matters more than being close, since it runs
/// for every row in the buffer on a resize.
///
/// Crude, but not *low*. The distinction matters because while the view
/// is following the bottom its scroll offset is `total - viewport`, and
/// the total is this estimate for every row that has not been laid out
/// yet — which always includes the row that was just appended. An
/// under-estimate therefore puts "the bottom" above the real bottom and
/// clips the newest message off the bottom edge. Over-estimating costs a
/// slightly wrong scrollbar thumb until the row is measured, which is
/// the tradeoff [`crate::index`] already documents.
///
/// Three things it has to get right for that, all of which it once got
/// wrong: a body's *hard newlines* (dividing the byte length by the
/// column count answers "how far would this wrap", not "how many lines
/// does it have", so a five-line message estimated as one); the width
/// the body actually wraps against, which in indent mode is the content
/// width less the gutter; and the vertical padding a code or image block
/// adds. The avatar floor is the same rule `layout_message` applies at
/// the end.
pub fn estimate_height(msg: &Message, params: &LayoutParams, measure: &dyn TextMeasure) -> u32 {
    let metrics = measure.metrics();
    let line_h = metrics.line_height.max(1);
    let body_x = if params.indent {
        params.indent_width
    } else {
        0
    };
    let body_width = params.width.saturating_sub(body_x).max(line_h);
    let cols = (body_width / metrics.space_width.max(1)).max(1) as usize;
    // Hard newlines break unconditionally; each resulting segment then
    // wraps on its own.
    let wrapped = |s: &str| {
        s.split('\n')
            .map(|seg| seg.len().div_ceil(cols).max(1))
            .sum::<usize>()
            .max(1)
    };
    let mut lines = 0usize;
    let mut extra = 0u32;

    // Single-column mode: the stamp and the gutter share the first body
    // line, so that line has that many fewer columns. Counted in bytes,
    // like everything else here, which over-counts a multibyte nick — on
    // the safe side.
    let gutter_cols = msg
        .gutter
        .as_ref()
        .filter(|g| !g.text.is_empty() && !msg.flags.contains(MessageFlags::GROUPED))
        .map(|g| g.text.len() + 1);
    let mut lead_cols = if params.indent {
        0
    } else {
        (params.stamp_width / metrics.space_width.max(1)) as usize + gutter_cols.unwrap_or(0)
    };
    // The same rule `layout_message` applies: a body that cannot start
    // beside the gutter starts on the line below it.
    let beside = matches!(msg.blocks.first(), Some(Block::Text(_))) && lead_cols <= cols / 2;
    if lead_cols > 0 && !beside {
        lines += 1;
        lead_cols = 0;
    }

    for (bi, b) in msg.blocks.iter().enumerate() {
        match b {
            Block::Text(p) if bi == 0 && lead_cols > 0 => {
                // Pad the first segment by the lead, then wrap as usual.
                let (first, rest) = p.text.split_once('\n').unwrap_or((&p.text, ""));
                lines += (first.len() + lead_cols).div_ceil(cols).max(1);
                if p.text.contains('\n') {
                    lines += wrapped(rest);
                }
            }
            Block::Text(p) => lines += wrapped(&p.text),
            Block::Code { text, .. } => {
                // Code never wraps, so it is exactly its own line count —
                // but it is boxed, and the box has padding above and
                // below.
                lines += text.split('\n').count().max(1);
                extra += params.block_padding * 2;
            }
            Block::Quote { content, depth } => {
                let qx = body_x + params.quote_indent * u32::from(*depth).max(1);
                let qw = params.width.saturating_sub(qx).max(line_h);
                let qcols = (qw / metrics.space_width.max(1)).max(1) as usize;
                lines += content
                    .text
                    .split('\n')
                    .map(|seg| seg.len().div_ceil(qcols).max(1))
                    .sum::<usize>()
                    .max(1);
            }
            Block::Image { size, alt, .. } => match size {
                Some(sz) => {
                    let (_, h) = measure.image_size((sz.width, sz.height), body_width);
                    extra += h + params.block_padding * 2;
                }
                None => lines += wrapped(alt),
            },
        }
    }
    let text_h = (lines.max(1) as u32) * line_h + extra;
    // A group head is at least as tall as the icon it draws.
    let avatar_h = if params.indent
        && params.avatar_size > 0
        && !msg.flags.contains(MessageFlags::GROUPED)
        && msg.speaker.as_ref().is_some_and(|s| s.key != 0)
    {
        params.avatar_size
    } else {
        0
    };
    text_h.max(avatar_h)
}

/// [`estimate_height`] for a row, with the line its reply header takes.
pub fn estimate_row_height(
    msg: &Message,
    reply: Option<&Reply>,
    reactions: &[Reaction],
    params: &LayoutParams,
    measure: &dyn TextMeasure,
) -> u32 {
    let line_h = measure.metrics().line_height.max(1);
    // In one column a long nick can put the header on a line of its own,
    // which the estimator, measuring no nick, has to assume.
    let lines = if params.indent { 1 } else { 2 };
    let header = reply.map_or(0, |_| lines * line_h + reply_gap(line_h));
    // Laid out for real: few rows have chips, and wrapping them takes a
    // measure per chip, so the estimate can be exact rather than crude.
    let chips = if reactions.is_empty() {
        0
    } else {
        let x = if params.indent {
            params.indent_width
        } else {
            0
        };
        let width = params.width.saturating_sub(x).max(line_h);
        layout_chips(reactions, x, 0, width, line_h, measure).1
    };
    header + estimate_height(msg, params, measure) + chips
}

/// Whether a message draws in the muted secondary colour.
pub fn is_muted(msg: &Message) -> bool {
    msg.flags.contains(MessageFlags::MUTED) || msg.flags.contains(MessageFlags::DELETED)
}

/// Spans of `p` clipped to `range`, for the view's snapshot pass.
pub fn spans_in(p: &ParsedText, range: Range<usize>) -> Vec<Span> {
    p.spans
        .iter()
        .filter(|s| s.range.start < range.end && s.range.end > range.start)
        .map(|s| Span {
            range: s.range.start.max(range.start)..s.range.end.min(range.end),
            style: s.style,
        })
        .collect()
}
