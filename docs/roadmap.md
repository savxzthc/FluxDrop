# Roadmap

## v0.1

- Single-file PC-to-phone sharing
- QR code
- One-time link
- Browser download page
- 10-minute token expiration
- Link cancellation
- Real-time progress in desktop UI
- Security headers
- Filename sanitization
- Basic rate limiting
- Heuristic LAN adapter filtering
- Self-contained mobile page
- Architecture, protocol, threat model, and security checklist documentation

Full Tauri binary builds are not included in CI yet. They require platform-specific runners and installer configuration; v0.1 CI focuses on Rust formatting, linting, tests, and frontend builds.

## v0.2

- PC approval mode before download
- Better LAN adapter selection UI
- Windows installer through Tauri
- Better firewall diagnostics
- UI polish pass
- Configurable expiration time
- Configurable single-use toggle

## v0.3

- Multiple files as an on-the-fly ZIP
- Folder sending as an on-the-fly ZIP
- Transfer history in the desktop app
- More robust rate limiting
- macOS support

## v0.4

- Optional local HTTPS using a self-signed certificate
- Research into mTLS or certificate pinning for local use
- Browser-compatible encryption research
- Trusted devices list
- Clipboard and text snippet sharing
- Linux support

Cloud sync is intentionally out of scope. Requests for cloud sync should be redirected because that is a different product.
