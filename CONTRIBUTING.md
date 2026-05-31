# Contributing to FluxDrop

FluxDrop is intentionally narrow: send one file from a PC to a phone over local Wi-Fi, without accounts or cloud services. Contributions should preserve that product shape.

## Prerequisites

- Rust latest stable
- Node.js LTS
- Tauri CLI through `npm run tauri` or `cargo install tauri-cli`
- Optional: `cargo-audit`

## Setup

```powershell
git clone https://github.com/fluxdrop/fluxdrop.git
cd fluxdrop
npm install
```

## Development

```powershell
npm run tauri dev
```

## Build

```powershell
npm run build
cd src-tauri
cargo build
cd ..
npm run tauri build
```

## Required Checks

```powershell
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cd ..
npm run build
npm audit
cargo install cargo-audit
cd src-tauri
cargo audit
```

Formatting must pass, clippy must pass with no warnings, tests must pass, and frontend builds must succeed before a pull request is submitted.

## Pull Requests

Describe what changed, link related issues, include the verification commands you ran, and explain any security-relevant behavior changes. Keep public APIs explicit and avoid `unsafe`; if `unsafe` is genuinely required, include a `SAFETY:` comment explaining the invariant.

## Issue Labels

Common labels are `bug`, `security`, `enhancement`, `documentation`, and `good first issue`.

## Security Issues

Report security issues through a private GitHub security advisory, not a public issue.
