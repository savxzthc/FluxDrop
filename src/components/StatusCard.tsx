import type { ShareInfo, ShareStatusInfo } from "../lib/api";
import { formatBytes, formatDuration, statusCopy } from "../lib/format";
import { TransferTimeline } from "./TransferTimeline";

interface StatusCardProps {
  share: ShareInfo;
  status: ShareStatusInfo | null;
  speedBytesPerSecond: number;
}

export function StatusCard({ share, status, speedBytesPerSecond }: StatusCardProps) {
  const current = status?.status ?? share.status;
  const copy = statusCopy(current.kind, current.kind === "Error" ? current.message : undefined, "send");
  const bytesSent = status?.bytes_sent ?? 0;
  const total = status?.file_size ?? share.file_size;
  const percent = status?.progress_percent ?? 0;
  const started = status?.download_started_at ? Date.parse(status.download_started_at) : null;
  const finished = status?.download_finished_at ? Date.parse(status.download_finished_at) : null;
  const elapsedSeconds = started ? Math.max(0, Math.round(((finished ?? Date.now()) - started) / 1000)) : 0;

  return (
    <section className={`panel status-panel tone-${copy.tone}`}>
      <div className="status-row">
        <div>
          <span className="eyebrow">Transfer status</span>
          <h2>{copy.label}</h2>
        </div>
        <span className="status-pill">{copy.label}</span>
      </div>
      <p>{copy.detail}</p>
      <TransferTimeline direction="send" status={current} />
      {current.kind === "Downloading" || current.kind === "Completed" ? (
        <div className="progress-wrap">
          <div className="progress-bar" aria-label="Download progress">
            <span style={{ width: `${percent}%` }} />
          </div>
          <div className="progress-stats">
            <span>
              {formatBytes(bytesSent)} / {formatBytes(total)}
            </span>
            <span>{percent.toFixed(1)}%</span>
          </div>
        </div>
      ) : null}
      <dl className="status-grid">
        <div>
          <dt>Server</dt>
          <dd>{status?.local_address ?? `${share.local_ip}:${share.port}`}</dd>
        </div>
        <div>
          <dt>Speed</dt>
          <dd>{speedBytesPerSecond > 0 ? `${formatBytes(speedBytesPerSecond)}/s` : "Waiting"}</dd>
        </div>
        <div>
          <dt>Elapsed</dt>
          <dd>{formatDuration(elapsedSeconds)}</dd>
        </div>
        <div>
          <dt>Phone IP</dt>
          <dd>{status?.client_ip ?? "Not connected"}</dd>
        </div>
      </dl>
      {status?.last_request_status ? <p className="request-note">{status.last_request_status}</p> : null}
    </section>
  );
}
