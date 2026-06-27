# Changelog

All notable changes to FluxDrop will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-06-27

### Added

- Public website with home, security, and support pages plus static build, secure preview, and security audit tooling.
- Windows shell integration for Explorer-based send flows and a global shortcut entry point.
- PC-side approval is now required by default before a phone can start a download.
- The desktop shows the requesting phone IP, filename, size, and approve/deny actions.
- Approval requests time out after 60 seconds with a distinct phone-facing timeout page.
- Persisted settings for 5/10/30/60-minute expiration, single-use links, approval mode, and preferred LAN adapter.
- Optional per-share overrides for expiration, single-use behavior, and approval mode.
- Multi-file and folder sharing as a bounded-memory, on-the-fly ZIP stream.
- ZIP entry path sanitization, symlink rejection, and duplicate-name disambiguation.
- System tray lifecycle with Open, Cancel current share, and Quit actions.
- Tray status indicators for idle, sharing, approval waiting, and active transfer states.
- Separate phone-to-PC Receive mode with QR setup, exact file metadata approval, and desktop progress.
- Streamed multipart uploads with configurable size limits, sanitized names, same-directory temp files, and no-clobber finalization.
- Persisted LAN adapter override with clean listener restart and active-transfer cancellation.
- Per-LAN-IP self-signed HTTPS for all token-bearing transfer pages, metadata, uploads, and downloads.
- Token-safe HTTP certificate onboarding page with browser instructions and same-host HTTPS target validation.
- Persisted System, Light, and Dark desktop themes with a header shortcut and automatic Windows appearance tracking.
- A native-style desktop shell with dedicated Send, Receive, and Settings workspaces, persistent navigation, clearer transfer guidance, and responsive layouts.
- Windows GUI subsystem builds in both debug and release modes so FluxDrop no longer opens a companion console window.
- Tag-driven Windows GitHub Releases with NSIS and MSI installers, draft release publishing, and version-consistency validation.
- A persisted History workspace showing transfer direction, filename, size, phone IP, outcome, and local time.
- Backend-only repeat references for restarting prior send or receive setups without persisting tokens or exposing full paths to React.
- Clear-history controls and a 100-record retention limit.

### Security

- Website deployment headers enforce a restrictive CSP, Trusted Types, cross-origin isolation, no-referrer policy, and same-origin runtime assets.
- Transfer traffic is encrypted against passive LAN observation after certificate acceptance.
- Certificate material is persisted in the app configuration directory and regenerated when the selected LAN IP changes.
- Documentation and UI explicitly disclose that self-signed TLS does not authenticate the PC or stop an active LAN man-in-the-middle.
- Vite and its React plugin were upgraded to remove the audited vulnerable esbuild dependency.

## [0.1.0] - 2026-06-15

### Added

- Initial PC-to-phone browser-based file sharing MVP
- Cryptographically secure one-time download tokens with 160-bit entropy
- Automatic link expiration with a 10-minute default
- Single-use links that expire after one successful download
- QR code display in the desktop app
- Mobile browser download page with no external dependencies
- Real-time transfer progress in the desktop UI
- Manual link cancellation
- Clean error pages for expired, cancelled, and invalid links
- Filename sanitization and header injection prevention
- HTML escaping of all dynamic content
- Security headers on HTTP responses
- Simple per-IP rate limiting for invalid token attempts
- LAN adapter filtering that prefers non-virtual private addresses
- Firewall and network troubleshooting help in the desktop UI
- Local HTTP server bound to a LAN IP instead of `0.0.0.0`

### Security

- Tokens use cryptographically secure random number generation
- File paths are never exposed in HTTP URLs
- No upload endpoints exist
- No directory listing endpoints exist
- Content-Security-Policy applied to mobile pages
- X-Content-Type-Options: nosniff on all responses
- X-Frame-Options: DENY on HTML responses
