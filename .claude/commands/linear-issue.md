---
description: Create a well-formed Linear issue in the Arenic project with the full label taxonomy
argument-hint: <short description of the bug/feature/task>
allowed-tools: mcp__claude_ai_Linear__save_issue, mcp__claude_ai_Linear__list_issue_labels, mcp__claude_ai_Linear__get_issue, mcp__claude_ai_Linear__list_issues, AskUserQuestion
---

# Create a Linear issue for Arenic

Create a Linear issue for the following request:

> $ARGUMENTS

If the request above is empty, ask the user what the issue is before doing anything else.

## Destination (always the same — never change these)

- **Team:** `Arenic` (key/prefix **`ARE`**, e.g. `ARE-42`)
- **Project:** `Arenic`
- Create with `mcp__claude_ai_Linear__save_issue` passing `team: "Arenic"` and `project: "Arenic"`.
- Team id (only if a name lookup ever fails): `b60a8e55-867c-4666-9c48-78cc95533dab`

## Label taxonomy

Four **label groups** — labels inside one group are **mutually exclusive** (Linear rejects two
from the same group on one issue). Apply **at most one label per group**, plus any of the flat
workspace labels. Omit a group if none fits (e.g. leave **Platform** off for cross-platform work).

- **Discipline** (who owns it): Engineering · Design · Art · Audio · Animation · Narrative · UI/UX · QA · Tech Art · Production
- **Area** (system/subsystem): Gameplay · Combat · Abilities · UI · HUD · Camera · Input · Rendering · VFX · AI · Physics · Multiplayer · Save/Load · Performance · Build/CI · Tooling · Design System · Storybook · Accessibility
- **Platform** (only if OS-specific): Windows · macOS · Linux
- **Type** (game-specific nuance): Polish · Tech Debt · Chore · Spike · Crash · Regression · Playtest

**Flat workspace labels** (not exclusive — pick the broad nature): `Bug` · `Feature` · `Improvement`

> Pass labels to `save_issue` as a `labels` array of names, e.g.
> `["Bug", "Crash", "Engineering", "Combat"]`. If you are unsure a label exists, call
> `mcp__claude_ai_Linear__list_issue_labels` with `team: "Arenic"` first.

## How to fill it in

1. **Infer the labels** from the request using the taxonomy above — one Discipline, one Area,
   one Type if applicable, one Platform only if OS-specific, plus one flat Bug/Feature/Improvement.
   If the Discipline or Area is genuinely ambiguous, use `AskUserQuestion` to confirm; otherwise
   pick the obvious one and proceed.
2. **Priority** (`priority`): `1`=Urgent, `2`=High, `3`=Medium, `4`=Low. Default to `3` unless the
   request implies otherwise (crashes/blockers → 1–2; minor polish → 4).
3. **Title:** concise, imperative or descriptive, no trailing period.
4. **Description:** Markdown using the template below. Use literal newlines, not `\n`.

```markdown
## Summary
<what's wrong / what's wanted, in 1–3 sentences>

## Expected behavior
<what should happen>

## Why this matters
<player/UX/engineering impact — short bullets>

## Acceptance criteria
- [ ] <testable outcome>
- [ ] <testable outcome>
- [ ] Builds clean: `cargo clippy --workspace --all-targets -- -D warnings`; Bevy 0.18 APIs only

## Notes
<links, related issues, file pointers — optional>
```

## After creating

Report back the issue identifier (e.g. `ARE-42`), its URL, and the labels you applied.
Do **not** start implementing the work unless the user explicitly asks.
