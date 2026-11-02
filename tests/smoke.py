#!/usr/bin/env python3
"""Drive librotulus-1 from Python through its introspection data."""

import gi

gi.require_version("Gtk", "4.0")
gi.require_version("Rotulus", "1")
from gi.repository import Gdk, Gtk, Rotulus  # noqa: E402

Gtk.init()

view = Rotulus.View()
assert isinstance(view, Gtk.Widget)
assert isinstance(Rotulus.View.new(), Rotulus.View)

view.set_avatar_func(lambda v, key: Gdk.Paintable.new_empty(16, 16) if key == 42 else None)

msg = Rotulus.Message.new(Rotulus.RowKind.MESSAGE)
msg.set_speaker(42, "alice")
msg.add_nick("alice", Rotulus.COLOR_DEFAULT, Rotulus.ATTR_BOLD)
msg.add_text("hello from Python", Rotulus.COLOR_DEFAULT, Rotulus.ATTR_NONE)
first = view.append_message(msg)
assert first is not None

second = view.append_text("a plain row", -1, 0)
assert view.get_last() is not None

assert view.replace_message(first, msg.copy())
n, current = view.search("python", False)
assert (n, current) == (1, 1), (n, current)

assert view.remove(second)
assert not view.remove(second)

seen = []
view.connect("load-more", lambda v, d: seen.append(d))
view.emit("load-more", Rotulus.LoadDirection.OLDER)
assert seen == [Rotulus.LoadDirection.OLDER]

# The signals naming a row hand over a mark the view takes back.
assert view.set_reply(first, "alice", Rotulus.COLOR_DEFAULT, "hello")
assert view.set_reply_unavailable(first, "gone")
marked = []
view.connect("reply-activated", lambda v, mark: marked.append(v.clear_reply(mark)))
view.connect("row-menu", lambda v, mark, x, y: marked.append(v.clear_reply(mark)) or True)
view.emit("reply-activated", first)
assert view.emit("row-menu", first, 1.0, 2.0)
assert marked == [True, True], marked

reactions = Rotulus.Reactions.new()
reactions.add("+1", 3, True, "thumbs up")
assert view.set_reactions(first, reactions)
keys = []
view.connect("reaction-activated", lambda v, mark, key: keys.append((v.set_reactions(mark, None), key)))
view.emit("reaction-activated", first, "+1")
assert keys == [(True, "+1")], keys

view.set_property("markdown", True)
assert view.get_property("markdown")
view.set_link_schemes(["https://", "irc://"])
assert list(view.get_property("link-schemes")) == ["https://", "irc://"]

palette = [Gdk.RGBA() for _ in range(Rotulus.PAL_COLS)]
view.set_palette(palette)

print("ok")
