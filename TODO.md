# TODO

Live task list. ROADMAP.md holds the milestone view.

## Next step (M0 part 3, waiting for go)
- [ ] Research notes: wtype/dotool/ydotool/kdotool source reading, whisper.cpp API
- [ ] Write ADRs 001-007, evaluate GUI toolkit and decide ADR 008

## Backlog notes
- [ ] Create GitHub remote once the project name is decided (a clean start beats rename redirects)
- [ ] Release workflow (artifacts on tag) lands with the first tag at end of M1
- [ ] Headless sway/Xvfb injection tests in CI land with the injection code in M1
- [ ] Replace the short code of conduct with Contributor Covenant if the community grows

## Open decisions
- [ ] Final project name. OpenFlow collides with the ONF OpenFlow SDN protocol (trademarked, huge mindshare, existing crates). Candidates: voxflow, libredictate, flowsay, dikt. Keeping the working name until decided.

## Done
- [x] 2026-07-13: workspace skeleton (3 binaries), CI (lint, test matrix, deny, privacy grep), hygiene files, templates
- [x] 2026-07-12: repo init (main + dev), LICENSE, .gitignore, ROADMAP.md, ADR index, injection backend decision table
