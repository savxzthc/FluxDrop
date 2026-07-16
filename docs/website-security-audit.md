# Website Security Audit

Date: 2026-07-07
Scope: `website/`, `site-dist/`, website build/deploy configuration, website audit tooling, and the static browser runtime served by `scripts/serve-secure-website.mjs`.

## Result

No exploitable issue remains in the audited static website surface after remediation.

The site is a static marketing/support/security website. There is no server-side application logic, authentication flow, form submission, cookie state, local storage, analytics, or external runtime dependency in the audited website. The meaningful attack surface is therefore:

- deployment headers and CSP enforcement
- accidental publishable artifacts in the web root
- DOM injection sinks and unsafe URL schemes
- third-party JavaScript/library exposure
- path traversal and hidden-file exposure in the local preview server
- public vulnerability-reporting metadata

## Changes Made

- Removed `website/website.zip` from the publishable web root.
- Added `website/*.zip`, `website/*.tar`, `website/*.tar.gz`, `website/*.tgz`, `website/*.rar`, and `website/*.7z` to `.gitignore`.
- Strengthened `scripts/audit-website.mjs` so the audit now fails if publishable source or build roots contain archives, source maps, logs, env files, SSH material, private keys, certificates, backups, temporary files, or special filesystem entries.
- Required `upgrade-insecure-requests` in website meta CSP checks.
- Added `upgrade-insecure-requests` to the meta CSP in `website/index.html`, `website/security.html`, and `website/support.html` to match deployment headers more closely.
- Added Apache-compatible shared-hosting controls in `website/public/.htaccess`: HTTPS redirect, directory listing disabled, first-party 404 handling, hardened response headers, artifact-deny rules, and `X-Powered-By` removal.
- Added a first-party `website/404.html` and `website/public/robots.txt` so missing paths and crawler requests do not fall through to provider-branded error redirects when the host honors `.htaccess`.
- Updated HSTS policy in `_headers`, `vercel.json`, and `.htaccess` to `max-age=31536000; includeSubDomains; preload`.

## Verified Controls

- Strict CSP is present in source pages, built pages, `_headers`, and `vercel.json`.
- CSP blocks default execution and network behavior with `default-src 'none'`, same-origin scripts/styles/images only, no connect targets, no objects, no frames, no workers, no forms, no base URI, Trusted Types enforcement, and insecure-request upgrades.
- Response headers include COEP, COOP, CORP, Origin-Agent-Cluster, Permissions-Policy, Referrer-Policy, HSTS, nosniff, DNS prefetch off, X-Frame-Options DENY, and X-Permitted-Cross-Domain-Policies none.
- The static build carries equivalent deploy policies for Netlify-style `_headers`, Vercel, and Apache-compatible shared hosting.
- The Apache deployment policy redirects HTTP to HTTPS, disables directory listings, serves `/404.html` as the local error page, and denies accidental archive, debug, secret, key, and certificate artifacts.
- All runtime assets are same-origin.
- No inline scripts, inline event handlers, inline style attributes, active embed elements, forms, unsafe URL schemes, external runtime assets, HTML comments, dangerous JavaScript sinks, CSS imports, external CSS URLs, or active SVG content were found by the audit script.
- External links using `target="_blank"` include `rel="noopener noreferrer"`.
- `/.well-known/security.txt` is present in the built site and points to GitHub private security advisories.
- Served preview blocks `/_headers`, `/.htaccess`, `/website.zip`, hidden paths outside `/.well-known/`, and encoded traversal probes.
- Headless Chrome renders `/`, `/security.html`, `/support.html`, and an XSS-payload URL without load/security errors or payload reflection.

## Verification Commands

```powershell
node --check scripts\audit-website.mjs
node --check scripts\serve-secure-website.mjs
npm run site:build
npm run site:audit
npm audit --audit-level=moderate
npx --yes retire --path site-dist --outputformat json
```

Additional live verification used `scripts/serve-secure-website.mjs` on `http://127.0.0.1:8766` and Chrome headless to verify expected 200/404 routes, security headers, CSP directives, clean rendering, and non-reflection of:

```text
?q=<img src=x onerror=alert(1)>#<script>alert(1)</script>
```

## Residual Risk

- The website depends on the deployment host applying one of `vercel.json`, `_headers`, or `.htaccess`. If a host ignores all response-header configuration, the meta CSP still helps, but `frame-ancestors`, HSTS, `X-Frame-Options`, COOP, COEP, CORP, and Permissions-Policy require response headers.
- InfinityFree/OpenResty TLS renegotiation behavior, the injected AES challenge, challenge cookie attributes, and shared-hosting open ports are provider-controlled. They cannot be corrected by static website source changes; fix them by changing hosting/TLS termination or disabling the provider challenge if the control panel supports it.
- HSTS only protects after a browser has seen the site over HTTPS on the deployed domain.
- HSTS preload requires successful live HTTPS deployment before submission to the browser preload list.
- GitHub release/download links leave this origin. Browser isolation headers and `rel="noopener noreferrer"` protect this site, but binary release integrity still depends on the GitHub release workflow and user-side OS checks.
- This audit covers the website, not the full desktop application protocol or Rust/Tauri runtime.
