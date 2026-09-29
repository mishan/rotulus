# rotulus-mirc

IRC formatting codes, as [Rotulus](https://github.com/mishan/rotulus)
styled text.

IRC carries its styling in-band: a byte like `0x02` toggles bold, and
`0x03` followed by digits sets a color. Rotulus never interprets bytes
like these itself, because a view that did would let anyone who can send
it text restyle its transcript. So an IRC client decides which messages
may carry formatting, and converts those here, into ordinary styled runs,
before appending them.

```rust
let text = rotulus_mirc::parse("\x02bold\x02 and \x034red");
assert_eq!(text.text, "bold and red");

assert_eq!(rotulus_mirc::strip("\x02plain\x02"), "plain");
```

The vocabulary is the one documented at
<https://modern.ircdocs.horse/formatting>: bold, italic, underline,
strikethrough, monospace, reverse, reset, and colors by number or by
hex. Nothing here fails: an incomplete code is dropped, and anything that
isn't a code is text.

License: LGPL-2.1-or-later.
