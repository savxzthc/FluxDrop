import { useState } from "react";
import type { ReceiveInfo } from "../lib/api";
import { AppIcon } from "./AppIcon";

export function ReceiveQrCard({ receive }: { receive: ReceiveInfo }) {
  const [copied, setCopied] = useState(false);

  async function copyLink() {
    await navigator.clipboard.writeText(receive.upload_url);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
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
      <div className="qr-code" dangerouslySetInnerHTML={{ __html: receive.qr_svg }} />
      <div className="qr-instruction">
        <span>1</span>
        <p>Scan, choose a file on your phone, then approve it on this PC.</p>
      </div>
      <div className="link-row">
        <p className="download-url">{receive.upload_url}</p>
        <button className="icon-button" type="button" onClick={copyLink} aria-label="Copy receive link">
          {copied ? <AppIcon name="check" size={18} /> : "Copy"}
        </button>
      </div>
      <p className="certificate-note">First visit may ask you to accept FluxDrop&apos;s local certificate.</p>
    </section>
  );
}
