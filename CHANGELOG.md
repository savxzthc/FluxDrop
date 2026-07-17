# Changelog

All notable changes to FluxDrop will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-07-16

### Added

- Resumable ordinary-file downloads with standards-compliant single-range responses and unique-byte completion tracking.
- Atomic multi-file phone uploads approved as one sanitized manifest with aggregate limits, collision-safe publication, and rollback.
- Structured Windows Firewall diagnosis plus an explicit UAC-elevated repair for a private-profile, local-subnet application rule.
- Signed installed-build updates with background verification, active-transfer deferral, restart confirmation, and portable-build release reporting.

### Changed

- Receive history records now store aggregate batch size and file count.
- Release automation now publishes signed installer updater artifacts, `latest.json`, installers, portable builds, and checksums.
- Automatic updates are enabled by default and can be disabled in Settings.

## [0.3.0] - 2026-07-03

### Added

- Start-screen readiness checks, recent repeatable transfers, per-transfer security overrides, and active transfer timelines for Send and Receive.
- Link expiry meters on QR cards plus cleaner copy controls and a bundled app favicon.
- Searchable transfer history with summary stats, filtered copy summaries, privacy-safe CSV export, and repeat-unavailable reasons.
- Private history mode, settings diagnostics copy, and a scrub action that removes saved local repeat paths from existing history.
- LAN troubleshooting copy bundles and checklists for both send and receive flows.
- Disabled/preparing states for start actions so users cannot accidentally queue duplicate share or receive setup attempts.
- Receive setup now verifies destination-folder write access before showing the phone QR code.
- Phone upload pages now preview the selected file name and size and block files above the receive limit before requesting approval.
- Saved settings and per-transfer receive options now expose upload limits up to the backend-supported 16 GB maximum.
- Send and Receive setup now fail clearly when no private LAN adapter is available instead of falling back to an unusable localhost QR link.

### Security

- Server upload streaming now keeps the unlimited body allowance scoped to `/upload/:token`; JSON metadata endpoints retain Axum's default request body cap.
- Download metadata APIs now return precise JSON errors for expired and denied links instead of collapsing every invalid state to `404`.
- Receive/upload phone pages now use upload-specific denied, expired, timeout, and rate-limit copy.
- QR images now render through data URIs instead of DOM SVG injection, and the desktop WebView CSP blocks object, base, and frame injection.
- Local transfer pages now send a stricter CSP that blocks script attributes, frames, workers, manifests, media, fonts, and non-FluxDrop form targets.
- Upload failures now return stable JSON error codes for storage/write failures so phone-side copy can stay actionable without exposing paths.
- Windows reserved device names and excessively long filename components are sanitized in incoming filenames and generated archive paths.
- History CSV export neutralizes spreadsheet formula prefixes in user-controlled fields.
- Token validation now requires FluxDrop's exact generated 27-character URL-safe token shape on server routes and certificate-onboarding links.
- Upload status polling now counts malformed token probes toward the same per-IP invalid-attempt limit as other transfer routes.
- PC approvals are now bound to the phone IP that requested them, so another LAN client cannot reuse an approved token from a different address.
- Settings and history saves now replace files atomically on Windows instead of deleting the old local store before renaming the new one.
- Desktop IPC no longer sends raw transfer tokens as separate fields; the UI only receives the link it must display or copy.

### Fixed

- Receive-side approve, deny, and cancel actions now persist the latest status message for subsequent UI polling.
- Receive-side upload start and progress events now persist the same status message shown by later polling.
- Receive status text now describes phone uploads instead of reusing download-specific completion and cancellation copy.
- Download progress and completion polling messages now update only after the active transfer token is confirmed.
- Start-screen action panels no longer stretch their centered content below the viewport when side panels are taller.
- Start-screen trust copy, quick-guide steps, live headings, and QR instructions now reflect per-transfer approval overrides.
- Settings and startup now report global hotkey registration failures instead of silently accepting a broken shortcut.
- Per-transfer receive options now warn when upload limits exceed the 2 GB secure default even if the saved setting is also higher.
- Settings changes no longer restart an active local server onto localhost when no private LAN adapter is detected.

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
