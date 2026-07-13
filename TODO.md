# TODO

Live task list. ROADMAP.md holds the milestone view.

## Next step (M0 wrap-up / M1 start, waiting for go)
- [ ] M0 exit review: /ponytail-audit over the repo, then decide whether to tag or fold remaining M0 leftovers into M1
- [ ] M1 planning: write the implementation plan for the vertical slice (audio -> whisper -> inject)

## Backlog notes
- [ ] Create GitHub remote once the project name is decided (a clean start beats rename redirects)
- [ ] Release workflow (artifacts on tag) lands with the first tag at end of M1
- [ ] Headless sway/Xvfb injection tests in CI land with the injection code in M1
- [ ] Replace the short code of conduct with Contributor Covenant if the community grows

## Open decisions
- [ ] Final project name. OpenFlow collides with the ONF OpenFlow SDN protocol (trademarked, huge mindshare, existing crates). Candidates: voxflow, libredictate, flowsay, dikt. Keeping the working name until decided.

## Done
- [x] 2026-07-13: research notes (injection tools from source, whisper.cpp API incl. built-in Silero VAD find), ADRs 001-008 accepted (GUI: GTK4 + libadwaita)
- [x] 2026-07-13: workspace skeleton (3 binaries), CI (lint, test matrix, deny, privacy grep), hygiene files, templates
- [x] 2026-07-12: repo init (main + dev), LICENSE, .gitignore, ROADMAP.md, ADR index, injection backend decision table
