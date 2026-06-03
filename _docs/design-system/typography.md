# Typography

One fluid type scale (~1.25 / major-third ratio at the top of each range),
`Archivo` for body and display. The web sizes are `clamp()` ranges; the Rust
constants in `scale::font_size` take each range's upper bound as fixed pixels.

| Token | Rust | px (game) | Use |
| --- | --- | --- | --- |
| `--font-size-00` | `F00` | 12.8 | fine print, captions |
| `--font-size-0` | `F0` | 16 | body baseline |
| `--font-size-1` | `F1` | 20 | lead, sub-nav |
| `--font-size-2` | `F2` | 25 | card titles |
| `--font-size-3` | `F3` | 31 | section heads |
| `--font-size-4` | `F4` | 39 | page heads |
| `--font-size-5`–`8` | `F5`–`F8` | 49–95 | display |

## The scale

<div class="type-row"><span class="meta">F0 · 16</span><span class="sample" style="font-size:var(--font-size-0)">The quick brown fox</span></div>
<div class="type-row"><span class="meta">F1 · 20</span><span class="sample" style="font-size:var(--font-size-1)">The quick brown fox</span></div>
<div class="type-row"><span class="meta">F2 · 25</span><span class="sample" style="font-size:var(--font-size-2)">The quick brown fox</span></div>
<div class="type-row"><span class="meta">F3 · 31</span><span class="sample" style="font-size:var(--font-size-3)">The quick brown fox</span></div>
<div class="type-row"><span class="meta">F4 · 39</span><span class="sample" style="font-size:var(--font-size-4)">Arenic</span></div>
<div class="type-row"><span class="meta">F5 · 49</span><span class="sample" style="font-size:var(--font-size-5)">Arenic</span></div>
<div class="type-row"><span class="meta">F6 · 61</span><span class="sample" style="font-size:var(--font-size-6)">Arenic</span></div>

## Families & weights

- **Sans / body:** `Archivo`, then `system-ui` fallbacks (`--font-sans`).
- **Display:** `Monument Extended`, falling back to the sans stack (`--font-display`).
- **Mono:** the platform mono stack (`--font-mono`).

Weights run `100`–`900` (`scale::weight::W1`–`W9`). Bevy renders text in the
loaded font's own weight, so in the game the weight scale is documentation until
weighted font assets are wired up; on the web the variable font honours all nine.

Line-heights tighten as type grows: `1.5` for body (`LH0`/`LH1`) down to `1.0`
for the largest display (`LH7`).
