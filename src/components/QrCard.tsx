import { useState } from "react";
import type { ShareInfo } from "../lib/api";

interface QrCardProps {
  share: ShareInfo;
}

export function QrCard({ share }: QrCardProps) {
  const [copied, setCopied] = useState(false);

  async function copyLink() {
    await navigator.clipboard.writeText(share.download_url);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  }

  return (
    <section className="panel qr-panel">
      <span className="eyebrow">Scan with phone camera</span>
      <div className="qr-code" dangerouslySetInnerHTML={{ __html: share.qr_svg }} />
      <p className="download-url">{share.download_url}</p>
      <button className="secondary-button" type="button" onClick={copyLink}>
        {copied ? "Copied!" : "Copy link"}
      </button>
    </section>
  );
}
