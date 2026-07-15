# TODO

Live task list. ROADMAP.md holds the milestone view.

## Next step (M1, waiting for go)
- [ ] M1 Task 2: active layout detection (plan: .claude/plans/2026-07-13-m1-vertical-slice.md, local only)

## Decided
- [x] Name: DictaForge (2026-07-13). Verified clean: crates.io free, AUR free, no web presence, one dormant unrelated 0-star GitHub repo. VoxForge rejected (voxforge.org, same-domain FOSS project), VoxSmith rejected (crates.io taken by active voxel crate, domains gone). Binaries: dictaforge (GUI), dictaforged (daemon), dictaforge-cli. Rename lands as M1 Task 0.

## Backlog notes
- [ ] Release workflow (artifacts on tag) lands with the first tag at end of M1
- [ ] Headless sway/Xvfb injection tests in CI land with the injection code in M1
- [ ] Replace the short code of conduct with Contributor Covenant if the community grows

## Done
- [x] 2026-07-15: M1 Task 1: reverse keymap core merged (PR #2, squash 74dbfeb, CI 6/6). KeymapIndex reverse-maps chars to evdev keycode + mod mask + group via xkbcommon; dead keys as compose Sequences; round-trip tests on us, de, us(dvorak), fr. Finding: KEY_EURO (evdev 435) makes the euro sign reachable on every layout, so plain us types it too
- [x] 2026-07-13: M1 Task 0: renamed to DictaForge repo-wide, private GitHub repo created (KylerianHD/dictaforge, dev default), first CI run green (6 jobs), PR #1 squash-merged. Server-side branch protection impossible on free private repos; local pre-push hook guards main/dev against force push and deletion instead, prepared rulesets wait in .claude/rulesets/ for when the repo goes public (target: v0.1.0)
- [x] 2026-07-13: M0 exit review (ponytail-audit: 2 micro-cuts applied, otherwise lean; no M0 tag, folds into v0.1.0), M1 plan written (11 tasks, local plan file)
- [x] 2026-07-13: research notes (injection tools from source, whisper.cpp API incl. built-in Silero VAD find), ADRs 001-008 accepted (GUI: GTK4 + libadwaita)
- [x] 2026-07-13: workspace skeleton (3 binaries), CI (lint, test matrix, deny, privacy grep), hygiene files, templates
- [x] 2026-07-12: repo init (main + dev), LICENSE, .gitignore, ROADMAP.md, ADR index, injection backend decision table
