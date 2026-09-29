# Rotulus: design

How the widget works, and why it is shaped the way it is.

**Rotulus** is a GTK4 scrollback view for text-stream chat,
LGPL-2.1-or-later, usable from Rust or from C. It knows nothing about any
chat protocol: the application hands it structured rows and answers its
signals, and everything in this document is the widget's own behavior.

The first application to use it is
[GtkHx](https://github.com/mishan/gtkhx), a Hotline client, where it was
written to replace a vendored copy of HexChat's xtext widget. GtkHx
examples below show how an application configures it. The measured
comparison that justified retiring xtext is
[xtext-benchmark.md](xtext-benchmark.md); that comparison cannot be
repeated, because only one backend exists now.

---

## 1. The three layers

```
crates/rotulus/include/rotulus.h   the C ABI. Declarations only — the
                                   symbols are exported from
                                   crates/rotulus/src/ffi.rs.
        │
crates/rotulus         gtk4-rs glib::subclass GtkWidget: measure /
                       size_allocate / snapshot, GtkScrollable, event
                       controllers, GSK render nodes, Pango measuring,
                       properties, signals, GtkAccessibleText.
        │
crates/rotulus-layout  pure layout engine, no widget: message model,
                       markdown, link detection, wrap/measure, height
                       index, scroll anchor, hit test, selection,
                       search. No dependencies at all.

crates/rotulus-mirc    IRC formatting codes to styled runs, for an
                       IRC client to opt into. Pure Rust, on top of
                       rotulus-layout.
```

The header lives in the crate, beside the code that implements it, so the
widget carries its own interface.

Two properties of that interface are worth not giving back. **No
struct-field access:** xtext's callers wrote `GTK_XTEXT(w)->wordwrap`,
`->max_lines`, `->urlcheck_function` and read `->buffer` and `->adj`;
here everything goes through properties and `rotulus_view_set_*`, so the
implementation owes callers behavior rather than layout. **No raw entry
pointers:** a caller that needs to come back to a row — to insert history
before it, replace it, or remove it — holds an opaque `RotulusMark`
backed by a message id, where xtext handed out live `textentry *` into
its internal list. Marks are weak references — `rotulus_view_remove` on a
stale one is a safe no-op returning `FALSE`, and that is the intended way
to find out it went stale.

### `rotulus-layout` has no dependencies

Not gtk4, not glib, **not pango**. Text measurement — the one thing that
genuinely needs a font stack — is abstracted behind the `TextMeasure`
trait in `measure.rs`. The widget supplies a Pango-backed implementation;
the crate's own tests supply `FixedMeasure`, where every character is
exactly N pixels wide, which makes wrap assertions exact and readable
instead of font-dependent and brittle. So the whole engine — wrapping,
height indexing, scroll anchoring, span parsing, selection extraction,
search — runs under `cargo test` on display-less CI. That is coverage
xtext never had.

The trait is also a hedge against xtext's worst performance bug. Its
`find_next_wrap` called a width function **per character**, and that did a
`pango_layout_set_text` + `pango_layout_get_pixel_size` round trip each
time. `TextMeasure`'s unit of work is a *run*, never a character, so an
implementation physically cannot repeat that mistake.

---

## 2. The message model

A row is a value with fields, not a byte string. Under xtext a chat line
was bytes with in-band escapes, and a speaker, a message id, an avatar or
a hit region had nowhere to live — which is why every feature added late had
to be smuggled in as a magic word (`hxmedia:N`) or a magic
non-breaking-space sentinel. `rotulus-layout::message` carries:

- **`MessageKind`** — `Live`, `History { server_message_id }`, `Divider`,
  `LoadMore(direction)`, `System`. `LoadMore` and `Divider` were ordinary
  text rows under xtext whose meaning was recovered by string-matching the
  rendered bytes: GtkHx's history handler compared the clicked word against
  a composed "↑\u{a0}Load\u{a0}older\u{a0}messages" sentinel, non-breaking
  spaces and all, because xtext's tokenizer splits on ASCII space.
- **`Speaker { key, nick, color }`**. `key` is the application's opaque
  identity for the person — GtkHx passes the Hotline user id; an IRC
  client might hash the account name — and `color` an optional
  per-person `0x00RRGGBB`, `None` meaning the view's default nick color.
- **`Block`** — `Text(ParsedText)`, `Code { text, language }`,
  `Quote { content, depth }`, `Image { token, size, alt }`. Adding a kind
  of content is adding a variant plus a measure arm and a snapshot arm.
  Under xtext, inline media needed a discriminator on `textentry`, a
  side-allocated media struct, a parallel render path, and a padding hack
  in the line-count math.
- **`MessageFlags`** — highlight, muted, action, outgoing, deleted, and
  grouped, which the buffer sets and recomputes from a row's neighbors
  rather than taking from the caller.

The image block deliberately holds no `GdkTexture`: the layout engine only
needs the size, and a texture cannot cross into a GTK-free crate. The view
keys its own texture table off `token`. `size` is `None` until the decode
lands, and the block measures as its `alt` text until then.

### Rows and runs: how C hands style over

C describes a row with a `RotulusRow`: a kind (`ROTULUS_ROW_MESSAGE`,
`_SYSTEM`, `_HISTORY`, `_DIVIDER`, `_LOAD_OLDER`, `_LOAD_NEWER`), flags
(`ROTULUS_ROW_OUTGOING`, `_ACTION`), a timestamp, a `RotulusSpeaker`
(key and nick), and two `RotulusRun` arrays — one for the gutter, one for
the body — borrowed for the duration of the call and built on the stack.
`rotulus_view_append`, `_insert_before` (history backfill) and `_replace`
(an edit, a redaction, a streamed reply) all take one. The kind is always
explicit. Under xtext it had to be inferred: a row whose every run was in
the muted history color was history, and the load-older row was
recognized by its text.

A run is `(text, palette index, attrs)`, written with `ROTULUS_RUN`,
plus optional fields that take effect only under their attribute bits: a
background (`ROTULUS_ATTR_BACKGROUND`), an RGB foreground or background
(`_RGB`, `_BACKGROUND_RGB`), and reverse video (`_REVERSE`). Most callers
need only `ROTULUS_RUN`; the optional fields are what
`rotulus_mirc_parse` produces for IRC colors.

### The palette

A view draws from a palette of `ROTULUS_PAL_COLS` colors, set with
`rotulus_view_set_palette`. Slots 0..31 keep their historical mIRC
values, so IRC formatting can address them by number. After them come
named roles — selection, default text, the last-read marker, muted text,
timestamps, nicks and your own nick, their brackets, the tag of a system
line, a mention, the column divider — and then a block of per-nick colors
an application can hash names onto. Roles have names so the gutter —
brackets, nicks, a status tag, a mention — follows the application's
theme, rather than being bare numbers scattered through the code that
builds rows, which is how a color choice ends up undocumented and
unsearchable.

A fully transparent role means "follow the system": the view carries
Adwaita's `.view` class, draws such text in its CSS color, and skips its
background fill so the CSS background shows. That is how an application
theme with no chat colors of its own matches the window around it. A new
view starts that way for every role except the selection, muted and
timestamp text, the marker and the per-nick colors, which need a color
of their own to be visible at all.

`rotulus.h` is the sole definition of that contract. Nothing at the FFI
boundary checks the palette's length — `ffi.rs` reads `PALETTE_COLS`
entries from the C array — so a rotulus test reads the defines out of
`rotulus.h` and fails if the Rust constants disagree with them. The run
attribute bits and row constants are held to the header the same way.

### Speaker identity is the application's identity

`RotulusSpeaker.key` should be the same identity the application's user
list uses, and `0` when it doesn't know — which is the honest answer more
often than it looks. In a *text-stream* protocol a chat line carries a
name, and the id has to come from the protocol when it sends one or from
a lookup by name when it doesn't. In GtkHx the uid comes from the Hotline
chat packet's UID field, or from a nick lookup against the conversation's
membership. That lookup can miss — the user parted, two users share a
name, the "nick" is server prose — and a wrong key is worse than none: it
would attach someone else's avatar and group two people's messages
together. So the key stays 0 and stays a miss.

`Speaker` is a render-time projection of the application's user record,
not a rival model. On a right-click over a nick or avatar the view emits
`speaker-menu (key, x, y)` and stops there; the application answers with
whatever it shows for that user elsewhere. GtkHx pops the same user menu
its user list does, so chat and the user list can't drift into two menus
that have to be kept in step.

### Grouping and the avatar gutter

Consecutive messages from one speaker collapse the nick column, and only
the group head draws an avatar. `Message::group_key()` keys on **both**
the speaker key and the *rendered gutter text*, and each half catches a
case the other misses: the key separates two people who happen to share
a nick, and the rendered nick separates one person before and after a
rename. The key survives a rename, so keying on it alone would group the
messages and the new name would simply never appear — worse than
repeating it, since the change is exactly what the reader needs to see.

System rows never group. They share a gutter (GtkHx's `[hx]` tag, say)
without sharing a speaker, so keying on the drawn nick would collapse
"connecting", "connected", "login ok" into one block under a single tag
and read as one event rather than three. That check is on the *kind*,
not on `speaker.is_none()`: some sources carry no identity at all — a
pre-1.5 Hotline server sends chat with no uid — and those rows are real
messages from a real person that should still group by nick. Actions
(`ROTULUS_ROW_ACTION`) and deleted rows never group either.
`ROTULUS_ROW_OUTGOING` breaks a group independently of the speaker: in a
conversation with yourself both halves have the same speaker, and only
direction tells your echo from the copy that came back.

The gap that breaks a run defaults to five minutes (`group-gap`, in
seconds; 0 turns grouping off) — short enough that a burst collapses
under one name, long enough that coming back to a room shows who is
talking rather than attaching your message to something you said an hour
ago. Continuation rows still *reserve* the gutter width, so a run forming
does not shift the column.

Avatars resolve through the application's avatar function
(`rotulus_view_set_avatar_func`). It should share the user list's rule for
which image a user has — in GtkHx, a GIF avatar wins over the classic
Hotline icon — or chat and the user list will disagree. The paintable is
borrowed only until the draw finishes, and the function is asked on
every draw: animated avatars advance on the application's own frame
timer, so the view asks per draw rather than caching a frame that would
freeze. Rows whose speaker is unknown get no slot at all; there would be
nothing to look up.

---

## 3. Layout, the height index, and scroll anchoring

Each row carries an optional `LayoutCache`: a `LayoutGeneration` key
(width, font, theme, zoom), a pixel height, the `LineBox`es for hit
testing, the gutter width the row naturally wanted, and the avatar slot if
it has one. A width, font, theme or zoom change bumps the generation.
Caches are *not* eagerly rebuilt — they are rebuilt lazily when a row is
next laid out, so a resize costs O(visible) rather than O(scrollback).
That is the single biggest departure from xtext's `gtk_xtext_calc_lines`,
which walked every entry on every width change.

### Chunked prefix sums, not a Fenwick tree

Variable heights need an O(log n) "what is at pixel Y" and a running
total. A Fenwick tree is the textbook answer for prefix sums with point
updates, and it is the wrong shape here: it is indexed from a fixed
origin, and this buffer grows at *both* ends (chat-history backfill
prepends) and shrinks at the front (scrollback trim). Every prepend would
renumber the whole tree.

Instead rows live in fixed-target-size chunks in a `VecDeque`, each chunk
caching its own summed height, with a lazily-repaired running prefix over
the chunks.

- Query pixel→row: binary search the chunk prefixes, then scan within one
  chunk, bounded by the chunk target.
- Append: touch one chunk plus the prefix tail.
- Prepend a history batch: push chunks at the front.
- Trim to `max_lines`: pop chunks off the front.

A chunk that grows past a split threshold after middle inserts is split,
so the within-chunk scan stays bounded.

**Unmeasured rows report an estimate** rather than forcing a measure; the
index records per row whether the height is real or estimated. Rows that
have never been on screen are never shaped. The honest cost is that the
scrollbar's extent is approximate until estimates are replaced — which is
survivable precisely because scroll position is not stored in pixels.

The estimate is allowed to be crude. It is not allowed to be *low*, and
the reason is the one case where an estimate does feed a pixel value:
while the view follows the bottom, the scroll offset is
`total_height − viewport`, and the total counts estimates. An
under-estimate therefore puts "the bottom" above the real bottom and
clips the newest message off the bottom edge. Over-estimating only
misplaces the scrollbar thumb. Three things the estimator has to respect
to stay on the safe side, all of which it originally got wrong: a body's
*hard newlines* (dividing byte length by a column count answers "how far
would this wrap", not "how many lines does it have", so a five-line
message estimated as one — which is why this surfaced as a multi-line
bug); the width the body actually wraps against, which in indent mode is
the content width less the settled gutter; and the padding a code or
image block adds.

Even a careful estimator cannot close the gap, because the engine has no
font stack — a proportional font's space width makes the column count
wildly optimistic, and a bold run is wider than the estimator can know.
So `snapshot` **re-derives the offset after the layout pass and before it
places anything**, and pushes the corrected value into the adjustment.
Each pass measures strictly more rows, so it settles immediately; the
loop bound is belt-and-braces, and the reconfigure is gated on having
actually corrected something so the scrolled window's redraw-on-changed
does not become a loop.

### The anchor

The scroll position is `(row, offset within it, gravity)`, and the
`GtkAdjustment` value is *derived* from it, never the source of truth.
This is the most load-bearing design decision in the engine. A raw pixel
scroll value is only meaningful relative to a particular set of row
heights, and everything interesting that happens to a chat buffer changes
those heights: a resize re-wraps, a zoom rescales, an image finishes
decoding and a 16-pixel placeholder becomes 240 pixels, a history batch
prepends rows above the viewport, a trim drops rows off the top.

Consequences, all of which xtext hand-patched case by case:

- Stick-to-bottom is `gravity == Bottom`, not a flag plus bookkeeping.
- Prepending a history batch cannot make the view jump — the anchor names
  a row, and that row did not move. xtext's insert path bumped
  `pagetop_line`, `last_pixel_pos`, `old_value` and the adjustment by the
  inserted row's subline count to approximate this, and the trim path did
  the mirror-image decrement.
- Resize preserves reading position exactly, even though every height
  changed. A font change under xtext just accepted the jump.
- Height-estimate corrections shift the thumb, never the content. Thumb
  drift is survivable; content jumping is not.

The adjustment's unit is **pixels**, with `page_size = widget height`.
xtext's unit was fractional text lines.

### Rendering

`snapshot` queries the index for the visible range, ensures each visible
row's `LayoutCache`, and emits GSK nodes — shaped layouts for text,
paintables or texture nodes for images and avatars, color nodes behind
the text for selection and highlight bands. No `append_cairo()`.

---

## 4. Markdown

Markdown is the inline formatting vocabulary. It produces the same spans
the layout engine already consumes, so it is a front-end on the parser,
not a rendering path. It is the view's `markdown` property, on by
default.

**Supported subset — inline, plus two block constructs.** Chat lines are
not documents:

| Syntax | Renders as |
|---|---|
| `**bold**` | bold |
| `*italic*` / `_italic_` | italic |
| `` `code` `` | monospace, background-tinted |
| `~~strike~~` | strikethrough |
| `[label](url)` | link (see security below) |
| ` ```lang ` fenced block | code block |
| `> quote` | quoted block, at line start |

**Deliberately not supported:** headings (`#` opens far too many ordinary
chat lines), images (`![]()` — inline media belongs to the application's
own pipeline, which may validate what it shows, and must not be
bypassable by an arbitrary URL), tables, raw HTML, reference links,
footnotes, thematic breaks (`---` is common in plain prose), setext
headings, and autolinking, which is `rotulus-layout::linkify`'s — see
"Link detection" below.

Backslash escapes any construct char, code spans suppress all other
parsing inside them, unmatched delimiters render literally (never eat a
lone asterisk), and nesting is depth-capped so a line of five thousand
asterisks cannot recurse the parser into the stack guard.

**Parser choice: a hand-written inline scanner, not `pulldown-cmark`.**
`pulldown-cmark` is the obvious pick (pure Rust, MIT, well-tested) and was
considered. Passed over for three reasons: it has no inline-only mode, so
we would be filtering a block-level event stream and fighting CommonMark's
block rules to suppress exactly the constructs listed above; its event
stream would still need converting into byte-ranged spans, which is most
of the work; and a scanner for a handful of constructs is exhaustively
unit-testable and predictable on the pathological input chat actually
produces.

**Send side — what goes on the wire is the literal text.** Markdown here
is a way of rendering plain text, so it needs no capability negotiation,
no wire change and no server cooperation: `**bold**` is sent as
`**bold**`, clients that render markdown show bold, and everyone else sees
asterisks. This is how Slack, Discord and IRC clients have always behaved,
and it is why markdown suits protocols that carry only plain text, like
Hotline in GtkHx, where a custom binary styling extension would not.

**Receive side — the honest tradeoff.** Rendering markdown on *incoming*
text means a message typed as literal `*emphasis*` on a client that knows
nothing of markdown (in GtkHx's case, a 1997 Mac client) renders as
italics. Mitigations, in order: the subset is conservative, unmatched
delimiters stay literal, and the property turns rendering off entirely,
so an application can offer the choice to people who would rather see
exactly what was typed. The property affects messages appended after it
changes; rows already in a buffer keep the rendering they were built
with, because re-parsing scrollback would mean holding every row's
original source text alive forever — a permanent memory cost for a
setting nobody flips twice.

**Security.** `[label](url)` is a phishing vector: the visible text can
lie about the destination. The parser allows exactly the view's link
schemes — the ones it autolinks, so the two cannot disagree. Anything
else — `javascript:`, `data:`, `file:`, an unrecognized scheme — makes the
whole construct render as literal text, delimiters included, so the user
sees exactly what was typed rather than a link they cannot inspect. A
labeled link never opens on a click either; see "Signals" below. Fenced
code is inert.

Three decisions worth recording. *The gutter is never parsed* — a nick
containing asterisks is a nick. *Only a stylistically uniform body is
parsed*: a body assembled from several differently-styled runs is chrome
the caller styled deliberately (a divider, a tagged status line) and
re-parsing it would fight that; in practice a chat message's body is a
single run, plain for live chat and muted for history. *The row's own
color is laid **under** the parse* — the renderer treats a gap between
spans as *default* style, not "whatever the row was", so without this a
muted history line would come back with only its bold words muted and
everything else at full contrast. There is a test.

Fenced code is deliberately not autolinked either: a URL inside a code
fence is being *shown*, not offered.

### Two gotchas worth keeping

**Code needs a box, not a font.** The first cut relied on the `CODE`
attribute alone, which sets the Pango font family to Monospace — and a
chat font is often *already* monospace (GtkHx's is), so `` `code` ``
rendered identically to code with the backticks quietly deleted. Strictly
worse than not parsing it. Inline code now gets a tint behind it and
fenced blocks a tinted, outlined rounded box, both derived from the text
foreground at low alpha so they read on light and dark without a second
color to keep in step. The block's box is computed from its *laid-out
line boxes* rather than from separate geometry, so it cannot land anywhere
other than under the code it belongs to.

**A one-line fence is a code block.** ```` ```like this``` ```` is how
people actually type one in a chat box, because chat boxes send on Enter.
The line scanner read it as an *opening* fence, made the rest of the line
the "language", and searched for a close that never came — yielding an
empty block, i.e. a blank row where the text should have been.

An unterminated fence runs to the end of the body. The alternative
(treating it as literal) means a message someone is mid-way through typing
flickers between two renderings.

### Composing

The view renders on display only; the input box is the application's.
What the crate offers it is `markdown::scan_delims`, a separate,
shallower scanner for syntax tinting in the input, so the user can see
what will render. GtkHx uses it to tint the delimiters in its message
entry, and binds `Ctrl+B` / `Ctrl+I` / `Ctrl+Shift+C` to wrap the
selection (or insert the delimiter pair).

It is not the renderer because it needs ranges in the *source*, while
the renderer reports ranges in the rendered text with the delimiters
removed. Being wrong in the compose box tints a character that will not
render, on text the user can see and is still editing; being wrong in the
renderer would change what a message *says*. A test pins the one thing
they must agree on: whether a delimiter is live at all.

---

## 5. Interaction

### Every controller callback captures the view weakly

`constructed` installs three groups of gestures, shortcuts and motion
handlers, and the view owns all of them. A closure that captures a strong
clone of the view therefore closes a cycle — view owns controller owns
closure owns view — and the view can never reach refcount zero. It is not a
subtle leak: the whole message buffer, the media table and the Pango
measurer go with it, once per chat window ever opened.

That is what the widget once did, at eighteen sites. Seventeen were
installed during construction (the zoom shortcut loop alone accounted for
five, one per accelerator), and the eighteenth appeared the first time a
scroll adjustment was attached. So a freshly built view had a refcount of
19 before anything had happened to it.

Every one of them is now `self.downgrade()` plus an `upgrade()` at the top
of the closure, the usual gtk-rs idiom for exactly this reason. The
upgrade cannot fail in practice — a controller does not outlive the
widget that owns it, so the closure cannot run after the widget is gone —
but writing the fallback is cheaper than arguing that it is unreachable.

**The reason this survived so long is worth more than the bug.** The smoke
test in `rotulus` asserts exactly this refcount, and had done since the
widget landed. But GTK 4 has no headless backend, so the test began with an
`if gtk4::init().is_err() { return; }` — and CI had no display. It reported
success without executing a line of itself, on every run, for the entire
life of the defect. A missing display is now a failure rather than an
early return, so the widget's tests have to run under a display: a
private one, not your desktop session, e.g. `xvfb-run -a cargo test -p
rotulus`.

### Selection

Drag-select, double-click word select, triple-click line select, and
autocopy (copy to the primary selection and the clipboard on drag-end,
the `autocopy` property; `copy-timestamps` prefixes each copied row with
its timestamp). Ctrl+C copies the selection, and a right-click away from
a link or nick offers Copy and Select All. Every change is reported by
`selection-changed`, and `has-selection` notifies when it flips.
Selection across an image block contributes the block's alt text to the
copied string rather than xtext's all-or-nothing behavior.

**Two controllers on one widget have no GTK-guaranteed ordering.**
Double- and triple-click were broken on arrival: the multi-click handler
set a word/row selection on press, and the drag gesture's `drag-begin`
collapsed the selection to a caret on the same press. Whichever ran second
won, and a code comment claiming drag "fires first" was an assumption, not
a fact. Fixed by removing the conflict rather than sequencing it:
`drag-begin` now only records the press point, and the collapsed selection
is installed on first *motion* — the moment it means something.
Click-to-dismiss consequently keys on "the pointer never moved" instead of
"the selection is empty".

**Auto-scroll while dragging past the viewport edge** is a
`GtkTickCallback` driven from the last recorded drag position. xtext's
scroll timers read a stale `select_end_y` rather than the live device
position, because GTK 4 has no synchronous "where is the pointer"
accessor; storing the position from the drag handler and consuming it from
a tick callback is the real answer. The rate is frame-time based, so it
scrolls at the same speed on a 60 Hz and a 144 Hz display.

### Search

**Search is O(scrollback), on purpose.** `ChatBuffer::search` walks the
*model*, not the layout, so a match in a row that has never been laid out
is still found — which is the entire point, since the reason to search is
to reach the part of the scrollback you have not scrolled to. The cost
belongs in a short debounce in the application's find bar rather than in
an index, because an index would have to be maintained across append,
prepend, trim and replace for a feature used seconds at a time. Matching
is literal, not regex, optionally case-sensitive: the needle is what the
user typed, so there is no metacharacter vocabulary to explain and no
pathological backtracking to defend against.

The find bar is the application's; the view supplies
`rotulus_view_search`, `_search_step` (forward or back, wrapping) and
`_search_clear`, with the match count and the current match's position
as out-parameters for the bar's readout. Search starts at the first match
at or below the viewport. GtkHx's bar is the model to copy: Ctrl+F opens
and focuses, selecting the existing query so typing replaces it; pressed
*again* while the entry already has focus and a query, it advances to the
next match instead — that is the "hit Ctrl+F, type, keep hitting Ctrl+F"
flow, and gating it on the entry already being focused is what keeps the
reopen-and-retype case intact. Ctrl+G / Ctrl+Shift+G and F3 / Shift+F3
both step, because which pair is muscle memory depends on where someone
came from.

Matches are drawn in fixed colors — Adwaita yellow `#f6d32d` on black
for every hit, orange `#ff7800` on white for the current one — rather
than palette roles, so a find bar doesn't widen the palette contract in
`rotulus.h` that every application theme has to answer. They match
GtkHx's other find bars, so one application doesn't highlight two ways.

### Keyboard paging, and why a global shortcut cannot do it

A chat application usually makes the view unfocusable
(`gtk_widget_set_can_focus (view, FALSE)`) so the message input keeps
focus. The input is typically a `GtkTextView`, which binds PgUp/PgDn to
its own cursor movement and swallows the key. That is why PgUp/PgDn never
worked over xtext in GtkHx.

A global-scope `GtkShortcut` would not help: global shortcuts run *after*
normal propagation, so the TextView still wins. The binding therefore
lives on a capture-phase key controller installed on the widget's **root**
(the same one Ctrl+C uses, for the same reason), which runs before the
focus path.

The steal is narrow on purpose. Unmodified paging applies only when focus
is in a text-entry widget — a message input or a subject entry, where
paging means nothing — so a list beside the view, such as a user list,
keeps its own page-by-page navigation. Shift+PgUp/PgDn, the long-standing
IRC binding for "scroll the log", is unambiguous anywhere and bypasses the
focus check. Ctrl+Home/End jump to the top of the scrollback and back to
following the tail. A view that is not mapped (a background tab, a closed
private chat) must not eat the window's keys, so the handler checks that
first. A page scroll keeps one line of overlap, so the line being read
survives the jump.

### Zoom

xtext had none: the only way to change chat text size in GtkHx was the
font setting, a modal round trip that touched nothing but the glyphs.

Zoom is a *view* scale, not a font-size change — text, the timestamp
gutter and nick column, inline media, avatars, indent and padding all
scale together, so the layout stays proportionate. It is stored in
per-mille and steps through a fixed ladder from 50% to 400%, the one
browsers and terminals have taught people. Bindings are `Ctrl` + `+` /
`-` / `0` and `Ctrl` + scroll wheel. It is distinct from the desktop-wide
text scaling factor.

Zoom is otherwise silent: text changes size and nothing says by how much
or how to get back to 100%. A badge showing the percentage holds for
around a second and then fades, drawn inside the widget's own snapshot
rather than as an overlay widget — an application may pack the view as a
bare child beside a `GtkScrollbar`, as GtkHx does, and a `GtkOverlay`
would mean restructuring every container that holds one for a label that
shows for a second.

Zoom changes every height in the buffer, which is exactly the case the
scroll anchor already handles: the anchored row stays put and the content
grows around it. Without anchor-based scrolling, zoom would fling the
viewport — a concrete second payoff from that design decision, and why
zoom landed cheaply here where it would have been painful to retrofit into
xtext.

**The view doesn't persist zoom.** It is per view, exposed as the `zoom`
property (1.0 is the font's own size), which notifies on every change; an
application that wants zoom to survive a restart saves and restores it.

### The indent separator

A drag pins the gutter explicitly, and nothing but an explicit unpin
releases it — not a buffer clear, not a stamp-width change. xtext left the
gutter's auto-grow enabled after a drag, so a long nick could silently
undo a narrowing the user had just made by hand; a widening only stuck
because it happened to exceed the auto-indent cap, which switched the auto
path off as a side effect. The grab tolerance is a few pixels rather than
xtext's ±1, which is unhittable on a fractional-scale display, and the
drawn rule and the hit test share one `separator_x()` so they cannot drift
apart. The drag is clamped to a band of the viewport rather than to
`max-indent`: that cap is about how far the gutter may grow unattended,
and the point of the drag is to overrule it.

---

## 6. Structured data, not escape codes

The view never interprets escape codes in text. A row arrives as fields —
a kind, a speaker, styled runs — and the text in a run is characters,
nothing more. That is a security property as much as a design one.

xtext took rows as byte strings with in-band mIRC escapes (`\003NN` for a
color, and so on) and interpreted them on every render. That had two
costs. The first is that any text arriving *off the wire* and appended
through that path could restyle the transcript: a remote user sending the
bytes could set colors in your chat, fake a status line, or forge another
user's nick brackets. An application had to sanitize every received
string before it reached the view, and one it missed was a hole. With
structured rows there is nothing to escape from: control bytes in a
message are drawn as characters, and nothing a remote user sends can
restyle anything.

The second cost is that structure the application already had — where a
nick ends, which rows are history, which row is the load-older button —
had to be flattened into a presentation format and then parsed back out.
GtkHx did exactly that: its chat and private-message code scanned its own
escape output for the bytes that closed a nick bracket. That is a data
structure round-tripped through a presentation format. Removing the
escapes without giving the API somewhere to put the structure would have
meant inventing a *different* string convention to re-parse — the same
mistake with fresh bytes. The row and run API is that somewhere.

In GtkHx's case the escapes were never protocol. The Hotline wire format
carries plain text, and a user's color is a separate attribute on the
user record; every escape in its scrollback had been written by GtkHx
itself.

### Where IRC formatting fits

IRC *is* different: mIRC codes are real content there, and an IRC client
wants to show them. That is what `rotulus-mirc` is for. It converts text
carrying the codes into styled runs, and the application decides which
messages go through it — see "IRC formatting" below. Nothing in the view
itself interprets them, so opting in is per message and explicit, and an
application that never calls it cannot be restyled by them.

---

## 7. What an application hooks

### Signals

Clicks reach the application as typed signals, never as a word to match:

| Signal | When | Unhandled |
|---|---|---|
| `link-activated (href) → handled` | primary click on a link, when `activate-links` is on | the view opens it with `GtkUriLauncher` |
| `link-menu (href, x, y) → handled` | secondary or middle click on a link | the view pops its own Open / Copy menu |
| `speaker-activated (key)` | primary click on a nick or avatar | nothing |
| `speaker-menu (key, x, y)` | secondary click on a nick or avatar | nothing |
| `load-more (direction)` | click anywhere on a load-more row | nothing |
| `media-activated (token)` | primary click on an image or its placeholder | nothing |
| `selection-changed` | the selection is made, extended or cleared | — |

They replaced xtext's `word-click`, which handed every click over as the
whitespace-delimited word under it and left the application to demux by
string: GtkHx's URL menu matched URL-shaped words, its history handler
matched a composed "↑ Load older messages" sentinel joined with
non-breaking spaces (so the tokenizer kept it one word), and its inline
media embedded `hxmedia:N` in the placeholder to be parsed back out.
None of that is needed with typed signals. GtkHx, for example, handles
`link-activated` to connect when a `hotline://` link is clicked and lets
every other scheme fall through to the desktop.

A primary click on a link opens it, as it does everywhere else on the
desktop — unless the view's `activate-links` property is off, which an
application can expose as a setting (GtkHx does). Off, a link behaves
like any text to a primary click and keeps its right-click menu.

A link whose visible text isn't its address — a markdown `[label](url)` —
never opens on a click. It pops the link menu instead, headed by the real
URL, so a label can't take someone somewhere they didn't see; and every
link shows its address in a tooltip. An application that handles
`link-menu` itself should keep showing the address. In GtkHx that matters
twice over, since a `hotline://` link connects to a server.

(A GLib detail worth not rediscovering: signal names must be canonical —
hyphens — because glib-rs's `Signal::builder` *panics* otherwise, and a
panic there is an abort, since it unwinds out of `class_init` across the
FFI.)

### Properties

Every setting is a GObject property, so a binding or `g_object_set` works
as well as the C setters, and each change notifies: `font`, `word-wrap`,
`max-lines`, `indent`, `max-indent`, `separator`, `show-timestamps`,
`timestamp-format`, `avatar-size`, `group-gap`, `markdown`,
`link-schemes`, `autocopy`, `copy-timestamps`, `activate-links`, `zoom`,
and the read-only `has-selection`. They are per view: a library cannot
have process-wide settings, so an application with preferences applies
them to each view it builds and re-applies them on a change. The view is
also a `GtkScrollable`, so it works inside a `GtkScrolledWindow`, or
beside a `GtkScrollbar` on `rotulus_view_get_vadjustment`.

### Link detection

`rotulus-layout::linkify` finds links: a scheme or a bare `www.` prefix
at the start of a word, running to whitespace or a closing delimiter, with
trailing sentence punctuation dropped unless the URL opened the bracket
itself (a Wikipedia link). It answers every "is this a link" question the
view asks: autolinking, the markdown allowlist, the link under the
pointer. Bare email addresses are links too, opening as `mailto:`. The
scheme list is the application's (`link-schemes`); the default is the set
any chat client agrees on — `http`, `https`, `ftp`, `ftps`, `irc`, `ircs`,
`mailto`, `magnet`, `git`, `ssh`, `sftp` — and GtkHx adds `hotline://`.
An application that detects links elsewhere can use the same `Linkifier`
on the same list, as GtkHx does for its news views, so the two cannot
disagree about what a link is.

### Avatars

`rotulus_view_set_avatar_func` installs a function from a speaker's key to
a `GdkPaintable`, asked on every draw so an animated avatar animates;
anything expensive belongs in the application's cache. The function must
not change the view. `avatar-size` sets the edge in pixels, and 0 hides
avatars.

### Inline media

`rotulus_view_append_media` adds a row that shows its `alt` text until an
image arrives, then the image. Its `token` identifies it to the
application: `media-activated` reports it, and `rotulus_view_media_mark`
finds the row by it, which is what an asynchronous decode should hold,
since the row may be trimmed while the decode runs.
`rotulus_view_media_set_texture` supplies a still image and
`rotulus_view_media_set_frames` an animation; either reverts the row to
its alt text when given nothing. An animation only advances while it is
on screen.

### One column

With `indent` off a row is a single column: the timestamp, then the nick,
then the body on the same line, wrapping to the left edge. A body that
cannot start beside the nick — a code block, a quote, an image, or one
left with less than half the width — starts on the line below it. The
height estimate follows the same rule, so it stays on the safe side. Log
viewers and IRC-style layouts want this; the mode used to drop the nick
altogether.

### The last-read marker

`rotulus_view_set_marker` draws a rule under a row, in
`ROTULUS_PAL_MARKER`. It goes with its row — removed, trimmed or cleared —
rather than moving to a neighbour, since a marker in the wrong place
claims something was read that wasn't.

### Replacing a row

`rotulus_view_replace` swaps a row's content in place, keeping its id and
its place: what an edit, a redaction or a streamed reply growing a token
at a time needs. The scroll anchor absorbs any change in height. A search
or a selection is dropped, since it would point at different bytes.

### Accessibility

The view's accessible role is `log`. Built against GTK 4.14 or newer (the
`v4_14` cargo feature, which an application's build turns on when it finds
one), it implements `GtkAccessibleText`: a screen reader reads the
transcript as one text, a row per line, as timestamp, nick and message. A
grouped row still names its speaker there, because "who said this" is the
first thing a listener needs and the visual cue that stands in for it
isn't available to them. The text is built the first time an assistive
technology asks and kept in step from then on, row by row: an append, a
page of history, a removal, a replace and the trim at the scrollback cap
each report only the rows they touched. Only what changes every row at
once — a clear, a new timestamp format — is reported as the whole text
going and coming back. Below 4.14 the view exposes its role only.

### IRC formatting

`rotulus-mirc` converts mIRC formatting codes — bold, italic, underline,
strikethrough, monospace, reverse, reset, colors by number or hex — into
styled runs; C reaches it as `rotulus_mirc_parse`, which returns runs
pointing into the caller's text (the `mirc` cargo feature of `rotulus`,
on by default). The view never interprets the codes itself, so it is the
application that decides which messages may carry them. Colors 0–15
address the palette's mIRC slots, so a theme can adjust them; the
extended colors and hex are RGB. GtkHx doesn't use it: Hotline has no
in-band styling.

### Translations

The widget's strings (its two context menus) are in the `rotulus` gettext
domain, with their catalogs in `crates/rotulus/po/`. An application that
ships the compiled catalog binds the domain at startup,
`bindtextdomain ("rotulus", localedir)`; one that doesn't gets English.

---

## 8. Measured

### Scale

`cargo bench -p rotulus-layout`, headless against the fixed-width measurer,
at 2,000, 20,000 and 200,000 rows:

| Benchmark | 2,000 | 20,000 | 200,000 |
|---|---|---|---|
| ingest (whole scrollback) | 0.2 ms | 5 ms | 41 ms |
| first paint after the burst | 20 µs | 83 µs | 1.9 ms |
| relayout after a font change | 4.4 µs | 8.3 µs | 49 µs |
| scroll walk, 120 frames | 0.31 ms | 0.48 ms | 4.3 ms |
| one message at the scrollback cap | 1.8 µs | 3.8 µs | 24 µs |

Layout stays O(visible): nothing off screen is shaped. The bookkeeping
around it does not — the height index repairs its prefix sums per chunk,
and the first frame after a burst sums every row's estimate — so frame
cost grows with the scrollback, slowly: every frame at 200,000 rows is
still under 2 ms. The in-app numbers against xtext, measured in GtkHx,
are in [xtext-benchmark.md](xtext-benchmark.md).

### Against GtkTextView and GtkListView

`cargo run --release -p rotulus --example compare -- WIDGET N`, under a
private display (`xvfb-run -a`), times each widget through a real window
and frame clock: Rotulus; a `GtkTextView` with a line per message and the
nick in a tag (what Polari and many small clients do); and a
`GtkListView` of wrapping labels with the nick in markup (the
widget-per-message shape). Each appends N chat lines one call at a time,
as a chat does. Medians of repeated runs on one machine, Xvfb at 60 Hz,
so no frame is shorter than 16.7 ms:

| 20,000 messages | Rotulus | GtkTextView | GtkListView |
|---|---|---|---|
| ingest | 38 ms | 190 ms | 4.8 s |
| first paint at the bottom | 11 ms | 21 ms | 53 ms |
| settled (nothing left to do) | 12 ms | 540 ms | 77 ms |
| resize, until settled | 7 ms | 495 ms | 12 ms |
| worst frame while resizing | 20 ms | 24 ms | 21 ms |
| memory | 15.6 MB | 41 MB | 40 MB |

| 200,000 messages | Rotulus | GtkTextView | GtkListView |
|---|---|---|---|
| ingest | 365 ms | 1.95 s | 790 s |
| first paint at the bottom | 13 ms | 22 ms | 92 ms |
| settled | 13 ms | 4.6 s | 115 ms |
| resize, until settled | 6.5 ms | 4.8 s | 11 ms |
| worst frame while resizing | 20 ms | 42 ms | 20 ms |
| memory per message | 505 B | 534 B | 487 B |

What the numbers say:

- **GtkTextView** paints the bottom quickly and then validates every line
  in the background: seconds of work after each burst of history and each
  resize at scale, during which frames run long. It is the leanest per
  message at 200,000 rows.
- **GtkListView** is cheap once it has its rows — resize and scroll only
  touch the visible ones — but appending to a `GtkStringList` one item at
  a time grows worse than linearly: 4.8 s for 20,000 messages, thirteen
  minutes for 200,000. A client built on it would batch history and write
  its own list model, which is real work the comparison doesn't do.
- **Rotulus** ingests about five times faster than the text view, settles
  when it paints, and resizes in one frame at any size. At 200,000 rows it
  uses a little less memory per message than the text view and a little
  more than the list view; it used the most of the three until the row
  structure was trimmed (below).

Scrolling is at the frame rate for all three.

### Memory

`cargo bench -p rotulus-layout --bench memory` counts heap bytes with a
wrapping allocator, for the same corpus (chat lines of 3–19 words):

| Rows | After appending | Per row | Allocations per row | Every row laid out | Per row |
|---|---|---|---|---|---|
| 20,000 | 11.3 MB | 566 B | 3.7 | 16.1 MB | 805 B |
| 200,000 | 96.6 MB | 482 B | 3.7 | 144.3 MB | 721 B |

A row's text is about 60 bytes, so most of that is structure: the message,
its gutter and speaker strings, the span list, the index. Every row is laid
out only after scrolling through all of it. A scrollback of 500 rows
(GtkHx's default) is about a quarter of a megabyte.

It was 682 bytes and 4.7 allocations a row at 200,000 rows. Three changes
took it down, each measured on its own against the timing suite:

| Change | Bytes per row (200,000) | Cost |
|---|---|---|
| Before | 682 | |
| Layout cache boxed: only drawn rows carry one | 587 | none; scrolling at 200,000 rows got faster, 3.9 → 2.2 ms |
| Messages compacted as they enter the buffer | 489 | about 40 ns per appended message |
| A single body block held inline | 482, and one allocation fewer | none; ingest at 200,000 rows 44.7 → 39.0 ms |

Two things the measuring turned up. Compacting with `shrink_to_fit` split
every allocation in place and fragmented the heap badly enough to make
appending three times slower and scrolling five; copying into an exact
allocation instead frees whole blocks and costs almost nothing
(`ParsedText::compact`). And the parser benchmark runs first, because
running it after groups that build and free 200,000-row buffers measured
the allocator's fragmentation rather than the parser.

In the real widget (the comparison above) the same three changes took
memory at 200,000 messages from 769 to 505 bytes a message, with ingest,
paint, resize and scroll times unchanged.

### Render tests

`crates/rotulus/tests/render.rs` draws scenes — two columns, one column
with IRC formatting, history with a marker, a selection — through a real
window and compares them with `crates/rotulus/tests/golden/`. The font is
bundled (DejaVu Sans Mono, with its license) and pinned through
fontconfig, the renderer is cairo, the zone UTC, and the comparison is at
half resolution with a tolerance, so antialiasing differences between
FreeType versions pass and a row a line out of place does not. Like the
rest of the widget's tests they need a display. After a deliberate
change, re-render them with:

```sh
ROTULUS_UPDATE_GOLDEN=1 xvfb-run -a cargo test -p rotulus --test render
```

---

## 9. Not built

### Live re-rendering of historical rows

`Speaker` copies a nick and color rather than referring to the
application's user record, so a rename or a color change repaints the
application's user list and not the chat scrollback. Copying is cheaper,
and messages are arguably historical records of who said what under what
name at the time — worth changing only if live re-rendering of old rows
turns out to be wanted.

### Announcing new messages

The accessible text reports each append as an insertion, but the view does
not call `gtk_accessible_announce` for it. Whether a screen reader should
speak every new message, and how politely, is worth deciding with someone
who uses one.

### An introspectable row API

`RotulusRow` holds pointers to run arrays, which GObject introspection
cannot describe. Python, JavaScript and Vala will want a boxed message
type built call by call; that belongs with packaging the widget.
