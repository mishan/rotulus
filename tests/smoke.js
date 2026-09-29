// Drive librotulus-1 from GJS through its introspection data.

import Gtk from 'gi://Gtk?version=4.0';
import Rotulus from 'gi://Rotulus?version=1';
import System from 'system';

Gtk.init();

const view = new Rotulus.View();
const msg = Rotulus.Message.new(Rotulus.RowKind.MESSAGE);
msg.set_speaker(1, 'carol');
msg.add_text('hello from GJS', Rotulus.COLOR_DEFAULT, Rotulus.ATTR_NONE);
const mark = view.append_message(msg);
if (mark === null)
    System.exit(1);

const [n, current] = view.search('gjs', false);
if (n !== 1 || current !== 1) {
    printerr(`search found ${n}, current ${current}`);
    System.exit(1);
}
if (!view.remove(mark) || view.remove(mark))
    System.exit(1);

print('ok');
