# FluxDrop HTTP Protocol

All routes are served by the PC over local HTTP. Every response includes `Content-Security-Policy`, `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`, `Permissions-Policy`, `Cache-Control: no-store`, and `Pragma: no-cache`.

## GET /

Reachability page. Returns minimal HTML stating that FluxDrop is running. It does not include file metadata, file paths, or tokens.

## GET /health

Returns:

```json
{ "status": "ok" }
```

No share information is exposed.

## GET /d/:token

Mobile landing page. The token must be URL-safe and match the active in-memory share. Valid requests set the share status to `PhoneConnected`, record the client IP, and emit `phone_connected`.

Success response: `200 text/html`, self-contained mobile page with escaped file name, human file size, MIME type, sender name, privacy note, and a download link.

Error responses:

- `404 Link Not Found` for invalid links
- `410 Link Expired` for expired or already completed links
- `410 Transfer Cancelled` for cancelled links
- `429 Too Many Attempts` for rate-limited invalid token attempts

## GET /api/share/:token

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

## GET /download/:token

Streams the file. The token must be valid, unexpired, not cancelled, not already completed, and approved if approval mode is enabled in a future version. Valid requests set status to `Downloading`, emit `download_started`, stream chunks from disk, emit throttled `progress_updated`, and emit `download_completed` after EOF.

Download headers include:

- `Content-Type: <detected MIME type>`
- `Content-Length: <file size>`
- `Content-Disposition: attachment; filename*=UTF-8''<encoded filename>`
- `X-Content-Type-Options: nosniff`
- `Cache-Control: no-store`
- `Pragma: no-cache`

## HEAD /download/:token

Performs the same validation as `GET /download/:token` and returns download headers without a body.

## Error Meanings

- `not_found`: token missing, malformed, or not the current share token
- `expired`: token timed out or was already used
- `cancelled`: sender cancelled the transfer
- `approval_required`: future approval mode state
- `rate_limited`: too many invalid token attempts from one IP
