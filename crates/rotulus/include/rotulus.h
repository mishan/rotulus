/* Rotulus — a GTK4 scrollback view for text-stream chat
 *
 * Copyright (C) 2026 Misha Nasledov <misha@nasledov.com>
 *
 * This library is free software; you can redistribute it and/or modify
 * it under the terms of the GNU Lesser General Public License as
 * published by the Free Software Foundation; either version 2.1 of the
 * License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful, but
 * WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 */

/*
 * Everything here is implemented in Rust; this header declares the C ABI
 * it exports. The comments are gtk-doc, and the annotations in them are
 * what the GObject introspection data is built from.
 */

#ifndef ROTULUS_H
#define ROTULUS_H

#include <gtk/gtk.h>

G_BEGIN_DECLS

/**
 * RotulusView:
 *
 * A widget that shows a long, fast-growing stream of chat messages.
 *
 * Rows are structured, never text with codes in it. The application
 * hands over a row kind, a speaker, and styled runs for the nick column
 * and the body; the view never parses escape codes, so nothing a remote
 * user sends can restyle the transcript.
 *
 * Configuration is GObject properties: "font", "word-wrap", "max-lines",
 * "indent", "max-indent", "separator", "show-timestamps",
 * "timestamp-format", "avatar-size", "group-gap", "markdown",
 * "link-schemes", "autocopy", "copy-timestamps", "activate-links", "zoom",
 * and the read-only "has-selection". The setters below are conveniences
 * over them.
 *
 * The view is a [iface@Gtk.Scrollable], so it goes in a
 * [class@Gtk.ScrolledWindow].
 *
 * Rows are added from C with [method@Rotulus.View.append] and a [struct@Rotulus.Row]
 * built on the stack, or, from C or a language binding, with
 * [method@Rotulus.View.append_message] and a [struct@Rotulus.Message].
 */
#define ROTULUS_TYPE_VIEW (rotulus_view_get_type ())
G_DECLARE_FINAL_TYPE (RotulusView, rotulus_view, ROTULUS, VIEW, GtkWidget)

/**
 * RotulusLoadDirection:
 * @ROTULUS_LOAD_OLDER: the application should load older rows
 * @ROTULUS_LOAD_NEWER: the application should load newer rows
 *
 * Which way a [signal@Rotulus.View::load-more] request pages.
 */
typedef enum {
    ROTULUS_LOAD_OLDER,
    ROTULUS_LOAD_NEWER,
} RotulusLoadDirection;

#define ROTULUS_TYPE_LOAD_DIRECTION (rotulus_load_direction_get_type ())
GType rotulus_load_direction_get_type (void);

/**
 * RotulusView::link-activated:
 * @self: the view
 * @href: the link's target
 *
 * A primary click on a link, when [property@Rotulus.View:activate-links] is on.
 *
 * Returns: %TRUE to say the link was handled; otherwise the view opens it
 *   with the desktop's handler
 */

/**
 * RotulusView::link-menu:
 * @self: the view
 * @href: the link's target
 * @x: where the click was, in widget coordinates
 * @y: where the click was, in widget coordinates
 *
 * A secondary or middle click on a link.
 *
 * Returns: %TRUE to say it was handled; otherwise the view pops its own
 *   Open / Copy menu
 */

/**
 * RotulusView::speaker-activated:
 * @self: the view
 * @key: the speaker's key, as the application gave it
 *
 * A primary click on a speaker's nick or avatar.
 */

/**
 * RotulusView::speaker-menu:
 * @self: the view
 * @key: the speaker's key, as the application gave it
 * @x: where the click was, in widget coordinates
 * @y: where the click was, in widget coordinates
 *
 * A secondary click on a speaker's nick or avatar.
 */

/**
 * RotulusView::load-more:
 * @self: the view
 * @direction: which way to page
 *
 * A click on a %ROTULUS_ROW_LOAD_OLDER or %ROTULUS_ROW_LOAD_NEWER row.
 */

/**
 * RotulusView::media-activated:
 * @self: the view
 * @token: the token the media row was appended with
 *
 * A primary click on an inline image or its placeholder.
 */

/**
 * RotulusView::selection-changed:
 * @self: the view
 *
 * The selection was made, changed or cleared.
 */

/* ---- palette ------------------------------------------------------ *
 *
 * A view draws from a palette of ROTULUS_PAL_COLS colors. Slots 0..31
 * are the legacy mIRC colors, so IRC formatting can address them by
 * number. The rest are roles.
 *
 * A role slot holding a fully transparent color means "follow the
 * system": text takes the widget's CSS color, and the background is
 * left to CSS. A new view starts with the mIRC colors and every role
 * following the system, apart from the selection, the muted and
 * timestamp text, the marker, and the per-nick colors, which have
 * defaults of their own.
 *
 * Keep each one a plain number: the crate's tests parse them. */
/**
 * ROTULUS_PAL_MIRC_COLS:
 *
 * How many palette slots the mIRC colors take, from slot 0, so that IRC
 * formatting can address them by number.
 */
#define ROTULUS_PAL_MIRC_COLS 32
/**
 * ROTULUS_PAL_MARK_FG:
 *
 * The palette slot for the selection's foreground.
 */
#define ROTULUS_PAL_MARK_FG 32
/**
 * ROTULUS_PAL_MARK_BG:
 *
 * The palette slot for the selection's background.
 */
#define ROTULUS_PAL_MARK_BG 33
/**
 * ROTULUS_PAL_FG:
 *
 * The palette slot for the default text color.
 */
#define ROTULUS_PAL_FG 34
/**
 * ROTULUS_PAL_BG:
 *
 * The palette slot for the default background.
 */
#define ROTULUS_PAL_BG 35
/**
 * ROTULUS_PAL_MARKER:
 *
 * The palette slot for the last-read marker line.
 */
#define ROTULUS_PAL_MARKER 36
/**
 * ROTULUS_PAL_MUTED:
 *
 * The palette slot for secondary text: history rows and captions.
 */
#define ROTULUS_PAL_MUTED 37
/**
 * ROTULUS_PAL_TIMESTAMP:
 *
 * The palette slot for the timestamp column.
 */
#define ROTULUS_PAL_TIMESTAMP 38
/**
 * ROTULUS_PAL_NICK:
 *
 * The palette slot for other people's nicks.
 */
#define ROTULUS_PAL_NICK 39
/**
 * ROTULUS_PAL_SELF_NICK:
 *
 * The palette slot for your own nick.
 */
#define ROTULUS_PAL_SELF_NICK 40
/**
 * ROTULUS_PAL_NICK_BRACKET:
 *
 * The palette slot for the brackets around other people's nicks.
 */
#define ROTULUS_PAL_NICK_BRACKET 41
/**
 * ROTULUS_PAL_SELF_BRACKET:
 *
 * The palette slot for the brackets around your own nick.
 */
#define ROTULUS_PAL_SELF_BRACKET 42
/**
 * ROTULUS_PAL_SYSTEM:
 *
 * The palette slot for the tag of a status line.
 */
#define ROTULUS_PAL_SYSTEM 43
/**
 * ROTULUS_PAL_SYSTEM_BRACKET:
 *
 * The palette slot for the brackets around a status line's tag.
 */
#define ROTULUS_PAL_SYSTEM_BRACKET 44
/**
 * ROTULUS_PAL_HIGHLIGHT:
 *
 * The palette slot for the nick on a line that mentions you.
 */
#define ROTULUS_PAL_HIGHLIGHT 45
/**
 * ROTULUS_PAL_RULE:
 *
 * The palette slot for the column divider.
 */
#define ROTULUS_PAL_RULE 46
/**
 * ROTULUS_PAL_NICK_COLOR0:
 *
 * The first of the per-nick color slots.
 */
#define ROTULUS_PAL_NICK_COLOR0 47
/**
 * ROTULUS_PAL_NICK_COLORS:
 *
 * How many per-nick color slots there are, from
 * %ROTULUS_PAL_NICK_COLOR0.
 */
#define ROTULUS_PAL_NICK_COLORS 8
/**
 * ROTULUS_PAL_COLS:
 *
 * How many colors a palette holds: the mIRC colors, the roles, and the
 * per-nick colors. [method@Rotulus.View.set_palette] takes this many.
 */
#define ROTULUS_PAL_COLS 55

/* ---- runs --------------------------------------------------------- *
 *
 * A run is a slice of text with a color and attributes. A row is built
 * from an array of them for the nick column and another for the body.
 *
 * `color` is a palette index, or ROTULUS_COLOR_DEFAULT. `attrs` is a set
 * of ROTULUS_ATTR_* bits; the last three say which of a run's other
 * color fields apply. */

/**
 * ROTULUS_COLOR_DEFAULT:
 *
 * A run color meaning the view's default text color.
 */
#define ROTULUS_COLOR_DEFAULT (-1)
/**
 * ROTULUS_ATTR_NONE:
 *
 * No attributes.
 */
#define ROTULUS_ATTR_NONE 0u
/**
 * ROTULUS_ATTR_BOLD:
 *
 * Bold text.
 */
#define ROTULUS_ATTR_BOLD (1u << 0)
/**
 * ROTULUS_ATTR_ITALIC:
 *
 * Italic text.
 */
#define ROTULUS_ATTR_ITALIC (1u << 1)
/**
 * ROTULUS_ATTR_UNDERLINE:
 *
 * Underlined text.
 */
#define ROTULUS_ATTR_UNDERLINE (1u << 2)
/**
 * ROTULUS_ATTR_STRIKETHROUGH:
 *
 * Struck-through text.
 */
#define ROTULUS_ATTR_STRIKETHROUGH (1u << 3)
/**
 * ROTULUS_ATTR_MONOSPACE:
 *
 * Monospace text, as for code.
 */
#define ROTULUS_ATTR_MONOSPACE (1u << 4)
/**
 * ROTULUS_ATTR_REVERSE:
 *
 * Swap the run's foreground and background.
 */
#define ROTULUS_ATTR_REVERSE (1u << 5)
/**
 * ROTULUS_ATTR_BACKGROUND:
 *
 * The run's `background` is a palette index for its background.
 */
#define ROTULUS_ATTR_BACKGROUND (1u << 8)
/**
 * ROTULUS_ATTR_RGB:
 *
 * The run's `rgb` is its foreground, as 0xRRGGBB, instead of `color`.
 */
#define ROTULUS_ATTR_RGB (1u << 9)
/**
 * ROTULUS_ATTR_BACKGROUND_RGB:
 *
 * The run's `background_rgb` is its background, as 0xRRGGBB.
 */
#define ROTULUS_ATTR_BACKGROUND_RGB (1u << 10)
/**
 * RotulusRun:
 * @text: the text, borrowed for the call
 * @len: its length in bytes, or -1 when it is NUL-terminated
 * @color: a palette index, or %ROTULUS_COLOR_DEFAULT
 * @attrs: ROTULUS_ATTR_* bits
 * @background: a palette index for the background, with
 *   %ROTULUS_ATTR_BACKGROUND
 * @rgb: the foreground as 0xRRGGBB, with %ROTULUS_ATTR_RGB
 * @background_rgb: the background as 0xRRGGBB, with
 *   %ROTULUS_ATTR_BACKGROUND_RGB
 *
 * A slice of text with a color and attributes, for the C row API. Runs
 * are borrowed for the duration of the call; the view copies what it
 * needs, so build them on the stack. The first four fields are all most
 * callers set (ROTULUS_RUN).
 */
typedef struct {
    const char *text;
    int len;
    gint16 color;
    guint16 attrs;
    gint16 background;
    guint32 rgb;
    guint32 background_rgb;
} RotulusRun;

#ifndef __GI_SCANNER__
/* A run with the four common fields; the rest are zero. Designated,
 * so -Wmissing-field-initializers has nothing to say about them. */
#define ROTULUS_RUN(t, l, c, a)                                                \
    ((RotulusRun){ .text = (t), .len = (l), .color = (c), .attrs = (a) })

/* The common "one unstyled run" case. */
#define ROTULUS_RUN_PLAIN(t, l)                                                \
    ROTULUS_RUN ((t), (l), ROTULUS_COLOR_DEFAULT, ROTULUS_ATTR_NONE)
#endif

/* ---- rows --------------------------------------------------------- */

/**
 * RotulusRowKind:
 * @ROTULUS_ROW_MESSAGE: a message someone sent
 * @ROTULUS_ROW_SYSTEM: a notice the application generated; never groups
 *   with its neighbours, even when they share a tag
 * @ROTULUS_ROW_HISTORY: a message loaded from history; doesn't count
 *   against [property@Rotulus.View:max-lines], and is never trimmed to make room for
 *   live rows
 * @ROTULUS_ROW_DIVIDER: a rule with a caption, framing a block of history
 * @ROTULUS_ROW_LOAD_OLDER: a row that asks for older rows when clicked;
 *   see [signal@Rotulus.View::load-more]
 * @ROTULUS_ROW_LOAD_NEWER: the same, for newer rows
 *
 * What a row is.
 */
typedef enum {
    ROTULUS_ROW_MESSAGE,
    ROTULUS_ROW_SYSTEM,
    ROTULUS_ROW_HISTORY,
    ROTULUS_ROW_DIVIDER,
    ROTULUS_ROW_LOAD_OLDER,
    ROTULUS_ROW_LOAD_NEWER,
} RotulusRowKind;

/**
 * ROTULUS_ROW_OUTGOING:
 *
 * The row originated here rather than arriving from elsewhere. It
 * breaks grouping independently of who the speaker is: in a
 * conversation with yourself, both halves have the same speaker, and
 * only direction tells your echo from the copy that came back.
 */
#define ROTULUS_ROW_OUTGOING (1u << 0)
/**
 * ROTULUS_ROW_ACTION:
 *
 * A "/me" action. It never groups with its neighbors.
 */
#define ROTULUS_ROW_ACTION (1u << 1)
/**
 * RotulusSpeaker:
 * @key: the application's identity for the person, or 0 when it has none
 * @nick: the nick, borrowed for the call; may be %NULL
 * @nick_len: its length in bytes, or -1 when it is NUL-terminated
 *
 * Who said it. The view only compares @key, hands it back in the speaker
 * signals, and passes it to the avatar function. A wrong key is worse
 * than none: it attaches the wrong avatar and groups two people's
 * messages.
 */
typedef struct {
    guint64 key;
    const char *nick;
    int nick_len;
} RotulusSpeaker;

#ifndef __GI_SCANNER__
#define ROTULUS_SPEAKER_NONE ((RotulusSpeaker){ 0, NULL, -1 })
#endif

/**
 * RotulusRow:
 * @kind: what the row is
 * @flags: %ROTULUS_ROW_OUTGOING, %ROTULUS_ROW_ACTION
 * @stamp: Unix seconds for the timestamp column; 0 is now
 * @speaker: who said it
 * @gutter: the runs of the nick column; %NULL for none
 * @n_gutter: how many there are
 * @body: the runs of the body
 * @n_body: how many there are
 *
 * A row, for the C row API. Language bindings use [struct@Rotulus.Message].
 */
typedef struct {
    RotulusRowKind kind;
    guint flags;
    gint64 stamp;
    RotulusSpeaker speaker;
    const RotulusRun *gutter;
    int n_gutter;
    const RotulusRun *body;
    int n_body;
} RotulusRow;

/**
 * RotulusMark:
 *
 * An opaque handle to a row. A mark stays valid until the row it names
 * goes (removed, trimmed by [property@Rotulus.View:max-lines], or cleared), and is
 * weak: using a stale one is a safe no-op, and [method@Rotulus.View.remove]
 * returning %FALSE is the intended way to find out.
 *
 * A mark is an id rather than a pointer to anything. It is a boxed type
 * so that language bindings can hold one, but copying it returns the
 * same mark and freeing it does nothing, so C code never needs to.
 */
typedef struct _RotulusMark RotulusMark;

#define ROTULUS_TYPE_MARK (rotulus_mark_get_type ())
GType rotulus_mark_get_type (void);

/* ---- messages ----------------------------------------------------- */

/**
 * RotulusMessage:
 *
 * A row under construction: the introspectable counterpart of
 * [struct@Rotulus.Row]. Build one, hand it to [method@Rotulus.View.append_message], and
 * free it; the view copies what it needs.
 */
typedef struct _RotulusMessage RotulusMessage;

#define ROTULUS_TYPE_MESSAGE (rotulus_message_get_type ())
GType rotulus_message_get_type (void);

/**
 * rotulus_message_new:
 * @kind: what the row is
 *
 * Returns: (transfer full): a new, empty message
 */
RotulusMessage *rotulus_message_new (RotulusRowKind kind);

/**
 * rotulus_message_copy:
 * @self: a message
 *
 * Returns: (transfer full): a copy of @self
 */
RotulusMessage *rotulus_message_copy (const RotulusMessage *self);

/**
 * rotulus_message_free:
 * @self: (transfer full): a message
 */
void rotulus_message_free (RotulusMessage *self);

/**
 * rotulus_message_set_flags:
 * @self: a message
 * @flags: %ROTULUS_ROW_OUTGOING, %ROTULUS_ROW_ACTION
 */
void rotulus_message_set_flags (RotulusMessage *self, guint flags);

/**
 * rotulus_message_set_timestamp:
 * @self: a message
 * @stamp: Unix seconds for the timestamp column; 0, the default, is the
 *   moment it is appended
 */
void rotulus_message_set_timestamp (RotulusMessage *self, gint64 stamp);

/**
 * rotulus_message_set_speaker:
 * @self: a message
 * @key: the application's identity for the person, or 0 when it has none
 * @nick: (nullable): their nick
 *
 * Says who sent it. See [struct@Rotulus.Speaker].
 */
void rotulus_message_set_speaker (RotulusMessage *self, guint64 key,
                                  const char *nick);

/**
 * rotulus_message_add_nick:
 * @self: a message
 * @text: the text
 * @color: a palette index, or %ROTULUS_COLOR_DEFAULT
 * @attrs: ROTULUS_ATTR_* bits
 *
 * Adds a run to the nick column. With none, the column shows the
 * speaker's nick.
 */
void rotulus_message_add_nick (RotulusMessage *self, const char *text,
                               int color, guint attrs);

/**
 * rotulus_message_add_text:
 * @self: a message
 * @text: the text
 * @color: a palette index, or %ROTULUS_COLOR_DEFAULT
 * @attrs: ROTULUS_ATTR_* bits
 *
 * Adds a run to the body.
 */
void rotulus_message_add_text (RotulusMessage *self, const char *text,
                               int color, guint attrs);

/**
 * rotulus_message_add_mirc:
 * @self: a message
 * @text: text carrying IRC formatting codes
 *
 * Adds @text to the body, styled by its IRC formatting codes, with the
 * codes removed. See [func@Rotulus.mirc_parse].
 */
void rotulus_message_add_mirc (RotulusMessage *self, const char *text);

G_DEFINE_AUTOPTR_CLEANUP_FUNC (RotulusMessage, rotulus_message_free)

/* ---- construction and configuration ------------------------------- */

/**
 * rotulus_view_new:
 *
 * Returns: a new [class@Rotulus.View]
 */
GtkWidget *rotulus_view_new (void);

/**
 * rotulus_view_set_palette:
 * @self: a view
 * @palette: (array fixed-size=55): %ROTULUS_PAL_COLS colors
 */
void rotulus_view_set_palette (RotulusView *self, const GdkRGBA *palette);

/**
 * rotulus_view_set_font:
 * @self: a view
 * @font: a Pango font description, such as "Monospace 10"
 */
void rotulus_view_set_font (RotulusView *self, const char *font);

/**
 * rotulus_view_set_word_wrap:
 * @self: a view
 * @word_wrap: whether to wrap at word boundaries
 */
void rotulus_view_set_word_wrap (RotulusView *self, gboolean word_wrap);

/**
 * rotulus_view_set_max_lines:
 * @self: a view
 * @max_lines: rows kept before the oldest are dropped; 0 keeps everything
 */
void rotulus_view_set_max_lines (RotulusView *self, int max_lines);

/**
 * rotulus_view_set_indent:
 * @self: a view
 * @indent: two columns or one
 *
 * With two columns, timestamps and nicks have a column of their own, and
 * bodies are indented past it. With one, the nick starts the body's
 * first line.
 */
void rotulus_view_set_indent (RotulusView *self, gboolean indent);

/**
 * rotulus_view_set_max_indent:
 * @self: a view
 * @max_indent_px: the widest the nick column grows, in pixels
 */
void rotulus_view_set_max_indent (RotulusView *self, int max_indent_px);

/**
 * rotulus_view_set_separator:
 * @self: a view
 * @separator: whether to draw a rule between the columns
 */
void rotulus_view_set_separator (RotulusView *self, gboolean separator);

/**
 * rotulus_view_set_show_timestamps:
 * @self: a view
 * @show: whether to show the timestamp column
 */
void rotulus_view_set_show_timestamps (RotulusView *self, gboolean show);

/**
 * rotulus_view_set_timestamp_format:
 * @self: a view
 * @format: (nullable): a strftime(3) format; %NULL or "" restores the
 *   default, "[%H:%M:%S] "
 */
void rotulus_view_set_timestamp_format (RotulusView *self,
                                        const char *format);

/**
 * rotulus_view_set_avatar_size:
 * @self: a view
 * @px: the edge of the avatar beside a speaker, in pixels; 0 hides
 *   avatars
 *
 * Only the first row of a group draws one.
 */
void rotulus_view_set_avatar_size (RotulusView *self, int px);
/**
 * ROTULUS_AVATAR_SIZE_DEFAULT:
 *
 * The default [property@Rotulus.View:avatar-size], in pixels.
 */
#define ROTULUS_AVATAR_SIZE_DEFAULT 32
/**
 * rotulus_view_set_group_gap:
 * @self: a view
 * @secs: seconds between one speaker's messages that start a new group;
 *   0 turns grouping off
 */
void rotulus_view_set_group_gap (RotulusView *self, int secs);
/**
 * ROTULUS_GROUP_GAP_DEFAULT:
 *
 * The default [property@Rotulus.View:group-gap], in seconds.
 */
#define ROTULUS_GROUP_GAP_DEFAULT 300
/**
 * rotulus_view_set_markdown:
 * @self: a view
 * @markdown: whether to render markdown
 *
 * Renders markdown in bodies appended from now on: bold, italic, code,
 * fenced code blocks, quotes, and labeled links to the view's link
 * schemes. Rows already appended keep their rendering.
 */
void rotulus_view_set_markdown (RotulusView *self, gboolean markdown);

/**
 * rotulus_view_set_link_schemes:
 * @self: a view
 * @schemes: (array zero-terminated=1) (nullable): the URL scheme prefixes
 *   that become links, such as "https://" and "mailto:"; %NULL restores
 *   the default set
 */
void rotulus_view_set_link_schemes (RotulusView *self,
                                    const char *const *schemes);

/**
 * rotulus_view_set_autocopy:
 * @self: a view
 * @autocopy: whether to copy to the clipboards when a drag-select ends
 */
void rotulus_view_set_autocopy (RotulusView *self, gboolean autocopy);

/**
 * rotulus_view_set_copy_timestamps:
 * @self: a view
 * @copy: whether to prefix each copied row with its timestamp
 */
void rotulus_view_set_copy_timestamps (RotulusView *self, gboolean copy);

/**
 * rotulus_view_set_activate_links:
 * @self: a view
 * @activate: whether a primary click opens a link (the default)
 *
 * Off, a primary click on a link does nothing special; the link menu
 * works either way.
 */
void rotulus_view_set_activate_links (RotulusView *self, gboolean activate);

/**
 * rotulus_view_set_zoom:
 * @self: a view
 * @zoom: the text scale, where 1.0 is the font's own size
 */
void rotulus_view_set_zoom (RotulusView *self, double zoom);

/**
 * RotulusAvatarFunc:
 * @view: the view
 * @key: a speaker's key
 * @user_data: the data passed to [method@Rotulus.View.set_avatar_func]
 *
 * Resolves a speaker's key to the image in their avatar slot. Called on
 * every draw, so an animated avatar animates; cache anything expensive.
 * The function must not change the view.
 *
 * Returns: (transfer full) (nullable): the avatar, or %NULL for none
 */
typedef GdkPaintable *(*RotulusAvatarFunc) (RotulusView *view, guint64 key,
                                            gpointer user_data);

/**
 * rotulus_view_set_avatar_func:
 * @self: a view
 * @func: (nullable) (scope notified) (closure user_data) (destroy destroy):
 *   the avatar function, or %NULL for none
 * @user_data: data for @func
 * @destroy: called on @user_data when the view lets go of @func
 */
void rotulus_view_set_avatar_func (RotulusView *self, RotulusAvatarFunc func,
                                   gpointer user_data, GDestroyNotify destroy);

/**
 * rotulus_view_get_vadjustment:
 * @self: a view
 *
 * The vertical adjustment, for a [class@Gtk.Scrollbar] beside the view, created
 * if the view has none.
 *
 * Returns: (transfer none): the adjustment
 */
GtkAdjustment *rotulus_view_get_vadjustment (RotulusView *self);

/* ---- content ------------------------------------------------------ */

/**
 * rotulus_view_append: (skip)
 * @self: a view
 * @row: the row
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_append (RotulusView *self, const RotulusRow *row);

/**
 * rotulus_view_insert_before: (skip)
 * @self: a view
 * @anchor: (nullable): the row to insert before; %NULL inserts at the top
 * @row: the row
 *
 * What is on screen stays where it is, so backfilling history doesn't
 * move what the user is reading. Insert rows in chronological order:
 * each lands directly before the anchor.
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_insert_before (RotulusView *self,
                                         RotulusMark *anchor,
                                         const RotulusRow *row);

/**
 * rotulus_view_replace: (skip)
 * @self: a view
 * @mark: the row to replace
 * @row: its new content
 *
 * Swaps the content of a row, keeping its place and the mark: an edit, a
 * redaction, a streamed reply growing.
 *
 * Returns: %FALSE if the row is gone
 */
gboolean rotulus_view_replace (RotulusView *self, RotulusMark *mark,
                               const RotulusRow *row);

/**
 * rotulus_view_append_message:
 * @self: a view
 * @message: the row
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_append_message (RotulusView *self,
                                          const RotulusMessage *message);

/**
 * rotulus_view_insert_message_before:
 * @self: a view
 * @anchor: (nullable): the row to insert before; %NULL inserts at the top
 * @message: the row
 *
 * As [method@Rotulus.View.insert_before].
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_insert_message_before (RotulusView *self,
                                                 RotulusMark *anchor,
                                                 const RotulusMessage *message);

/**
 * rotulus_view_replace_message:
 * @self: a view
 * @mark: the row to replace
 * @message: its new content
 *
 * As [method@Rotulus.View.replace].
 *
 * Returns: %FALSE if the row is gone
 */
gboolean rotulus_view_replace_message (RotulusView *self, RotulusMark *mark,
                                       const RotulusMessage *message);

/**
 * rotulus_view_remove:
 * @self: a view
 * @mark: the row to remove
 *
 * Returns: %FALSE if the row was already gone, which is not an error
 */
gboolean rotulus_view_remove (RotulusView *self, RotulusMark *mark);

/**
 * rotulus_view_append_text:
 * @self: a view
 * @text: the text
 * @len: its length in bytes, or -1 when it is NUL-terminated
 * @stamp: Unix seconds for the timestamp column; 0 is now
 *
 * Appends a plain row with no nick column.
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_append_text (RotulusView *self, const char *text,
                                       int len, gint64 stamp);

/**
 * rotulus_view_clear:
 * @self: a view
 *
 * Drops every row. Every mark goes stale.
 */
void rotulus_view_clear (RotulusView *self);

/**
 * rotulus_view_get_last:
 * @self: a view
 *
 * Returns: (transfer none) (nullable): the newest row, or %NULL when the
 *   view is empty
 */
RotulusMark *rotulus_view_get_last (RotulusView *self);

/**
 * rotulus_view_set_marker:
 * @self: a view
 * @mark: (nullable): the row to draw the last-read marker under; %NULL
 *   removes it
 *
 * The marker goes with its row rather than moving to a neighbour.
 */
void rotulus_view_set_marker (RotulusView *self, RotulusMark *mark);

/* ---- inline media ------------------------------------------------- *
 *
 * A media row shows `alt` as text until an image arrives, then the
 * image. `token` identifies it to the application: media-activated
 * reports it, and rotulus_view_media_mark finds the row by it, which is
 * what an asynchronous decode should hold — the row may be trimmed
 * while it runs. */

/**
 * RotulusFrame:
 * @texture: the frame
 * @delay_ms: how long it shows
 *
 * One frame of an animation.
 */
typedef struct {
    GdkTexture *texture;
    guint32 delay_ms;
} RotulusFrame;

/**
 * rotulus_view_append_media:
 * @self: a view
 * @texture: (nullable): the image, or %NULL to show @alt until one
 *   arrives
 * @alt: (nullable): the text shown in its place
 * @token: the application's identity for it
 * @stamp: Unix seconds for the timestamp column; 0 is now
 *
 * Returns: (transfer none): a mark for the new row
 */
RotulusMark *rotulus_view_append_media (RotulusView *self,
                                        GdkTexture *texture, const char *alt,
                                        guint token, gint64 stamp);

/**
 * rotulus_view_media_mark:
 * @self: a view
 * @token: a media row's token
 *
 * Returns: (transfer none) (nullable): the row, or %NULL if it is gone
 */
RotulusMark *rotulus_view_media_mark (RotulusView *self, guint token);

/**
 * rotulus_view_media_set_texture:
 * @self: a view
 * @mark: a media row
 * @texture: (nullable): the image; %NULL reverts the row to its alt text
 */
void rotulus_view_media_set_texture (RotulusView *self, RotulusMark *mark,
                                     GdkTexture *texture);

/**
 * rotulus_view_media_set_frames: (skip)
 * @self: a view
 * @mark: a media row
 * @frames: (array length=n_frames): the animation
 * @n_frames: how many frames; 0 reverts the row to its alt text
 *
 * The view takes its own references to the textures.
 */
void rotulus_view_media_set_frames (RotulusView *self, RotulusMark *mark,
                                    const RotulusFrame *frames,
                                    guint n_frames);

/* ---- search ------------------------------------------------------- */

/**
 * rotulus_view_search:
 * @self: a view
 * @needle: (nullable): what to find; %NULL or "" clears the search
 * @case_sensitive: whether case matters
 * @n_matches: (out) (optional): how many matches there are
 * @current: (out) (optional): which one is current, from 1; 0 when none
 *   is
 *
 * Selects the first match at or below the viewport.
 */
void rotulus_view_search (RotulusView *self, const char *needle,
                          gboolean case_sensitive, guint *n_matches,
                          guint *current);

/**
 * rotulus_view_search_step:
 * @self: a view
 * @dir: forward when positive, back when negative, wrapping
 * @n_matches: (out) (optional): how many matches there are
 * @current: (out) (optional): which one is current, from 1
 */
void rotulus_view_search_step (RotulusView *self, int dir, guint *n_matches,
                               guint *current);

/**
 * rotulus_view_search_clear:
 * @self: a view
 */
void rotulus_view_search_clear (RotulusView *self);

/* ---- IRC formatting ----------------------------------------------- */

/**
 * rotulus_mirc_parse: (skip)
 * @text: text carrying IRC formatting codes
 * @len: its length in bytes, or -1 when it is NUL-terminated
 * @n_runs: (out): how many runs there are
 *
 * Splits text carrying mIRC formatting codes (bold, italic, underline,
 * strikethrough, monospace, reverse, reset, and colors by number or
 * hex) into runs, with the codes removed. The runs point into @text,
 * which must outlive them. Colors 0..15 address the palette's mIRC
 * slots; the extended colors and hex colors are RGB.
 *
 * Returns: (transfer full): the runs; free the array with [func@GLib.free]
 */
RotulusRun *rotulus_mirc_parse (const char *text, int len, int *n_runs);

G_END_DECLS

#endif /* ROTULUS_H */
