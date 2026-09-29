# Rotulus

A GTK4 scrollback view for text-stream chat that stays fast and stable
however long the scrollback gets.

A *rotulus* is a medieval scroll, read top to bottom and grown by stitching
on new sheets, which is how chat scrollback grows too. Rotulus is a
successor to HexChat's xtext, rebuilt around structured messages instead of
text with escape codes in it. It started life as the chat view in
[GtkHx](https://github.com/mishan/gtkhx), which is still its first user.

Rotulus is not 1.0 yet, and its API may still change.

## What it promises

- **Cost follows what is on screen, not how long the scrollback is.**
  Appends, resizes and zooms lay out only the rows you can see. A row that
  never scrolls into view is never shaped.
- **Your reading position never jumps.** The scroll position is a row plus
  an offset, not a pixel value. Loading older history, trimming old lines,
  an image finishing its download, a resize and a font change all leave
  the text you are reading where it was. Sticking to the bottom is built
  in.
- **Scrollback grows at both ends.** Prepending a batch of history (IRCv3
  `chathistory`, Matrix backfill) is cheap, and so is trimming to a line
  limit.
- **Messages are data, not text with codes in it.** The application passes
  a speaker, styled runs, and blocks of text, code, quotes and images. The
  widget parses no escape codes, so a remote user can't restyle your chat by
  sending control bytes.
- **Chat conventions work out of the box.** Rotulus supports:
  - grouping by speaker
  - avatar and timestamp columns
  - copy that includes or leaves out timestamps
  - search and zoom
  - inline images
  - markdown code and quote blocks
  - a last-read marker line
  - a single-column mode for logs

  With no colors set, it follows the Adwaita theme.
- **The engine is tested without a display.** The layout crate has no
  dependencies, and its whole suite runs under plain `cargo test`.

Against GtkTextView and GtkListView, at 20,000 messages (medians on one
machine, at 60 Hz):

| | Rotulus | GtkTextView | GtkListView |
|---|---|---|---|
| ingest | 38 ms | 190 ms | 4.8 s |
| settled after the burst | 12 ms | 540 ms | 77 ms |
| resize, until settled | 7 ms | 495 ms | 12 ms |
| memory | 15.6 MB | 41 MB | 40 MB |

The full tables, including 200,000 messages, are in
[docs/design.md](docs/design.md).

## What it isn't

- **Not a widget per message.** It has no reaction buttons or thread
  expanders inside rows. An application that needs them wants GtkListView.
- **Not an editor.** The view is read-only.
- **Not a terminal.** It has no cursor addressing and no VT escapes; that's
  VTE's job.
- **No protocol knowledge.** A protocol converts its formatting into styled
  runs before appending them. `rotulus-mirc` does that for IRC formatting
  codes, as an opt-in.
- **GTK4 only.**

## Crates

| Crate | What it is |
|---|---|
| [`rotulus-layout`](crates/rotulus-layout) | The layout engine: the message model, wrapping, the height index, scroll anchoring, selection and search. It has no dependencies. |
| [`rotulus`](crates/rotulus) | The GTK4 widget over the engine, and the C ABI in [`include/rotulus.h`](crates/rotulus/include/rotulus.h). |
| [`rotulus-mirc`](crates/rotulus-mirc) | Converts IRC (mIRC) formatting codes into styled runs. The `rotulus` crate's default `mirc` feature exposes it to C as `rotulus_mirc_parse`. |

## Using it

**From Rust:**

```rust
use rotulus::RotulusView;
use rotulus_layout::{Message, ParsedText, Speaker};

let view = RotulusView::new();
view.append(Message::live(Speaker::new(42, "alice"), ParsedText::plain("hello")));
```

**From C**, through `rotulus.h`:

```c
GtkWidget *view = rotulus_view_new ();

RotulusRun nick = ROTULUS_RUN_PLAIN ("alice", -1);
RotulusRun body = ROTULUS_RUN ("hello", -1, ROTULUS_COLOR_DEFAULT,
                               ROTULUS_ATTR_BOLD);
RotulusRow row = {
    .kind = ROTULUS_ROW_MESSAGE,
    .speaker = { .key = 42, .nick = "alice", .nick_len = -1 },
    .gutter = &nick, .n_gutter = 1,
    .body = &body, .n_body = 1,
};
rotulus_view_append (view, &row);
```

The view is a GtkScrollable, so put it in a GtkScrolledWindow. The
application reacts through signals:

- `link-activated`, `link-menu`
- `speaker-activated`, `speaker-menu`
- `load-more`
- `media-activated`
- `selection-changed`

Its behavior is set through GObject properties such as `markdown`,
`link-schemes`, `activate-links`, `show-timestamps` and `max-lines`.
[docs/design.md](docs/design.md) covers what an application hooks, and
why the widget is shaped the way it is.

Today a C application links Rotulus statically: it bundles the `rotulus`
rlib into a Rust staticlib of its own, as GtkHx does. A versioned shared
library with pkg-config and GObject introspection files is planned before
1.0.

## Building and testing

You need GTK 4.10 or newer and Rust 1.92 or newer (see
`rust-toolchain.toml`).

```sh
cargo build
cargo test -p rotulus-layout                 # headless
xvfb-run -a cargo test --workspace            # the widget's tests need a display
```

Cargo features on `rotulus`:

- **`mirc`** (default): the C entry point to `rotulus-mirc`.
- **`v4_14`**: implements GtkAccessibleText, so a screen reader can read
  the transcript. Needs GTK 4.14. Without it, the view builds against
  GTK 4.10 and exposes only its accessible role.

The golden render tests draw with the bundled DejaVu Sans Mono and compare
the result at a tolerance. To accept a deliberate change to the rendering:

```sh
ROTULUS_UPDATE_GOLDEN=1 xvfb-run -a cargo test -p rotulus --test render
```

Benchmarks:

- **The engine:** `cargo bench -p rotulus-layout` runs the criterion
  benchmarks.
- **Against GtkTextView and GtkListView:** run
  `crates/rotulus/examples/compare.rs`.

## Translations

The widget's own strings, in its context menus, are in the `rotulus`
gettext domain. The catalogs are in `crates/rotulus/po`, with a Meson
fragment that compiles them. An application that installs the compiled
catalog calls `bindtextdomain ("rotulus", localedir)`. One that doesn't
gets English.

## License

LGPL-2.1-or-later. See [COPYING](COPYING). The bundled test fonts carry
their own license, in `crates/rotulus/tests/fonts/LICENSE`.
