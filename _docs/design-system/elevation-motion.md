# Elevation & Motion

## Elevation

Five soft shadows (`--shadow-1` → `--shadow-5`), each derived from the theme's
text colour so they re-tone automatically — dark themes get paler shadows. Their
presence is gated by the per-theme `--depth` knob (some themes are deliberately
flat).

<div class="elev-grid">
  <figure><div class="card" style="box-shadow:var(--shadow-1)"></div>shadow-1</figure>
  <figure><div class="card" style="box-shadow:var(--shadow-2)"></div>shadow-2</figure>
  <figure><div class="card" style="box-shadow:var(--shadow-3)"></div>shadow-3</figure>
  <figure><div class="card" style="box-shadow:var(--shadow-4)"></div>shadow-4</figure>
  <figure><div class="card" style="box-shadow:var(--shadow-5)"></div>shadow-5</figure>
  <figure><div class="card" style="box-shadow:3px 3px 0 var(--text-1);border:2px solid var(--text-1);border-radius:var(--radius-xs)"></div>hard 3,3,0</figure>
</div>

The last card is the **neobrutalist** flat-offset shadow the components lean on
(a 2px border in `--text-1` plus a hard `3px 3px 0` shadow). The Bevy storybook
reproduces both the soft and the hard styles with `BoxShadow`.

## Motion

Durations and easings, composed into named motion presets:

| Preset | Duration | Easing | For |
| --- | --- | --- | --- |
| `--motion-press` | 80ms (`fastest`) | `ease-3` | button press |
| `--motion-hover` | 150ms (`fast`) | `ease-3` | hover states |
| `--motion-enter` | 250ms (`base`) | `ease-out-3` | elements entering |
| `--motion-exit` | 150ms (`fast`) | `ease-in-3` | elements leaving |
| `--motion-emphasized` | 400ms (`slow`) | `ease-spring-2` | emphasized, springy moves |

Base durations: `fastest 80ms · fast 150ms · base 250ms · slow 400ms`. Easings
range from gentle (`ease-1`…`ease-5`) to expressive (`ease-elastic-3`,
`ease-squish-2`, the `ease-spring-2` `linear()` spring).
