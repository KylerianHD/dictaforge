# Security policy

OpenFlow runs a daemon with access to your microphone and input devices, so security reports are taken seriously.

## Reporting a vulnerability

Use GitHub's private vulnerability reporting ("Report a vulnerability" under the Security tab). Please do not open public issues for exploitable bugs.

You can expect an initial response within a week. Coordinated disclosure is fine; state your preferred timeline in the report.

## Scope notes

- The daemon must never require root. uinput access goes through a udev rule or the input group.
- Anything that makes the default build touch the network is a vulnerability by project policy, not just a bug.
