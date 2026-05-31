# Threat Model

## Assets

- The file being shared
- The file path on the user's PC
- The user's local network topology

## Trust Assumptions

- The user's PC is trusted
- The user's home or office Wi-Fi is trusted
- The phone belongs to the user and is trusted

## Attackers

Passive LAN observer: can intercept local HTTP traffic. FluxDrop reduces exposure with a short-lived one-time token, but v0.1 does not encrypt traffic.

Active LAN attacker: can try to guess the URL. A 160-bit token makes brute force computationally infeasible, and per-IP rate limiting adds friction for invalid attempts.

Link-sharing mistake: a user may accidentally expose the QR code or URL. FluxDrop mitigates with 10-minute expiration and single-use completion.

XSS via filename: a malicious filename could be reflected into HTML. FluxDrop escapes all dynamic HTML values.

Header injection via filename: a crafted filename could include CRLF. FluxDrop strips control characters before building `Content-Disposition`.

Path traversal: an attacker could try to request arbitrary paths. FluxDrop never places file paths in URLs and only serves the canonical file selected at share creation.

Directory listing: an attacker could try to browse files. FluxDrop has no listing routes.

## Out Of Scope For v0.1

- Internet-accessible attackers; FluxDrop does not open router ports
- Physical access to the PC
- Malicious apps on the PC itself
- Phone-to-PC upload attack surface, because upload is not implemented

## Known Limitations

- Local HTTP is not encrypted
- Anyone on the same LAN who obtains the link before expiry can download the file
- VPN and client-isolation detection is heuristic
