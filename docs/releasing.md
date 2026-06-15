# Releasing FluxDrop

FluxDrop publishes Windows binaries from GitHub Actions. A version tag creates a GitHub Release containing:

- an NSIS setup executable for normal installation;
- an MSI package for managed deployment;
- a portable executable that runs without installation;
- a SHA-256 checksum file for all published binaries.

## Prepare a release

1. Update the same semantic version in:
   - `package.json`
   - `src-tauri/tauri.conf.json`
   - `src-tauri/Cargo.toml`
2. Move the relevant entries from `CHANGELOG.md` into a dated release section.
3. Run the release checks:

```powershell
npm ci
npm run release:check
npm run build
cd src-tauri
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cd ..
```

4. Commit and push the release preparation.
5. Create and push a matching tag:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

The `Release` workflow validates the tag against all three manifests, builds the NSIS installer, MSI package, and portable executable on Windows, then publishes the GitHub Release.

## Code signing

The current pipeline produces unsigned installers. They work normally, but Windows SmartScreen may warn users because the publisher cannot be verified. Add a protected Windows code-signing certificate or a managed signing service before presenting FluxDrop as a trusted publisher.
