use super::{ReceiveSnapshot, ShareSnapshot};
use crate::file_utils::{escape_html, format_file_size};
use crate::share::ShareStatus;

pub(super) const ROOT_HTML: &str = "<!doctype html><html><head><meta charset=\"utf-8\"><title>FluxDrop</title></head><body><h1>FluxDrop is running</h1><p>The local transfer server is ready.</p></body></html>";
pub(super) fn mobile_download_html(snapshot: &ShareSnapshot) -> String {
    let file_name = escape_html(&snapshot.safe_file_name);
    let file_size = escape_html(&snapshot.file_size_human);
    let mime_type = escape_html(&snapshot.mime_type);
    let sender = escape_html(&snapshot.sender_name);
    let token = escape_html(&snapshot.token);
    let (refresh, action, intro, note) = match snapshot.status {
        ShareStatus::AwaitingApproval => (
            r#"<meta http-equiv="refresh" content="2">"#,
            r#"<div class="waiting">Waiting for approval on the PC...</div>"#.to_string(),
            format!(
                "{sender} received your request and must approve it before the download starts."
            ),
            "This page refreshes automatically. Approval requests time out after 60 seconds."
                .to_string(),
        ),
        ShareStatus::Approved => (
            "",
            format!(r#"<a class="button" href="/download/{token}">Download approved file</a>"#),
            format!("{sender} approved this download."),
            "The link expires automatically after the configured time.".to_string(),
        ),
        _ if !snapshot.approval_required => (
            "",
            format!(r#"<a class="button" href="/download/{token}">Download</a>"#),
            format!(
                "{sender} is sharing {} with this browser over local Wi-Fi.",
                if snapshot.is_archive {
                    format!("{} files in a ZIP archive", snapshot.file_count)
                } else {
                    "one file".to_string()
                }
            ),
            "This link expires automatically. Use FluxDrop only on trusted networks.".to_string(),
        ),
        _ => (
            r#"<meta http-equiv="refresh" content="2">"#,
            r#"<div class="waiting">Contacting the PC...</div>"#.to_string(),
            format!("{sender} is preparing this transfer."),
            "This page refreshes automatically.".to_string(),
        ),
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  {refresh}
  <title>FluxDrop Download</title>
  <style>
    :root {{ color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; box-sizing: border-box; }}
    main {{ width: min(100%, 460px); background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.65rem; }}
    p {{ color: #4b5563; line-height: 1.5; }}
    dl {{ display: grid; grid-template-columns: auto 1fr; gap: 10px 14px; margin: 24px 0; }}
    dt {{ color: #6b7280; }}
    dd {{ margin: 0; font-weight: 650; overflow-wrap: anywhere; }}
    a.button {{ display: block; text-align: center; padding: 15px 18px; border-radius: 7px; background: #0f172a; color: #fff; text-decoration: none; font-weight: 750; }}
    .waiting {{ padding: 15px 18px; border-radius: 7px; background: #fef3c7; color: #92400e; text-align: center; font-weight: 750; }}
    .note {{ font-size: .92rem; color: #6b7280; margin-bottom: 0; }}
  </style>
</head>
<body>
  <main>
    <h1>FluxDrop</h1>
    <p>{intro}</p>
    <dl>
      <dt>File</dt><dd>{file_name}</dd>
      <dt>Size</dt><dd>{file_size}</dd>
      <dt>Type</dt><dd>{mime_type}</dd>
    </dl>
    {action}
    <p class="note">{note}</p>
  </main>
</body>
</html>"#
    )
}

pub(super) fn mobile_upload_html(snapshot: &ReceiveSnapshot) -> String {
    let token = escape_html(&snapshot.token);
    let max_size = escape_html(&format_file_size(snapshot.max_upload_bytes));
    let max_upload_bytes = snapshot.max_upload_bytes;
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Upload to FluxDrop</title>
  <style>
    :root {{ color-scheme: light; font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; box-sizing: border-box; }}
    main {{ width: min(100%, 460px); background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.65rem; }}
    p {{ color: #4b5563; line-height: 1.5; }}
    form {{ display: grid; gap: 14px; margin-top: 22px; }}
    input {{ width: 100%; padding: 13px; box-sizing: border-box; border: 1px solid #cbd5e1; border-radius: 7px; }}
    button {{ padding: 15px 18px; border: 0; border-radius: 7px; background: #0f172a; color: #fff; font: inherit; font-weight: 750; }}
    button:disabled {{ opacity: .6; }}
    #status {{ padding: 12px; border-radius: 7px; background: #f8fafc; color: #334155; }}
  </style>
</head>
<body data-token="{token}" data-max-upload-bytes="{max_upload_bytes}">
  <main>
    <h1>Send a file to this PC</h1>
    <p>Select one file. FluxDrop sends its name and size to the PC for approval before the upload begins.</p>
    <p>Maximum upload size: <strong>{max_size}</strong></p>
    <form id="upload-form">
      <input id="file-input" name="file" type="file" required>
      <button id="upload-button" type="submit">Request upload</button>
    </form>
    <p id="status" aria-live="polite">Waiting for a file selection.</p>
  </main>
  <script src="/upload.js" defer></script>
</body>
</html>"#
    )
}

pub(super) const ONBOARDING_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Connect securely to FluxDrop</title>
  <style>
    :root { color-scheme: light; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; }
    body { margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; background: #f6f7f9; color: #111827; }
    main { width: min(100%, 470px); box-sizing: border-box; background: #fff; border: 1px solid #d7dce3; border-radius: 12px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }
    .eyebrow { color: #1d4ed8; font-size: .78rem; font-weight: 800; letter-spacing: .08em; text-transform: uppercase; }
    h1 { margin: 10px 0 12px; font-size: 1.65rem; line-height: 1.2; }
    p, li { color: #4b5563; line-height: 1.55; }
    ol { padding-left: 22px; }
    .button { display: block; margin-top: 22px; padding: 13px 18px; border-radius: 8px; background: #1d4ed8; color: #fff; font-weight: 800; text-align: center; text-decoration: none; }
    .button[hidden] { display: none; }
    .note { margin-top: 18px; padding: 12px 14px; border-radius: 8px; background: #eff6ff; color: #1e3a8a; font-size: .92rem; }
    .error { color: #b91c1c; font-weight: 700; }
  </style>
  <script src="/connect.js" defer></script>
</head>
<body>
  <main>
    <span class="eyebrow">Encrypted local transfer</span>
    <h1>One browser confirmation is required</h1>
    <p>FluxDrop encrypts this transfer with a certificate generated by your PC. Because it is self-signed, your phone cannot verify it automatically.</p>
    <ol>
      <li>Tap <strong>Continue securely</strong>.</li>
      <li>On the browser warning, tap <strong>Advanced</strong>.</li>
      <li>Choose <strong>Proceed</strong> to open FluxDrop.</li>
    </ol>
    <a id="continue-link" class="button" href="#" rel="noreferrer" hidden>Continue securely</a>
    <p id="status" class="note">Preparing the encrypted connection...</p>
    <p class="note">Only continue when this QR code came from the FluxDrop PC you expect. Self-signed HTTPS blocks passive Wi-Fi snooping, but it does not prove the PC's identity.</p>
  </main>
</body>
</html>"##;

// The token-bearing HTTPS URL stays in the fragment so the HTTP onboarding
// request never transmits the transfer token, even in server logs.
pub(super) const ONBOARDING_SCRIPT: &str = r#"(() => {
  "use strict";
  const link = document.getElementById("continue-link");
  const status = document.getElementById("status");

  try {
    const encodedTarget = window.location.hash.slice(1);
    if (!encodedTarget) throw new Error("This setup link is incomplete. Scan the current FluxDrop QR code again.");
    const target = new URL(decodeURIComponent(encodedTarget));
    const validPath = /^\/(?:d|u)\/[A-Za-z0-9_-]{27}$/.test(target.pathname);
    if (target.protocol !== "https:" || target.hostname !== window.location.hostname || !target.port || !validPath) {
      throw new Error("This setup link is invalid. Scan the current FluxDrop QR code again.");
    }
    link.href = target.toString();
    link.hidden = false;
    status.textContent = "Ready. The next page is the encrypted FluxDrop connection.";
  } catch (error) {
    status.classList.add("error");
    status.textContent = error instanceof Error ? error.message : "This setup link is invalid.";
  }
})();"#;

pub(super) const UPLOAD_SCRIPT: &str = r#"(() => {
  const token = document.body.dataset.token;
  const form = document.getElementById("upload-form");
  const input = document.getElementById("file-input");
  const button = document.getElementById("upload-button");
  const status = document.getElementById("status");
  const maxUploadBytes = Number(document.body.dataset.maxUploadBytes || "0");
  let active = false;

  const setStatus = (message) => { status.textContent = message; };
  const wait = (ms) => new Promise((resolve) => window.setTimeout(resolve, ms));
  const errorMessages = {
    approval_required: "The PC still needs to approve this upload.",
    approval_timed_out: "Approval timed out. Start a new receive link and try again.",
    cancelled: "The PC cancelled this receive link.",
    client_mismatch: "This approval belongs to a different phone. Scan a fresh QR code from this device.",
    denied: "The PC denied this upload.",
    expired: "This receive link has expired or was already used.",
    invalid_multipart: "The phone could not package this file correctly. Try selecting it again.",
    metadata_mismatch: "The selected file changed after approval. Choose it again and retry.",
    missing_file: "No file was included in the upload.",
    missing_filename: "Choose a file with a valid name.",
    not_found: "This receive link is invalid or has expired.",
    rate_limited: "Too many invalid attempts were received. Wait a minute and try again.",
    request_in_progress: "The PC is already handling an upload request for this link.",
    size_mismatch: "The file size changed during upload. Choose it again and retry.",
    store_failed: "The PC could not safely store this upload. Try a different destination folder.",
    temp_unavailable: "The PC could not create a temporary upload file.",
    too_large: "This file is larger than the receive limit.",
    upload_failed: "The upload did not complete.",
    write_failed: "The PC could not write the incoming file."
  };

  function formatBytes(bytes) {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"];
    let size = bytes;
    let index = 0;
    while (size >= 1024 && index < units.length - 1) {
      size /= 1024;
      index += 1;
    }
    return index === 0 ? `${bytes} B` : `${size.toFixed(1)} ${units[index]}`;
  }

  function selectedFileTooLarge(file) {
    return maxUploadBytes > 0 && file.size > maxUploadBytes;
  }

  async function readApiError(response, fallback) {
    const payload = await response.json().catch(() => null);
    if (payload && typeof payload.message === "string" && payload.message.trim()) {
      return payload.message;
    }
    const code = payload && typeof payload.error === "string" ? payload.error : "";
    return errorMessages[code] || `${fallback} (${response.status})`;
  }

  input.addEventListener("change", () => {
    if (active) return;
    const file = input.files && input.files[0];
    if (!file) {
      button.disabled = false;
      setStatus("Waiting for a file selection.");
      return;
    }
    if (selectedFileTooLarge(file)) {
      button.disabled = true;
      setStatus(`This file is ${formatBytes(file.size)}, which exceeds the ${formatBytes(maxUploadBytes)} receive limit.`);
      return;
    }
    button.disabled = false;
    setStatus(`Ready to request approval for ${file.name} (${formatBytes(file.size)}).`);
  });

  async function pollForApproval(file) {
    for (;;) {
      await wait(1000);
      const response = await fetch(`/api/upload-status/${encodeURIComponent(token)}`, { cache: "no-store" });
      if (!response.ok) {
        throw new Error(await readApiError(response, "The receive link is no longer available."));
      }
      const current = await response.json();
      if (current.status.kind === "Approved") return upload(file);
      if (current.status.kind === "Denied") {
        throw new Error(current.approval_timed_out ? errorMessages.approval_timed_out : errorMessages.denied);
      }
      if (current.status.kind === "Cancelled" || current.status.kind === "Expired") {
        throw new Error(errorMessages.expired);
      }
      setStatus("Waiting for approval on the PC...");
    }
  }

  async function upload(file) {
    setStatus("Approved. Uploading to the PC...");
    const body = new FormData();
    body.append("file", file, file.name);
    const response = await fetch(`/upload/${encodeURIComponent(token)}`, { method: "POST", body });
    if (!response.ok) {
      throw new Error(await readApiError(response, "Upload failed"));
    }
    setStatus("Upload complete. The file is safely stored on the PC.");
    button.textContent = "Uploaded";
  }

  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (active) return;
    const file = input.files && input.files[0];
    if (!file) return;
    if (selectedFileTooLarge(file)) {
      setStatus(`This file is ${formatBytes(file.size)}, which exceeds the ${formatBytes(maxUploadBytes)} receive limit.`);
      return;
    }
    active = true;
    button.disabled = true;
    try {
      setStatus("Sending file details to the PC...");
      const response = await fetch(`/api/upload-request/${encodeURIComponent(token)}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ file_name: file.name, file_size: file.size, mime_type: file.type || null })
      });
      if (!response.ok) {
        throw new Error(await readApiError(response, "The PC could not accept this upload request"));
      }
      setStatus("Waiting for approval on the PC...");
      await pollForApproval(file);
    } catch (error) {
      setStatus(error instanceof Error ? error.message : String(error));
      button.disabled = false;
      active = false;
    }
  });
})();"#;

pub(super) fn error_html(title: &str, message: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{}</title>
  <style>
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; padding: 24px; font-family: ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif; background: #f6f7f9; color: #111827; }}
    main {{ max-width: 440px; background: #fff; border: 1px solid #d7dce3; border-radius: 8px; padding: 28px; box-shadow: 0 20px 60px rgba(15, 23, 42, .12); }}
    h1 {{ margin: 0 0 8px; font-size: 1.5rem; }}
    p {{ margin-bottom: 0; color: #4b5563; line-height: 1.5; }}
  </style>
</head>
<body><main><h1>{}</h1><p>{}</p></main></body>
</html>"#,
        escape_html(title),
        escape_html(title),
        escape_html(message)
    )
}
