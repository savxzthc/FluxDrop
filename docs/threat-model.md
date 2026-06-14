# Threat Model

## Assets

- The file being shared
- The file path on the user's PC
- The user's local network topology
- The PC-selected destination folder for incoming uploads

## Trust Assumptions

- The user's PC is trusted
- The local network permits direct device-to-device connections
- The phone belongs to the user and is trusted

## Attackers

Passive LAN observer: can observe the generic HTTP certificate-onboarding request, but the secure target and token are stored in the URL fragment and are not sent to that listener. Transfer metadata and file bytes then use HTTPS, preventing passive plaintext capture.

Active LAN attacker: can try to guess the URL. A 160-bit token makes brute force computationally infeasible, and per-IP rate limiting adds friction for invalid attempts. An attacker capable of intercepting or rewriting LAN traffic may also substitute onboarding content or another self-signed certificate; FluxDrop does not provide a public trust chain or pinned device identity.

Link-sharing mistake: a user may accidentally expose the QR code or URL. FluxDrop mitigates with configurable 5/10/30/60-minute expiration, single-use completion by default, and explicit PC approval by default.

XSS via filename: a malicious filename could be reflected into HTML. FluxDrop escapes all dynamic HTML values.

Header injection via filename: a crafted filename could include CRLF. FluxDrop strips control characters before building `Content-Disposition`.

Path traversal: an attacker could try to request arbitrary paths. FluxDrop never places file paths in URLs and only serves the canonical file selected at share creation.

Upload path traversal: an untrusted phone filename could contain separators or `..`. FluxDrop reduces it to a sanitized basename before joining it to the canonical PC-selected destination folder.

Oversized upload / disk exhaustion: a phone could claim or send excessive data. FluxDrop rejects metadata above the configured limit, checks `Content-Length` when present, and stops the stream when the byte counter crosses the limit. Available disk space can still change during a legitimate upload, so write failures are handled and the temp file is removed.

Partial or conflicting uploads: network interruption must not leave a plausible final filename, and a new upload must not overwrite an existing file. FluxDrop writes to a randomized `.part` file, syncs it, and uses no-clobber persistence to a collision-safe final name.

Approval bypass: the phone submits filename and size metadata first. The multipart endpoint independently revalidates the token and approval state, then requires the actual filename and byte count to match the approved metadata.

Directory listing: an attacker could try to browse files. FluxDrop has no listing routes.

Local history disclosure: the history file contains filenames, phone IP addresses, timestamps, and original source or destination paths needed for repeat actions. It deliberately excludes tokens, transfer URLs, certificates, and file contents. Anyone with access to the same Windows account may be able to read this metadata, so the desktop UI provides a clear-history control.

History tampering: history is not an authorization source. A repeat action revalidates that saved local paths still exist, then creates a new transfer session with a fresh random token and current approval, expiration, and single-use settings.

## Out Of Scope For v0.1

- Internet-accessible attackers; FluxDrop does not open router ports
- Physical access to the PC
- Malicious apps on the PC itself
- Malware scanning of files chosen by the user for upload; FluxDrop transports bytes but does not execute or inspect them

## Known Limitations

- TLS is self-signed and therefore encrypts traffic without independently authenticating the PC
- The generic HTTP onboarding page can be tampered with by an active LAN attacker
- Anyone who obtains a valid link before expiry can request a transfer, although approval is required by default
- VPN and client-isolation detection is heuristic
