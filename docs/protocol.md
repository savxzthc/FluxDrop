# FluxDrop Local Transfer Protocol

FluxDrop runs two LAN listeners:

- A self-signed HTTPS listener serves every token-bearing transfer route.
- A separate HTTP listener serves only the generic certificate onboarding page and health response.

The QR code places the percent-encoded HTTPS transfer URL after `#` in the HTTP onboarding URL. Browser fragments are not sent in HTTP requests, so the onboarding listener never receives the share token. Same-origin JavaScript validates that the destination is HTTPS, uses the same hostname, has an explicit port, and matches a FluxDrop `/d/:token` or `/u/:token` path before enabling navigation.

Every response includes `Content-Security-Policy`, `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, `Permissions-Policy`, `Cache-Control: no-store`, and `Pragma: no-cache`.

New shares use the persisted desktop defaults: 5, 10, 30, or 60 minutes of validity; single-use or reusable-until-expiry behavior; and approval required or disabled. Tauri callers may pass those values as per-share overrides.

## HTTP GET /connect

Returns a generic page explaining the expected self-signed certificate warning. It contains no token, filename, destination, or secure URL. `/connect.js` reads the secure URL from the browser fragment and validates it before enabling the Continue securely link.

## HTTP GET /health

Returns `{ "status": "ok" }` without share data.

## HTTPS GET /

Reachability page. Returns minimal HTML stating that FluxDrop is running. It does not include file metadata, file paths, or tokens.

## HTTPS GET /health

Returns:

```json
{ "status": "ok" }
```

No share information is exposed.

## HTTPS GET /d/:token

Mobile landing page. The token must be URL-safe and match the active in-memory share. With approval enabled, the first valid request sets the share status to `AwaitingApproval`, records the client IP, emits `approval_requested`, and starts a 60-second timeout.

Success response: `200 text/html`, self-contained mobile page with escaped file name, human file size, MIME type, sender name, and either a waiting state or an approved download link. The waiting page refreshes itself without JavaScript.

Error responses:

- `404 Link Not Found` for invalid links
- `410 Link Expired` for expired or already completed links
- `410 Transfer Cancelled` for cancelled links
- `403 Transfer Denied` for manually denied requests
- `408 Approval Timed Out` when the PC does not respond within 60 seconds
- `429 Too Many Attempts` for rate-limited invalid token attempts

## HTTPS GET /api/share/:token

Returns safe metadata for a valid token.

```json
{
  "app": "FluxDrop",
  "file_name": "example.pdf",
  "file_size": 2048576,
  "file_size_human": "2.0 MB",
  "mime_type": "application/pdf",
  "expires_at": "2026-05-31T12:10:00Z",
  "single_use": true,
  "sender_name": "Massimo-PC",
  "status": { "kind": "Ready" }
}
```

Invalid or expired tokens return a generic error:

```json
{
  "error": "not_found",
  "message": "This link is invalid or has expired."
}
```

## HTTPS GET /download/:token

Streams the file. The token must be valid, unexpired, not cancelled, not already completed, and approved when approval mode is enabled. Valid requests set status to `Downloading`, emit `download_started`, stream chunks from disk, emit throttled `progress_updated`, and emit `download_completed` after EOF.

Download headers include:

- `Content-Type: <detected MIME type>`
- `Content-Length: <file size>`
- `Content-Disposition: attachment; filename*=UTF-8''<encoded filename>`
- `X-Content-Type-Options: nosniff`
- `Cache-Control: no-store`
- `Pragma: no-cache`

## HTTPS HEAD /download/:token

Performs the same validation as `GET /download/:token` and returns download headers without a body.

## HTTPS GET /u/:token

Serves the self-contained Receive mode page. The page loads only the same-origin `/upload.js` helper under CSP. Selecting a file does not upload its bytes yet.

## HTTPS POST /api/upload-request/:token

Accepts JSON metadata containing `file_name`, `file_size`, and optional `mime_type`. FluxDrop sanitizes the filename, rejects files above the configured limit, records the phone IP, transitions to `AwaitingApproval`, and emits `upload_approval_requested`. Approval times out after 60 seconds.

## HTTPS GET /api/upload-status/:token

Returns the current receive status so the phone can wait for `Approved`, `Denied`, or a timeout without sending file bytes.

## HTTPS POST /upload/:token

Accepts one streamed multipart field named `file` only after approval. FluxDrop:

- checks `Content-Length` early when present
- enforces the byte limit while streaming
- requires the multipart filename and final byte count to match the approved metadata
- writes into a randomized `.part` file inside the selected destination folder
- flushes and syncs the file before no-clobber persistence under the sanitized name
- removes incomplete temp files on failure
- emits upload progress and completion events

## Error Meanings

- `not_found`: token missing, malformed, or not the current share token
- `expired`: token timed out or was already used
- `cancelled`: sender cancelled the transfer
- `approval_required`: the phone must wait for an explicit PC decision
- `denied`: the PC denied the request
- `approval_timed_out`: the PC did not respond within 60 seconds
- `rate_limited`: too many invalid token attempts from one IP

## TLS Lifecycle

On first server start, FluxDrop generates a private key and self-signed certificate whose subject alternative name is the selected LAN IP. The PEM files are stored in the app configuration directory and reused while that IP remains selected. Changing the LAN IP regenerates the certificate before the listener restarts.

The self-signed certificate encrypts traffic but provides no independently trusted identity. It protects against passive observation after the browser proceeds, but not an active attacker capable of intercepting onboarding or substituting another self-signed endpoint.
