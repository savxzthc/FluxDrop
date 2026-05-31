# Security Policy

## Supported Versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |

## Reporting a Vulnerability

Please report security issues through a GitHub private security advisory. Do not open public issues for vulnerabilities, token bypasses, path exposure, or local network attack reports.

## Security Model

FluxDrop v0.1 sends one file from a trusted PC to a trusted phone over a trusted local network. It does not use cloud infrastructure, accounts, analytics, automatic update checks, or telemetry.

FluxDrop protects against link guessing by using a URL-safe token generated from 20 cryptographically random bytes. Links expire after 10 minutes and are single-use after one completed download.

FluxDrop protects file paths by validating and canonicalizing the selected file at share creation, then storing the path in memory. HTTP routes only accept tokens, never file paths. There are no upload endpoints and no directory listing routes.

FluxDrop escapes dynamic HTML content, sanitizes filenames, strips CRLF from header filenames, and sends security headers on HTTP responses.

## What FluxDrop Does Not Protect Against

FluxDrop v0.1 does not encrypt traffic. A passive LAN observer may be able to read local HTTP traffic. Someone who sees or receives the QR code before it expires may be able to download the file. Malicious software on the PC or phone is outside the v0.1 threat model.

## Token Model

Tokens are generated with Rust's CSPRNG through `rand::thread_rng` and encoded as unpadded base64url. Tokens are stored only in process memory, tied to one active share session, and invalidated by timeout, cancellation, or successful single-use completion.

## No Telemetry

FluxDrop never phones home, never checks for updates automatically, and never sends any usage data anywhere.

## Known Limitations

- v0.1 uses local HTTP, not HTTPS.
- LAN adapter detection is heuristic and may choose the wrong adapter when VPNs or virtual adapters are present.
- Approval-before-download and trusted-device workflows are intentionally deferred to the roadmap.
