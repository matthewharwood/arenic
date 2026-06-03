# Space & Radii

## Space

A single rhythm for gaps, padding and margins — `3xs` (5px) up to `4xl` (160px).
Rust: `scale::space::{XS3, XS2, XS, S, M, L, XL, XL2, XL3, XL4}`.

<div class="scale-row"><span class="meta">3xs · 5</span><span class="bar" style="width:var(--size-3xs)"></span></div>
<div class="scale-row"><span class="meta">2xs · 10</span><span class="bar" style="width:var(--size-2xs)"></span></div>
<div class="scale-row"><span class="meta">xs · 15</span><span class="bar" style="width:var(--size-xs)"></span></div>
<div class="scale-row"><span class="meta">s · 20</span><span class="bar" style="width:var(--size-s)"></span></div>
<div class="scale-row"><span class="meta">m · 30</span><span class="bar" style="width:var(--size-m)"></span></div>
<div class="scale-row"><span class="meta">l · 40</span><span class="bar" style="width:var(--size-l)"></span></div>
<div class="scale-row"><span class="meta">xl · 60</span><span class="bar" style="width:var(--size-xl)"></span></div>
<div class="scale-row"><span class="meta">2xl · 80</span><span class="bar" style="width:var(--size-2xl)"></span></div>
<div class="scale-row"><span class="meta">3xl · 120</span><span class="bar" style="width:var(--size-3xl)"></span></div>

## Radii

`scale::radius::{XS3 … XL2}`, plus `PILL` for fully-rounded ends (for a circle
use `Val::Percent(50.0)`). Each theme also carries structural radii
(`radius_selector` / `radius_field` / `radius_box`) — that's why themes like
**synthwave** and **cyberpunk** read as sharp (all `0`), while **dark** and
**forest** soften (`0.5rem` / `1rem`).

<div class="radii-grid">
  <figure><div class="box" style="border-radius:var(--radius-3xs)"></div>3xs · 3</figure>
  <figure><div class="box" style="border-radius:var(--radius-2xs)"></div>2xs · 4</figure>
  <figure><div class="box" style="border-radius:var(--radius-xs)"></div>xs · 6</figure>
  <figure><div class="box" style="border-radius:var(--radius-s)"></div>s · 8</figure>
  <figure><div class="box" style="border-radius:var(--radius-m)"></div>m · 12</figure>
  <figure><div class="box" style="border-radius:var(--radius-l)"></div>l · 16</figure>
  <figure><div class="box" style="border-radius:var(--radius-xl)"></div>xl · 24</figure>
  <figure><div class="box" style="border-radius:var(--radius-2xl)"></div>2xl · 32</figure>
</div>
