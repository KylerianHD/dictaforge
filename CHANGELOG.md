# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Project foundations: roadmap, ADR index, text injection backend research.
- Cargo workspace with dictaforged, dictaforge, and dictaforge-cli stubs.
- CI: format and clippy checks, test matrix (Ubuntu x86_64/aarch64, Arch, Fedora), cargo-deny, and a check that no networking crates enter the default build.
- README, PRIVACY, CONTRIBUTING, SECURITY, code of conduct, and issue/PR templates.
- Research notes on wtype/dotool/ydotool/kdotool internals and the whisper.cpp API.
- ADRs 001 through 008: language, process split, injection chain, STT abstraction, audio capture, hotkey tiers, cleanup LLM, GUI toolkit (GTK4 + libadwaita).
