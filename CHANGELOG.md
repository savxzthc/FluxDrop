# Changelog

All notable changes to FluxDrop will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - Unreleased

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
