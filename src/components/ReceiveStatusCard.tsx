import type { ReceiveInfo, ReceiveStatusInfo } from "../lib/api";
import { formatBytes, statusCopy } from "../lib/format";

interface ReceiveStatusCardProps {
  receive: ReceiveInfo;
  status: ReceiveStatusInfo | null;
}

export function ReceiveStatusCard({ receive, status }: ReceiveStatusCardProps) {
  const current = status?.status ?? receive.status;
  const copy = statusCopy(current.kind, current.kind === "Error" ? current.message : undefined);
  const total = status?.file_size ?? 0;
  const received = status?.bytes_received ?? 0;
  const percent = status?.progress_percent ?? 0;

  return (
    <section className={`panel status-panel tone-${copy.tone}`}>
      <div className="status-row">
        <div>
          <span className="eyebrow">Receive status</span>
          <h2>{copy.label}</h2>
        </div>
        <span className="status-pill">{copy.label}</span>
      </div>
      <p>{copy.detail}</p>
      {current.kind === "Uploading" || current.kind === "Completed" ? (
        <div className="progress-wrap">
          <div className="progress-bar" aria-label="Upload progress">
            <span style={{ width: `${percent}%` }} />
          </div>
          <div className="progress-stats">
            <span>
              {formatBytes(received)} / {formatBytes(total)}
            </span>
            <span>{percent.toFixed(1)}%</span>
          </div>
        </div>
      ) : null}
      <dl className="status-grid">
        <div>
          <dt>Destination</dt>
          <dd>{status?.destination_folder_name ?? receive.destination_folder_name}</dd>
        </div>
        <div>
          <dt>Maximum size</dt>
          <dd>{receive.max_upload_size_human}</dd>
        </div>
        <div>
          <dt>Phone IP</dt>
          <dd>{status?.client_ip ?? "Not connected"}</dd>
        </div>
        <div>
          <dt>Incoming file</dt>
          <dd>{status?.file_name ?? "Not selected"}</dd>
        </div>
      </dl>
      {status?.last_request_status ? <p className="request-note">{status.last_request_status}</p> : null}
    </section>
  );
}
