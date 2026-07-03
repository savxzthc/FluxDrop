import { useState } from "react";
import type { ShareInfo } from "../lib/api";
import { copyTextToClipboard } from "../lib/clipboard";
import { AppIcon } from "./AppIcon";
import { ExpiryMeter } from "./ExpiryMeter";

interface QrCardProps {
  share: ShareInfo;
}

export function QrCard({ share }: QrCardProps) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");
  const qrSrc = svgDataUri(share.qr_svg);

  async function copyLink() {
    const copied = await copyTextToClipboard(share.download_url);
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
          <h2>Scan to download</h2>
        </div>
      </div>
      <div className="qr-code">
        <img className="qr-code-image" src={qrSrc} alt="QR code for the download link" draggable={false} />
      </div>
      <div className="qr-instruction">
        <span>1</span>
        <p>
          {share.approval_required
            ? "Scan with your phone camera, then approve the request on this PC."
            : "Scan with your phone camera to open the direct download link."}
        </p>
      </div>
      <ExpiryMeter createdAt={share.created_at} expiresAt={share.expires_at} label="Download link" />
      <div className="link-row">
        <p className="download-url">{share.download_url}</p>
        <button className="icon-button" type="button" onClick={copyLink} aria-label="Copy download link">
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
