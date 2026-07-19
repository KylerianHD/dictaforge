# Contributing

Early days; expect churn until v0.1.0. Check ROADMAP.md before starting anything big, and open an issue first for features.

## Workflow

- `main` is release-only. `dev` is the integration branch.
- Branch from `dev`: `feat/...`, `fix/...`, `docs/...`. Pull requests are squash-merged into `dev`.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/): `feat:`, `fix:`, `docs:`, `chore:`, `refactor:`.

## Before you push

```
cargo fmt
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Ground rules

- No telemetry and no networking crates in the default build. CI enforces this; it is not negotiable.
- Prefer the simplest change that works. Unused flexibility is a bug.
- New injection or keymap logic needs tests across keyboard layouts, not just US QWERTY.
