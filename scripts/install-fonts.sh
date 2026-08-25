#!/usr/bin/env bash
# Fetch Newsreader and Karla into assets/fonts and install them for the user.
#
# Rinch discovers fonts through fontconfig, so the two families the design
# calls for have to exist on the system. Run this once. Everything else in the
# app works with no network; this step does not.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="$root/assets/fonts"
mkdir -p "$dest"

base="https://github.com/google/fonts/raw/main/ofl"
declare -A fonts=(
  ["Karla[wght].ttf"]="$base/karla/Karla%5Bwght%5D.ttf"
  ["Newsreader[opsz,wght].ttf"]="$base/newsreader/Newsreader%5Bopsz%2Cwght%5D.ttf"
  ["Newsreader-Italic[opsz,wght].ttf"]="$base/newsreader/Newsreader-Italic%5Bopsz%2Cwght%5D.ttf"
)

for name in "${!fonts[@]}"; do
  if [[ ! -f "$dest/$name" ]]; then
    echo "downloading $name"
    curl -fsSL "${fonts[$name]}" -o "$dest/$name"
  fi
done

user_fonts="${XDG_DATA_HOME:-$HOME/.local/share}/fonts"
mkdir -p "$user_fonts"
cp "$dest"/*.ttf "$user_fonts/"
fc-cache -f "$user_fonts" >/dev/null

echo "installed:"
fc-list | grep -iE "newsreader|karla" | sed 's/^/  /'
