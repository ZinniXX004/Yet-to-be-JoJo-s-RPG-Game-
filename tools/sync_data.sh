#!/usr/bin/env sh
# Copies canonical content data into the Godot project.
#
# Godot cannot read files outside res://, but /data must stay the single source
# of truth (the Rust tests and the Python validator both read it directly).
# Rather than duplicating the JSON in version control, we copy it as a build
# step. src/game/data/ is therefore generated output and is gitignored.

set -eu

root=$(cd "$(dirname "$0")/.." && pwd)
src="$root/data"
dest="$root/src/game/data"

if [ ! -d "$src" ]; then
  echo "error: $src not found" >&2
  exit 1
fi

mkdir -p "$dest"
rm -f "$dest"/*.json
cp "$src"/*.json "$dest"/

echo "synced $(ls -1 "$dest"/*.json | wc -l) content file(s) to $dest"
