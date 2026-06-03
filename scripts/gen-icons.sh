#!/usr/bin/env bash
# Regenerate the Lucide icon PNGs used by the game and storybook.
#
# Lucide ships monochrome stroke SVGs (ISC licensed). Bevy has no native SVG
# renderer, so we rasterize each to a white-on-transparent PNG: rendered white,
# the icon can be tinted to any colour by `ImageNode.color` (see
# `arenic_game::icon`), so it re-tones with the theme.
#
# Requires: curl, rsvg-convert (librsvg).  Run from the repo root: ./scripts/gen-icons.sh
set -euo pipefail

ICONS=(
  panel-left-close panel-left-open   # sidebar collapse / expand
  chevron-right chevron-down         # tree disclosure
  folder folder-open                 # tree folders
  palette swatch-book type ruler     # story leaves …
  box layers component user
  rotate-3d                          # orbit/unlock 3D camera control
)

SRC="https://raw.githubusercontent.com/lucide-icons/lucide/main/icons"
OUT="assets/icons"
SVG="$OUT/svg"
SIZE="${1:-96}"   # raster size in px (24px viewBox upscaled)

mkdir -p "$SVG"
for name in "${ICONS[@]}"; do
  curl -fsSL "$SRC/$name.svg" -o "$SVG/$name.svg"
  # Render the stroke white so ImageNode tinting can recolour it.
  sed 's/currentColor/#ffffff/g' "$SVG/$name.svg" \
    | rsvg-convert -w "$SIZE" -h "$SIZE" -o "$OUT/$name.png"
  printf 'icon %-18s -> %s/%s.png\n' "$name" "$OUT" "$name"
done
echo "Done: ${#ICONS[@]} icons -> $OUT (white-on-transparent, ${SIZE}px)."
