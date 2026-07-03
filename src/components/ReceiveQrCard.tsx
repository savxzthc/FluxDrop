import { useState } from "react";
import type { ReceiveInfo } from "../lib/api";
import { copyTextToClipboard } from "../lib/clipboard";
import { AppIcon } from "./AppIcon";
import { ExpiryMeter } from "./ExpiryMeter";

export function ReceiveQrCard({ receive }: { receive: ReceiveInfo }) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const qrSrc = svgDataUri(receive.qr_svg);

  async function copyLink() {
    const copied = await copyTextToClipboard(receive.upload_url);
    setCopyState(copied ? "copied" : "failed");
    window.setTimeout(() => setCopyState("idle"), 1800);
  }

  return (
    <section className="panel qr-panel">
      <div className="qr-heading">
        <span className="feature-icon">
          <AppIcon name="phone" />
        </span>
        <div>
          <span className="eyebrow">Phone handoff</span>
          <h2>Scan to upload</h2>
        </div>
      </div>
      <div className="qr-code">
        <img className="qr-code-image" src={qrSrc} alt="QR code for the upload link" draggable={false} />
      </div>
      <div className="qr-instruction">
        <span>1</span>
        <p>
          {receive.approval_required
            ? "Scan, choose a file on your phone, then approve it on this PC."
            : "Scan, choose a file on your phone, and upload within the size limit."}
        </p>
      </div>
      <ExpiryMeter createdAt={receive.created_at} expiresAt={receive.expires_at} label="Upload link" />
      <div className="link-row">
        <p className="download-url">{receive.upload_url}</p>
        <button className="icon-button" type="button" onClick={copyLink} aria-label="Copy receive link">
          <AppIcon name={copyState === "copied" ? "check" : "copy"} size={18} />
        </button>
      </div>
      <p className={`copy-status ${copyState === "failed" ? "copy-status-error" : ""}`} aria-live="polite">
        {copyState === "copied" ? "Link copied." : copyState === "failed" ? "Copy failed. Select the link manually." : ""}
      </p>
      <p className="certificate-note">First visit may ask you to accept FluxDrop&apos;s local certificate.</p>
    </section>
  );
}

function svgDataUri(svg: string) {
  return `data:image/svg+xml;utf8,${encodeURIComponent(svg)}`;
}
