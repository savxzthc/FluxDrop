# Security Policy

## Supported Versions

| Version | Supported |
| --- | --- |
| 0.1.x | Yes |

## Reporting a Vulnerability

Report security issues through a [GitHub private security advisory](https://github.com/savxzthc/FluxDrop/security/advisories/new). Do not open a public issue for token bypasses, path exposure, remote code execution, or local network attack findings.

Include affected versions, reproduction steps, impact, and any proposed mitigation. Do not include active transfer URLs, tokens, private keys, or personal file paths.

## Security Model

FluxDrop transfers files between a trusted Windows PC and a trusted phone on a trusted private local network. It does not use cloud infrastructure, accounts, analytics, automatic update checks, or telemetry.

Token-bearing pages, metadata, uploads, and downloads use HTTPS with a self-signed certificate scoped to the selected LAN IP. The certificate protects against passive Wi-Fi observation after the user accepts it, but it does not authenticate the PC to the phone.

PC approval is required by default before a phone can download or upload a file. Approval requests include the phone IP and file metadata and expire after 60 seconds.

## Token and Link Controls

- Transfer tokens contain 160 bits of cryptographically secure randomness.
- Tokens are URL-safe, stored in memory only, and never logged in full.
- Links expire after a configurable 5, 10, 30, or 60 minutes.
- Links are single-use by default after a completed download.
- Invalid-token attempts are rate-limited per client IP.
- File paths are never accepted in transfer URLs.

## Data Handling

- Files stream directly between the PC and phone.
- Multiple files and folders are streamed as a bounded-memory ZIP.
- Uploads use randomized temporary files and no-clobber finalization.
- Partial upload files are removed after failure.
- Filenames and ZIP entry paths are sanitized before use.
- Transfer history excludes tokens, URLs, certificates, and file contents.

## Web and Desktop Hardening

- The public website loads runtime assets from the same origin and rejects inline executable code.
- The public website deploys a restrictive CSP, Trusted Types enforcement, clickjacking protection, cross-origin isolation headers, and a no-referrer policy.
- `npm run site:audit` rejects HTML comments, unsafe DOM sinks, external runtime assets, inline handlers, unsafe URL schemes, and missing deployment policies.
- Dynamic mobile-page content is HTML escaped.
- Download filenames are sanitized for traversal, control characters, and header injection.
- Mobile responses use a restrictive Content Security Policy and additional browser security headers.
- The Tauri desktop shell uses a restrictive CSP and minimal capabilities.
- The LAN server binds to a selected private address instead of `0.0.0.0` by default.

## Known Limitations

- Self-signed TLS cannot prevent an active man-in-the-middle attack on a hostile local network.
- Anyone who obtains a valid link may request access until it expires, although PC approval is required by default.
- LAN adapter detection is heuristic and can select the wrong adapter when VPNs or virtual adapters are active.
- Malware or a compromised operating system on either endpoint is outside FluxDrop's threat model.
- Current Windows installers are unsigned and may trigger Microsoft SmartScreen warnings.

Use FluxDrop on trusted private networks. Avoid guest Wi-Fi, public hotspots, and networks where untrusted clients can communicate directly.
