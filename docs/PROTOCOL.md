# Federation Protocol Notes (Draft)

This document expands on the federation model outlined in `../BRIEF.md`.

## Event Graph

- Deterministic canonical JSON payloads.
- Event IDs derived from BLAKE3 hashes.
- Auth chains defined by `auth_events` references.

## Federation APIs

Document HTTP endpoints (`/_federation/...`) including authentication, payload schemas, and error handling.

## State Resolution

Specify linear resolver for MVP and planned progression to Matrix-style conflict resolution.

## Security

- Server key rotation cadence.
- HTTP Signature algorithm negotiation.
- Transparency log requirements.

_Fill each section with precise specs as features land._
