#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"
cargo build --release
stage="dist/RustyShelf-linux-x86_64"
mkdir -p "$stage"
install -m 755 target/release/rustyshelf "$stage/rustyshelf"
install -m 644 packaging/rustyshelf.desktop "$stage/rustyshelf.desktop"
install -m 644 README.md "$stage/README.md"
install -m 644 CUSTOM_STYLE.md "$stage/CUSTOM_STYLE.md"
install -m 644 examples/rustyshelf.css "$stage/rustyshelf.css"
tar -C dist -czf dist/RustyShelf-linux-x86_64.tar.gz RustyShelf-linux-x86_64
printf 'Package créé : %s\n' "$PWD/dist/RustyShelf-linux-x86_64.tar.gz"
