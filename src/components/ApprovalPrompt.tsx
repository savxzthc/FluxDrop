import { useEffect, useState } from "react";
import { formatDuration } from "../lib/format";

interface ApprovalPromptProps {
  direction: "download" | "upload";
  clientIp: string | null;
  fileName: string;
  fileSizeHuman: string;
  files?: Array<{ file_name: string; size: number }>;
  approvalDeadline?: string | null;
  busy: boolean;
  onApprove: () => void;
  onDeny: () => void;
}

export function ApprovalPrompt({
  direction,
  clientIp,
  fileName,
  fileSizeHuman,
  files,
  approvalDeadline,
  busy,
  onApprove,
  onDeny
}: ApprovalPromptProps) {
  const isUpload = direction === "upload";
  const [now, setNow] = useState(Date.now());
  const deadlineMs = approvalDeadline ? Date.parse(approvalDeadline) : null;
  const remainingSeconds = deadlineMs ? Math.max(0, Math.ceil((deadlineMs - now) / 1000)) : 60;
  const urgencyPercent = Math.max(0, Math.min(100, (remainingSeconds / 60) * 100));

  useEffect(() => {
    if (!approvalDeadline) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [approvalDeadline]);

  return (
    <section className="approval-banner" role="alertdialog" aria-labelledby="approval-title">
      <div>
        <span className="eyebrow">{isUpload ? "Upload approval required" : "Download approval required"}</span>
        <h2 id="approval-title">{isUpload ? "Allow this phone to upload?" : "Allow this phone to download?"}</h2>
        <p>
          <strong>{clientIp ?? "Unknown device"}</strong> requested{" "}
          <strong>{fileName}</strong> ({fileSizeHuman}).
        </p>
        {isUpload && files && files.length > 1 ? (
          <p className="approval-manifest" title={files.map((file) => file.file_name).join("\n")}>
            {files.slice(0, 4).map((file) => file.file_name).join(", ")}
            {files.length > 4 ? `, and ${files.length - 4} more` : ""}
          </p>
        ) : null}
        <div className="approval-countdown">
          <span>{remainingSeconds > 0 ? `Request expires in ${formatDuration(remainingSeconds)}` : "Request expired"}</span>
          <i>
            <b style={{ width: `${urgencyPercent}%` }} />
          </i>
        </div>
      </div>
      <div className="approval-actions">
        <button className="danger-button" type="button" onClick={onDeny} disabled={busy}>
          Deny
        </button>
        <button className="approve-button" type="button" onClick={onApprove} disabled={busy || remainingSeconds === 0}>
          {isUpload ? "Approve upload" : "Approve download"}
        </button>
      </div>
    </section>
  );
}
