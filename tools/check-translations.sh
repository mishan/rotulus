#!/bin/sh
# Fail when the widget's translations have fallen behind its source.
#
# Two ways they fall behind, and nothing about changing a string reminds
# anyone of either:
#
# - po/POTFILES doesn't list a source file that has strings in it, so those
#   strings never reach the catalog and can't be translated.
# - A catalog lacks a string, or has it only as fuzzy or untranslated, so a
#   user of that language gets English for it.
#
# A new string therefore needs a translation in every language in LINGUAS
# before it merges. Update the catalogs from the source with
# `meson compile -C _build rotulus-update-po`.
set -eu
cd "$(dirname "$0")/../crates/rotulus"

xgettext_rs() {
    xgettext --language=Rust --from-code=UTF-8 --keyword=tr:1 "$@"
}

# Which sources actually yield a msgid. The grep only narrows the
# candidates: it would also match a tr( with no literal to extract.
expected=$(
    for f in $(grep -rlE '\btr[[:space:]]*\(' --include='*.rs' src | sort); do
        if xgettext_rs -o - "$f" 2>/dev/null | grep -q '^msgid "..*"'; then
            echo "$f"
        fi
    done
)
listed=$(grep -v '^#' po/POTFILES)
if [ "$expected" != "$listed" ]; then
    echo "crates/rotulus/po/POTFILES is out of date; it should read:" >&2
    echo "$expected" >&2
    exit 1
fi

pot=$(mktemp)
trap 'rm -f "$pot"' EXIT
xgettext_rs --files-from=po/POTFILES -o "$pot"
# xgettext leaves the charset as a placeholder; the sources are UTF-8.
sed -i.bak 's/charset=CHARSET/charset=UTF-8/' "$pot" && rm -f "$pot.bak"

status=0
for lang in $(grep -v '^#' po/LINGUAS); do
    # msgcmp fails on any msgid the catalog lacks or leaves fuzzy or
    # untranslated.
    if ! msgcmp "po/$lang.po" "$pot"; then
        echo "crates/rotulus/po/$lang.po is missing translations" >&2
        status=1
    fi
done
[ "$status" = 0 ] && echo "Translations are up to date."
exit "$status"
