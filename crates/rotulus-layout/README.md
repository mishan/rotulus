# rotulus-layout

The layout engine behind [Rotulus](https://github.com/mishan/rotulus), a
GTK4 scrollback view for text-stream chat.

It covers everything between "a message arrives" and "the view knows
which pixels go where":

- the message model, with styled text, links and blocks
- markdown and link detection
- wrapping, and a height index that answers pixel-to-row lookups over
  rows of different heights
- a scroll anchor that keeps the reader's place through resizes, history
  backfill and trimming
- hit testing, selection and search

It has no dependencies: no GTK, no GLib, no Pango. Text measurement sits
behind the `TextMeasure` trait, which the `rotulus` widget implements
with Pango and the tests implement with a fixed-width font, so the whole
engine runs under a plain `cargo test`. A front end other than GTK can
supply its own.

Most applications want the widget, [`rotulus`](https://crates.io/crates/rotulus),
rather than this crate. How the two fit together is in the repository's
[design notes](https://github.com/mishan/rotulus/blob/main/docs/design.md).

License: LGPL-2.1-or-later.
