# TODO

Live task list. ROADMAP.md holds the milestone view.

## Next step (M1, waiting for go)
- [ ] M1 Task 1: reverse keymap core (plan: .claude/plans/2026-07-13-m1-vertical-slice.md, local only)
- [ ] M1 Task 0 (rename + GitHub remote) slots in as soon as the name is settled

## Open decisions
- [ ] Name: user picked VoxForge 2026-07-13, but voxforge.org is an active FOSS speech-recognition corpus project (same domain, since 2006). Decision pending: keep anyway, nearby variant, or new candidates.

## Backlog notes
- [ ] Create GitHub remote once the project name is decided (a clean start beats rename redirects)
- [ ] Release workflow (artifacts on tag) lands with the first tag at end of M1
- [ ] Headless sway/Xvfb injection tests in CI land with the injection code in M1
- [ ] Replace the short code of conduct with Contributor Covenant if the community grows

## Done
- [x] 2026-07-13: M0 exit review (ponytail-audit: 2 micro-cuts applied, otherwise lean; no M0 tag, folds into v0.1.0), M1 plan written (11 tasks, local plan file)
- [x] 2026-07-13: research notes (injection tools from source, whisper.cpp API incl. built-in Silero VAD find), ADRs 001-008 accepted (GUI: GTK4 + libadwaita)
- [x] 2026-07-13: workspace skeleton (3 binaries), CI (lint, test matrix, deny, privacy grep), hygiene files, templates
- [x] 2026-07-12: repo init (main + dev), LICENSE, .gitignore, ROADMAP.md, ADR index, injection backend decision table
