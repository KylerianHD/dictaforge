# TODO

Live task list. ROADMAP.md holds the milestone view.

## Next step (M0 part 2, waiting for go)
- [ ] Cargo workspace skeleton: openflowd, openflow, openflow-cli, crates/*
- [ ] CI workflows: fmt, clippy -D warnings, test, cargo-deny, no-network-crates grep, build matrix
- [ ] Repo hygiene: README, PRIVACY.md, CONTRIBUTING.md, SECURITY.md, CODE_OF_CONDUCT.md, templates

## Then (M0 part 3)
- [ ] Research notes: wtype/dotool/ydotool/kdotool source reading, whisper.cpp API
- [ ] Write ADRs 001-007, evaluate GUI toolkit and decide ADR 008

## Open decisions
- [ ] Final project name. OpenFlow collides with the ONF OpenFlow SDN protocol (trademarked, huge mindshare, existing crates). Candidates: voxflow, libredictate, flowsay, dikt. Keeping the working name until decided.

## Done
- [x] 2026-07-12: repo init (main + dev), LICENSE, .gitignore, ROADMAP.md, ADR index, injection backend decision table
