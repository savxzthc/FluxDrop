import type { AppSettings, CreateShareOptions, StartReceiveOptions } from "../lib/api";
import { formatBytes } from "../lib/format";
import { AppIcon } from "./AppIcon";

export interface TransferOptionsDraft {
  expiration_minutes: number;
  single_use: boolean;
  approval_required: boolean;
  max_upload_bytes: number;
}

interface TransferOptionsCardProps {
  direction: "send" | "receive";
  settings: AppSettings;
  value: TransferOptionsDraft;
  onChange: (next: TransferOptionsDraft) => void;
}

const EXPIRATION_OPTIONS = [5, 10, 30, 60];
const SECURE_DEFAULT_UPLOAD_BYTES = 2 * 1024 * 1024 * 1024;
const UPLOAD_SIZE_OPTIONS: Array<[number, string]> = [
  [512 * 1024 * 1024, "512 MB"],
  [1024 * 1024 * 1024, "1 GB"],
  [2 * 1024 * 1024 * 1024, "2 GB"],
  [4 * 1024 * 1024 * 1024, "4 GB"],
  [8 * 1024 * 1024 * 1024, "8 GB"],
  [16 * 1024 * 1024 * 1024, "16 GB"]
];

export function transferOptionsFromSettings(settings: AppSettings): TransferOptionsDraft {
  return {
    expiration_minutes: settings.expiration_minutes,
    single_use: settings.single_use,
    approval_required: settings.approval_required,
    max_upload_bytes: settings.max_upload_bytes
  };
}

export function shareOptionsFromDraft(options: TransferOptionsDraft): CreateShareOptions {
  return {
    expiration_minutes: options.expiration_minutes,
    single_use: options.single_use,
    approval_required: options.approval_required
  };
}

export function receiveOptionsFromDraft(options: TransferOptionsDraft): StartReceiveOptions {
  return {
    expiration_minutes: options.expiration_minutes,
    approval_required: options.approval_required,
    max_upload_bytes: options.max_upload_bytes
  };
}

export function TransferOptionsCard({ direction, settings, value, onChange }: TransferOptionsCardProps) {
  const sending = direction === "send";
  const risks = optionRisks(value, settings, direction);

  return (
    <aside className="panel transfer-options-panel">
      <div className="panel-title-with-icon">
        <span className="feature-icon compact">
          <AppIcon name="settings" size={18} />
        </span>
        <div>
          <span className="eyebrow">This transfer</span>
          <h2>Link options</h2>
        </div>
      </div>

      <div className="transfer-options-grid">
        <label>
          <span>Expires after</span>
          <select
            value={value.expiration_minutes}
            onChange={(event) => onChange({ ...value, expiration_minutes: Number(event.target.value) })}
          >
            {EXPIRATION_OPTIONS.map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutes} minutes
              </option>
            ))}
          </select>
        </label>

        {!sending ? (
          <label>
            <span>Upload limit</span>
            <select
              value={value.max_upload_bytes}
              onChange={(event) => onChange({ ...value, max_upload_bytes: Number(event.target.value) })}
            >
              {UPLOAD_SIZE_OPTIONS.map(([bytes, label]) => (
                <option key={bytes} value={bytes}>
                  {label}
                </option>
              ))}
            </select>
          </label>
        ) : null}

        <label className="option-toggle">
          <input
            type="checkbox"
            checked={value.approval_required}
            onChange={(event) => onChange({ ...value, approval_required: event.target.checked })}
          />
          <span>
            <strong>Require PC approval</strong>
            <small>{sending ? "Phone downloads wait for your decision." : "Phone uploads wait for your decision."}</small>
          </span>
        </label>

        {sending ? (
          <label className="option-toggle">
            <input
              type="checkbox"
              checked={value.single_use}
              onChange={(event) => onChange({ ...value, single_use: event.target.checked })}
            />
            <span>
              <strong>Single-use link</strong>
              <small>Expire after the first completed download.</small>
            </span>
          </label>
        ) : null}
      </div>

      <div className={`transfer-option-note ${risks.length > 0 ? "warning" : ""}`}>
        <strong>{risks.length > 0 ? "Review before sharing" : "Using secure transfer defaults"}</strong>
        <span>
          {risks.length > 0
            ? risks.join(" ")
            : sending
              ? "Approval, one-time use, and a short expiration are active."
              : `Approval and a ${formatBytes(value.max_upload_bytes)} upload limit are active.`}
        </span>
      </div>

      <button
        className="subtle-button compact-button"
        type="button"
        onClick={() => onChange(transferOptionsFromSettings(settings))}
      >
        Match saved settings
      </button>
    </aside>
  );
}

function optionRisks(
  options: TransferOptionsDraft,
  settings: AppSettings,
  direction: "send" | "receive"
) {
  const risks: string[] = [];
  if (!options.approval_required) risks.push("PC approval is disabled.");
  if (direction === "send" && !options.single_use) risks.push("The link can be reused until it expires.");
  if (options.expiration_minutes > settings.expiration_minutes) {
    risks.push("This link stays open longer than your saved default.");
  }
  if (direction === "receive" && options.max_upload_bytes > settings.max_upload_bytes) {
    risks.push("This receive link accepts larger uploads than your saved default.");
  }
  if (direction === "receive" && options.max_upload_bytes > SECURE_DEFAULT_UPLOAD_BYTES) {
    risks.push("This receive link accepts uploads above the 2 GB secure default.");
  }
  return risks;
}
