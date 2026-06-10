# Encounter scores

Authored boss-encounter timelines, written by **author mode** (`just author`)
and replayed by the game. One directory per arena (its slug), one per
difficulty inside it:

```
encounters/<arena-slug>/<difficulty>/boss.v0001.ron   # boss movement + ability staff
encounters/<arena-slug>/<difficulty>/tiles.v0001.ron  # tile choreography keyframes
```

- **Readers always load the highest `vNNNN`** for the active difficulty, and
  re-check at every 2-minute cycle wrap — so the game and author mode iterate
  in tandem through these files.
- **Writers always write `latest + 1`** (author mode's Commit / `W`).
- **Roll back** by deleting the newest file(s); the previous take is live at
  the next wrap (`F5` in author mode restarts an arena immediately). These
  files are committed to git, so history is the second level of undo.
- Schemas live in `arenic_game::encounter` (`BossScoreFile`) and
  `arenic_game::tile_script` (`TileScriptFile`); both are plain RON and safe
  to hand-edit — e.g. a `SineWave` tile selector is easier typed than painted.
