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

Pull-request CI focuses on Rust formatting, linting, tests, audits, and frontend builds. Version tags additionally run the Windows release workflow to produce NSIS and MSI installers.

## v0.2

- [x] PC approval mode before download
- [x] Better LAN adapter selection UI
- [x] Windows NSIS/MSI installers and tag-driven GitHub Releases
- Better firewall diagnostics
- [x] Native-style desktop shell with dedicated Send, Receive, and Settings workspaces
- [x] Persisted light, dark, and system-following themes
- [x] Configurable expiration time
- [x] Configurable single-use toggle
- [x] Persisted desktop settings panel
- [x] System tray support with close-to-tray behavior

## v0.3

- [x] Multiple files as an on-the-fly ZIP
- [x] Folder sending as an on-the-fly ZIP
- [x] Persisted transfer history with repeat and clear actions
- More robust rate limiting
- macOS support

## v0.4

- [x] Local HTTPS using a per-LAN-IP self-signed certificate
- [x] Token-safe HTTP onboarding instructions for browser certificate acceptance
- Research into mTLS or certificate pinning for local use
- Trusted devices list and mDNS discovery are deferred: browser-only phones cannot reliably consume raw mDNS discovery without a companion client, and presence broadcasting adds a privacy surface that needs a separate opt-in design
- Clipboard and text snippet sharing
- Linux support

## Completed beyond the original roadmap

- [x] Phone-to-PC Receive mode with approval, progress, size limits, and safe temp-file finalization

Cloud sync is intentionally out of scope. Requests for cloud sync should be redirected because that is a different product.
## v0.4 reliability release

Delivered: resumable ordinary-file downloads, multi-file phone upload batches, Windows Firewall diagnosis/repair, and signed installed-build updates. Cloud transfer, accounts, mDNS discovery, macOS, and Linux remain deferred.
