# ADR 005: Audio capture through cpal

Status: accepted (2026-07-13)

## Context

Target systems run PipeWire (current), PulseAudio (older LTS), or bare ALSA. cpal covers all three behind one API and is the ecosystem default.

## Decision

cpal for capture, preferring the PipeWire host where present, falling back to Pulse/ALSA. Resample whatever the device gives us to 16 kHz mono f32 in our own pipeline. A simple gain-normalization/AGC stage sits before the STT engine so whispered speech transcribes reliably.

## Consequences

- No hard PipeWire dependency, so old Debian/Ubuntu LTS still works.
- Device picking (settings UI) is a cpal device enumeration, nothing distro-specific.
- If cpal's PipeWire support proves too coarse (per-application capture indicators, node naming), a direct pipewire-rs capture path can replace it behind the same internal interface; not built until proven necessary.
