#!/bin/sh
# Writes THIRD_PARTY_RUST.txt: every Rust crate compiled into the release app, with its licence.
set -eu
cd "$(dirname "$0")/.."
TARGET=${TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
{
    echo "Rust crates compiled into Mural for $TARGET (name version: licence)."
    echo "Licence texts ship with each crate's source on https://crates.io."
    echo
    cargo tree --locked --target "$TARGET" --edges normal,build --prefix none --format '{p}: {l}' \
        | sed -e 's/ (\*)$//' -e 's/ (\/[^)]*)//' | grep -v '^mural v' | sort -u
} > THIRD_PARTY_RUST.txt
echo "Wrote THIRD_PARTY_RUST.txt ($(($(wc -l < THIRD_PARTY_RUST.txt) - 3)) crates)"
