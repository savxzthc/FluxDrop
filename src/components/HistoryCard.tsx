import { useMemo, useState } from "react";
import { AppIcon } from "./AppIcon";
import type { HistoryEntry, TransferOutcome } from "../lib/api";
import { copyTextToClipboard } from "../lib/clipboard";
import { formatBytes } from "../lib/format";

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

type DirectionFilter = "all" | "send" | "receive";
type OutcomeFilter = "all" | TransferOutcome;

export function HistoryCard({ entries, busyId, clearing, onRepeat, onClear }: HistoryCardProps) {
  const [directionFilter, setDirectionFilter] = useState<DirectionFilter>("all");
  const [outcomeFilter, setOutcomeFilter] = useState<OutcomeFilter>("all");
  const [query, setQuery] = useState("");
  const [copyMessage, setCopyMessage] = useState<string | null>(null);
  const filteredEntries = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return entries.filter((entry) => {
      if (directionFilter !== "all" && entry.direction !== directionFilter) return false;
      if (outcomeFilter !== "all" && entry.outcome !== outcomeFilter) return false;
      if (!normalizedQuery) return true;
      return [entry.file_name, entry.client_ip ?? "", entry.file_size_human ?? ""]
        .join(" ")
        .toLowerCase()
        .includes(normalizedQuery);
    });
  }, [directionFilter, entries, outcomeFilter, query]);
  const stats = useMemo(() => summarizeHistory(entries), [entries]);

  function requestClear() {
    if (window.confirm("Clear all local transfer history metadata from this PC?")) onClear();
  }

  async function copySummary() {
    const copied = await copyTextToClipboard(
      buildHistorySummary(entries, filteredEntries, stats, directionFilter, outcomeFilter, query)
    );
    setCopyMessage(copied ? "History summary copied." : "Could not copy history summary.");
    window.setTimeout(() => setCopyMessage(null), 1800);
  }

  async function copyCsv() {
    const copied = await copyTextToClipboard(buildHistoryCsv(filteredEntries));
    setCopyMessage(copied ? "Filtered history CSV copied." : "Could not copy history CSV.");
    window.setTimeout(() => setCopyMessage(null), 1800);
  }

  return (
    <section className="history-panel">
      <div className="history-panel-header">
        <div>
          <span className="eyebrow">Stored on this PC</span>
          <h2>Recent transfers</h2>
          <p>FluxDrop keeps up to 100 metadata records. Transfer tokens and file contents are never saved.</p>
        </div>
        <div className="history-actions">
          <button
            className="subtle-button history-clear"
            type="button"
            disabled={entries.length === 0}
            onClick={() => void copySummary()}
          >
            Copy summary
          </button>
          <button
            className="subtle-button history-clear"
            type="button"
            disabled={filteredEntries.length === 0}
            onClick={() => void copyCsv()}
          >
            Copy CSV
          </button>
          <button
            className="subtle-button history-clear"
            type="button"
            disabled={entries.length === 0 || clearing}
            onClick={requestClear}
          >
            <AppIcon name="trash" size={17} />
            {clearing ? "Clearing..." : "Clear history"}
          </button>
        </div>
      </div>
      {copyMessage ? <p className="history-message" aria-live="polite">{copyMessage}</p> : null}

      {entries.length === 0 ? (
        <div className="history-empty">
          <span className="feature-icon">
            <AppIcon name="history" size={23} />
          </span>
          <h3>No transfers yet</h3>
          <p>Completed, denied, cancelled, expired, and failed transfers will appear here.</p>
        </div>
      ) : (
        <>
          <div className="history-stats" aria-label="Transfer history summary">
            <HistoryStat label="Transfers" value={String(stats.total)} detail={`${stats.sent} sent | ${stats.received} received`} />
            <HistoryStat label="Completed" value={`${stats.completionRate}%`} detail={`${stats.completed} successful`} />
            <HistoryStat label="Data moved" value={formatBytes(stats.bytesMoved)} detail="Known completed transfer size" />
            <HistoryStat label="Repeatable" value={String(stats.repeatable)} detail="Original location still available" />
          </div>
          <div className="history-toolbar">
            <div className="segmented-control" aria-label="Filter transfer direction">
              {(["all", "send", "receive"] as const).map((value) => (
                <button
                  key={value}
                  className={directionFilter === value ? "active" : ""}
                  type="button"
                  onClick={() => setDirectionFilter(value)}
                >
                  {value === "all" ? "All" : value === "send" ? "Sent" : "Received"}
                </button>
              ))}
            </div>
            <select
              className="history-filter-select"
              value={outcomeFilter}
              aria-label="Filter transfer outcome"
              onChange={(event) => setOutcomeFilter(event.target.value as OutcomeFilter)}
            >
              <option value="all">All outcomes</option>
              {Object.entries(OUTCOME_COPY).map(([value, label]) => (
                <option key={value} value={value}>
                  {label}
                </option>
              ))}
            </select>
            <input
              className="history-search"
              type="search"
              value={query}
              placeholder="Search file or phone IP"
              onChange={(event) => setQuery(event.target.value)}
            />
          </div>
          {filteredEntries.length === 0 ? (
            <div className="history-empty history-empty-compact">
              <span className="feature-icon">
                <AppIcon name="history" size={23} />
              </span>
              <h3>No matching transfers</h3>
              <p>Try a different search, direction, or outcome filter.</p>
            </div>
          ) : (
            <div className="history-list">
              {filteredEntries.map((entry) => (
                <article className="history-row" key={entry.id}>
                  <span className={`history-direction ${entry.direction}`}>
                    <AppIcon name={entry.direction === "send" ? "send" : "receive"} size={19} />
                  </span>
                  <div className="history-file">
                    <strong>{entry.file_name}</strong>
                    <span>{historyMeta(entry)}</span>
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
                    title={entry.can_repeat ? undefined : entry.repeat_unavailable_reason ?? "This transfer cannot be repeated."}
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
        </>
      )}
    </section>
  );
}

function HistoryStat({ label, value, detail }: { label: string; value: string; detail: string }) {
  return (
    <div className="history-stat">
      <span>{label}</span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </div>
  );
}

function summarizeHistory(entries: HistoryEntry[]) {
  const total = entries.length;
  const completedEntries = entries.filter((entry) => entry.outcome === "completed");
  const completed = completedEntries.length;
  const sent = entries.filter((entry) => entry.direction === "send").length;
  const received = entries.filter((entry) => entry.direction === "receive").length;
  const repeatable = entries.filter((entry) => entry.can_repeat).length;
  const bytesMoved = completedEntries.reduce((sum, entry) => sum + (entry.file_size ?? 0), 0);
  const completionRate = total === 0 ? 0 : Math.round((completed / total) * 100);
  return { bytesMoved, completed, completionRate, received, repeatable, sent, total };
}

function buildHistorySummary(
  entries: HistoryEntry[],
  filteredEntries: HistoryEntry[],
  stats: ReturnType<typeof summarizeHistory>,
  directionFilter: DirectionFilter,
  outcomeFilter: OutcomeFilter,
  query: string
) {
  const lines = [
    "FluxDrop transfer history summary",
    `Generated: ${new Date().toISOString()}`,
    `Transfers: ${stats.total}`,
    `Completed: ${stats.completed} (${stats.completionRate}%)`,
    `Sent: ${stats.sent}`,
    `Received: ${stats.received}`,
    `Data moved: ${formatBytes(stats.bytesMoved)}`,
    `Repeatable: ${stats.repeatable}`,
    "",
    "Filters",
    `- Direction: ${directionFilter}`,
    `- Outcome: ${outcomeFilter}`,
    `- Query: ${query.trim() || "none"}`,
    "",
    `Visible entries: ${filteredEntries.length} of ${entries.length}`,
    ...filteredEntries.slice(0, 25).map((entry) =>
      `- ${formatHistoryDate(entry.finished_at)} | ${entry.direction} | ${OUTCOME_COPY[entry.outcome]} | ${entry.file_name} | ${entry.file_size_human ?? "unknown size"} | ${entry.client_ip ?? "no phone IP"}`
    )
  ];
  if (filteredEntries.length > 25) lines.push(`- ${filteredEntries.length - 25} more entries omitted`);
  return lines.join("\n");
}

function buildHistoryCsv(entries: HistoryEntry[]) {
  const rows = [
    [
      "finished_at",
      "direction",
      "outcome",
      "file_name",
      "file_size_human",
      "file_count",
      "is_archive",
      "client_ip",
      "can_repeat",
      "repeat_unavailable_reason"
    ],
    ...entries.map((entry) => [
      entry.finished_at,
      entry.direction,
      OUTCOME_COPY[entry.outcome],
      entry.file_name,
      entry.file_size_human ?? "",
      String(entry.file_count),
      entry.is_archive ? "yes" : "no",
      entry.client_ip ?? "",
      entry.can_repeat ? "yes" : "no",
      entry.repeat_unavailable_reason ?? ""
    ])
  ];
  return rows.map((row) => row.map(csvCell).join(",")).join("\n");
}

function csvCell(value: string) {
  const trimmed = value.trimStart();
  const safeValue = /^[=+\-@]/.test(trimmed) ? `'${value}` : value;
  return `"${safeValue.replace(/"/g, '""')}"`;
}

function historyMeta(entry: HistoryEntry) {
  return [
    entry.direction === "send" ? "Sent to phone" : "Received from phone",
    entry.file_size_human,
    entry.file_count > 1 ? `${entry.file_count} files` : null
  ]
    .filter(Boolean)
    .join(" | ");
}

function formatHistoryDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit"
  }).format(new Date(value));
}
