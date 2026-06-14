import { AppIcon } from "./AppIcon";
import type { HistoryEntry, TransferOutcome } from "../lib/api";

interface HistoryCardProps {
  entries: HistoryEntry[];
  busyId: string | null;
  clearing: boolean;
  onRepeat: (entry: HistoryEntry) => void;
  onClear: () => void;
}

const OUTCOME_COPY: Record<TransferOutcome, string> = {
  completed: "Completed",
  denied: "Denied",
  timed_out: "Timed out",
  cancelled: "Cancelled",
  expired: "Expired",
  failed: "Failed"
};

export function HistoryCard({ entries, busyId, clearing, onRepeat, onClear }: HistoryCardProps) {
  return (
    <section className="history-panel">
      <div className="history-panel-header">
        <div>
          <span className="eyebrow">Stored on this PC</span>
          <h2>Recent transfers</h2>
          <p>FluxDrop keeps up to 100 metadata records. Transfer tokens and file contents are never saved.</p>
        </div>
        <button
          className="subtle-button history-clear"
          type="button"
          disabled={entries.length === 0 || clearing}
          onClick={onClear}
        >
          <AppIcon name="trash" size={17} />
          {clearing ? "Clearing..." : "Clear history"}
        </button>
      </div>

      {entries.length === 0 ? (
        <div className="history-empty">
          <span className="feature-icon">
            <AppIcon name="history" size={23} />
          </span>
          <h3>No transfers yet</h3>
          <p>Completed, denied, cancelled, expired, and failed transfers will appear here.</p>
        </div>
      ) : (
        <div className="history-list">
          {entries.map((entry) => (
            <article className="history-row" key={entry.id}>
              <span className={`history-direction ${entry.direction}`}>
                <AppIcon name={entry.direction === "send" ? "send" : "receive"} size={19} />
              </span>
              <div className="history-file">
                <strong>{entry.file_name}</strong>
                <span>
                  {entry.direction === "send" ? "Sent to phone" : "Received from phone"}
                  {entry.file_size_human ? ` · ${entry.file_size_human}` : ""}
                  {entry.file_count > 1 ? ` · ${entry.file_count} files` : ""}
                </span>
              </div>
              <div className="history-device">
                <strong>{formatHistoryDate(entry.finished_at)}</strong>
                <span>{entry.client_ip ?? "No phone connected"}</span>
              </div>
              <span className={`history-outcome ${entry.outcome}`}>{OUTCOME_COPY[entry.outcome]}</span>
              <button
                className="subtle-button history-repeat"
                type="button"
                disabled={!entry.can_repeat || busyId !== null}
                title={entry.can_repeat ? undefined : "The original location is no longer available."}
                onClick={() => onRepeat(entry)}
              >
                <AppIcon name="repeat" size={16} />
                {busyId === entry.id
                  ? "Starting..."
                  : entry.direction === "send"
                    ? "Send again"
                    : "Receive again"}
              </button>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}

function formatHistoryDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit"
  }).format(new Date(value));
}
