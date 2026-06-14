interface ApprovalPromptProps {
  direction: "download" | "upload";
  clientIp: string | null;
  fileName: string;
  fileSizeHuman: string;
  busy: boolean;
  onApprove: () => void;
  onDeny: () => void;
}

export function ApprovalPrompt({
  direction,
  clientIp,
  fileName,
  fileSizeHuman,
  busy,
  onApprove,
  onDeny
}: ApprovalPromptProps) {
  const isUpload = direction === "upload";
  return (
    <section className="approval-banner" role="alertdialog" aria-labelledby="approval-title">
      <div>
        <span className="eyebrow">{isUpload ? "Upload approval required" : "Download approval required"}</span>
        <h2 id="approval-title">{isUpload ? "Allow this phone to upload?" : "Allow this phone to download?"}</h2>
        <p>
          <strong>{clientIp ?? "Unknown device"}</strong> requested{" "}
          <strong>{fileName}</strong> ({fileSizeHuman}).
          The request expires after 60 seconds.
        </p>
      </div>
      <div className="approval-actions">
        <button className="danger-button" type="button" onClick={onDeny} disabled={busy}>
          Deny
        </button>
        <button className="approve-button" type="button" onClick={onApprove} disabled={busy}>
          {isUpload ? "Approve upload" : "Approve download"}
        </button>
      </div>
    </section>
  );
}
