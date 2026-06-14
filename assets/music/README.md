# Music

Soundtrack files for the music engine (`arenic_game::audio`). The track table
lives in `crates/arenic_game/src/audio/mod.rs` (`arena_track` + `THEME_PATH`) —
**wiring a new arena's track is: drop the file here, fill in its table line.**

- `arenic_theme.mp3` — the title screen.
- `guildmaster_guildhouse.mp3` — Guildmaster / Guild House (zoomed in).
- `warrior_bastion.mp3` — Warrior / Bastion (zoomed in).
- `merchant_casino.mp3` — Merchant / Casino (zoomed in).
- The **overworld** plays no file: its soundtrack is the procedural oscillator
  drone (`arenic_game::audio::DroneSource`), generated in code.
- Arenas without a track play nothing while zoomed in — the spatial SFX carry
  the scene until their music lands.

Conventions for new tracks:

- **Format**: `.mp3` or `.ogg` (the build enables Bevy's `mp3` + default
  `vorbis` features; nothing else — no wav/flac).
- **Loop-ready**: tracks play on `PlaybackSettings::LOOP`; trim silence so the
  seam isn't a dropout.
- **Loudness**: target roughly −16 LUFS integrated so the ~1.2 s crossfades
  stay level between arenas (the engine plays tracks at a fixed 0.55 gain).
- Arena changes crossfade automatically; zooming out crossfades into the
  drone and ducks the SFX bus.
