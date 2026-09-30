Title: Overview

Rotulus is a GTK4 scrollback view for text-stream chat that stays fast
and stable however long the scrollback gets.

The widget is [class@Rotulus.View]. Put it in a [class@Gtk.ScrolledWindow], set
it up through its properties, and add rows to it.

## Adding rows

From C, build a [struct@Rotulus.Row] on the stack and pass it to
[method@Rotulus.View.append]. From a language binding, build a
[struct@Rotulus.Message] call by call and pass it to
[method@Rotulus.View.append_message]:

```python
import gi
gi.require_version("Rotulus", "1")
from gi.repository import Rotulus

view = Rotulus.View()
msg = Rotulus.Message.new(Rotulus.RowKind.MESSAGE)
msg.set_speaker(42, "alice")
msg.add_text("hello", Rotulus.COLOR_DEFAULT, Rotulus.ATTR_BOLD)
view.append_message(msg)
```

Rows are structured data, never text with escape codes in it, so nothing
a remote user sends can restyle the transcript. IRC formatting codes go
through [method@Rotulus.Message.add_mirc], which converts them to styled
runs.

Every append returns a [struct@Rotulus.Mark], a weak handle to the row,
for inserting history before it, replacing it, or removing it.

## Reacting

The application answers the view's signals: a link clicked or
right-clicked, a speaker clicked, a request for older rows, an inline
image clicked, the selection changing.

## Using it

C programs link `librotulus-1` through pkg-config:

```sh
cc app.c $(pkg-config --cflags --libs rotulus-1)
```

Rust programs use the [rotulus crate](https://crates.io/crates/rotulus).

How the widget works, and why it is shaped the way it is, is in the
[design notes](https://github.com/mishan/rotulus/blob/main/docs/design.md).
