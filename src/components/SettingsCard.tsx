import { useEffect, useState } from "react";
import type { AppSettings, NetworkAddress, UpdateInfo } from "../lib/api";
import { copyTextToClipboard } from "../lib/clipboard";
import { formatBytes } from "../lib/format";

interface SettingsCardProps {
  settings: AppSettings;
  addresses: NetworkAddress[];
  onForgetHistoryLocations: () => Promise<number>;
  onSave: (settings: AppSettings) => Promise<void>;
  updateInfo: UpdateInfo | null;
  updateBusy: boolean;
  updateMessage: string | null;
  onCheckUpdates: () => Promise<void>;
  onApplyUpdate: () => Promise<void>;
}

const SECURE_DEFAULTS: AppSettings = {
  expiration_minutes: 10,
  single_use: true,
  approval_required: true,
  preferred_lan_ip: null,
  max_upload_bytes: 2 * 1024 * 1024 * 1024,
  theme: "system",
  shell_integration: false,
  global_hotkey: false,
  remember_transfer_locations: true,
  automatic_updates: true
};

type SettingsMessage = { tone: "success" | "error"; text: string };

export function SettingsCard({
  settings,
  addresses,
  onForgetHistoryLocations,
  onSave,
  updateInfo,
  updateBusy,
  updateMessage,
  onCheckUpdates,
  onApplyUpdate
}: SettingsCardProps) {
  const [draft, setDraft] = useState(settings);
  const [saving, setSaving] = useState(false);
  const [forgettingLocations, setForgettingLocations] = useState(false);
  const [message, setMessage] = useState<SettingsMessage | null>(null);
  const dirty = !sameSettings(draft, settings);
  const riskItems = settingsRisks(draft);

  useEffect(() => {
    setDraft(settings);
  }, [settings]);

  async function save() {
    setSaving(true);
    setMessage(null);
    try {
      await onSave(draft);
      setMessage({ tone: "success", text: "Settings saved." });
    } catch (err) {
      setMessage({ tone: "error", text: err instanceof Error ? err.message : String(err) });
    } finally {
      setSaving(false);
    }
  }

  async function copyDiagnostics() {
    const diagnostics = buildDiagnostics(draft, addresses, dirty, riskItems);
    const copied = await copyTextToClipboard(diagnostics);
    setMessage(
      copied
        ? { tone: "success", text: "Diagnostics copied without transfer tokens or file contents." }
        : { tone: "error", text: "Could not copy diagnostics. Select the settings manually." }
    );
  }

  async function forgetLocations() {
    if (!window.confirm("Forget saved local file and folder paths from existing transfer history? Metadata stays, but those entries cannot be repeated.")) {
      return;
    }
    setForgettingLocations(true);
    setMessage(null);
    try {
      const changed = await onForgetHistoryLocations();
      setMessage({
        tone: "success",
        text:
          changed === 0
            ? "No saved history locations needed scrubbing."
            : `Forgot saved locations for ${changed} history record${changed === 1 ? "" : "s"}.`
      });
    } catch (err) {
      setMessage({ tone: "error", text: err instanceof Error ? err.message : String(err) });
    } finally {
      setForgettingLocations(false);
    }
  }

  return (
    <section className="panel settings-panel">
      <div className="panel-heading">
        <div>
          <span className="eyebrow">Defaults and appearance</span>
          <h2>FluxDrop preferences</h2>
        </div>
        <div className="settings-actions">
          <button className="subtle-button compact-button" type="button" onClick={() => void copyDiagnostics()}>
            Copy diagnostics
          </button>
          <button
            className="subtle-button compact-button"
            type="button"
            onClick={() => {
              setDraft(settings);
              setMessage(null);
            }}
            disabled={!dirty || saving}
          >
            Discard
          </button>
          <button
            className="secondary-button"
            type="button"
            onClick={() => void save()}
            disabled={saving || !dirty}
          >
            {saving ? "Saving..." : dirty ? "Save settings" : "Saved"}
          </button>
        </div>
      </div>
      <div className="settings-health">
        <div>
          <span className={`settings-health-dot ${riskItems.length === 0 ? "ready" : "warning"}`} />
          <div>
            <strong>{riskItems.length === 0 ? "Secure defaults are active" : "Review security and behavior tradeoffs"}</strong>
            <p>
              {riskItems.length === 0
                ? "Approval, one-time links, and a short lifetime are all enabled."
                : riskItems.join(" ")}
            </p>
          </div>
        </div>
        <button
          className="subtle-button compact-button"
          type="button"
          onClick={() =>
            setDraft({
              ...SECURE_DEFAULTS,
              theme: draft.theme,
              preferred_lan_ip: draft.preferred_lan_ip,
              remember_transfer_locations: draft.remember_transfer_locations,
              automatic_updates: draft.automatic_updates
            })
          }
          disabled={saving}
        >
          Restore secure defaults
        </button>
      </div>
      <div className="settings-privacy-tools">
        <div>
          <span className="eyebrow">Signed updates</span>
          <strong>
            {updateInfo?.available
              ? `FluxDrop ${updateInfo.version} is available`
              : `FluxDrop ${updateInfo?.current_version ?? "update status"}`}
          </strong>
          <p>
            {updateMessage ??
              (updateInfo?.available
                ? updateInfo.portable
                  ? "Portable builds report updates and open the release page without modifying themselves."
                  : updateInfo.downloaded
                    ? "The signed update was downloaded and verified. Installation waits for your confirmation."
                    : "The signed update can be downloaded when you choose to install it."
                : "Checks GitHub-hosted signed release metadata. Installed builds can update automatically.")}
          </p>
        </div>
        <div className="settings-actions">
          <button className="subtle-button compact-button" type="button" onClick={() => void onCheckUpdates()} disabled={updateBusy}>
            {updateBusy ? "Checking..." : "Check now"}
          </button>
          {updateInfo?.available ? (
            <button className="secondary-button" type="button" onClick={() => void onApplyUpdate()} disabled={updateBusy}>
              {updateInfo.portable ? "Open download page" : "Install update"}
            </button>
          ) : null}
        </div>
      </div>
      <div className="settings-privacy-tools">
        <div>
          <span className="eyebrow">History privacy</span>
          <strong>Scrub saved repeat locations</strong>
          <p>Keep transfer metadata, outcomes, sizes, and phone IPs while removing local file and folder paths from existing history.</p>
        </div>
        <button
          className="danger-button compact-button"
          type="button"
          onClick={() => void forgetLocations()}
          disabled={forgettingLocations}
        >
          {forgettingLocations ? "Forgetting..." : "Forget saved locations"}
        </button>
      </div>
      <div className="settings-grid">
        <label>
          <span>Appearance</span>
          <select
            value={draft.theme}
            onChange={(event) =>
              setDraft({ ...draft, theme: event.target.value as AppSettings["theme"] })
            }
          >
            <option value="system">Use Windows setting</option>
            <option value="light">Light</option>
            <option value="dark">Dark</option>
          </select>
          <small>System follows the Windows light or dark appearance automatically.</small>
        </label>
        <label>
          <span>Link expiration</span>
          <select
            value={draft.expiration_minutes}
            onChange={(event) => setDraft({ ...draft, expiration_minutes: Number(event.target.value) })}
          >
            {[5, 10, 30, 60].map((minutes) => (
              <option key={minutes} value={minutes}>
                {minutes} minutes
              </option>
            ))}
          </select>
        </label>
        <fieldset className="adapter-fieldset">
          <legend>Preferred LAN adapter</legend>
          <label className="adapter-option">
            <input
              type="radio"
              name="preferred-adapter"
              checked={draft.preferred_lan_ip === null}
              onChange={() => setDraft({ ...draft, preferred_lan_ip: null })}
            />
            <span>
              <strong>Automatic selection</strong>
              <small>Use FluxDrop's private-LAN heuristic.</small>
            </span>
          </label>
          {addresses.map((address) => (
            <label className="adapter-option" key={`${address.interface_name}-${address.ip}`}>
              <input
                type="radio"
                name="preferred-adapter"
                checked={draft.preferred_lan_ip === address.ip}
                onChange={() => setDraft({ ...draft, preferred_lan_ip: address.ip })}
              />
              <span>
                <strong>
                  {address.interface_name} - {address.ip}
                </strong>
                <small>{address.reason}</small>
              </span>
            </label>
          ))}
        </fieldset>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.automatic_updates}
            onChange={(event) => setDraft({ ...draft, automatic_updates: event.target.checked })}
          />
          <span>
            <strong>Automatic updates</strong>
            <small>Installed builds download signed updates in the background and ask before restarting.</small>
          </span>
        </label>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.single_use}
            onChange={(event) => setDraft({ ...draft, single_use: event.target.checked })}
          />
          <span>
            <strong>Single-use links</strong>
            <small>Invalidate the link after one completed transfer.</small>
          </span>
        </label>
        <label>
          <span>Maximum phone upload size</span>
          <select
            value={draft.max_upload_bytes}
            onChange={(event) => setDraft({ ...draft, max_upload_bytes: Number(event.target.value) })}
          >
            {[
              [512 * 1024 * 1024, "512 MB"],
              [1024 * 1024 * 1024, "1 GB"],
              [2 * 1024 * 1024 * 1024, "2 GB"],
              [4 * 1024 * 1024 * 1024, "4 GB"],
              [8 * 1024 * 1024 * 1024, "8 GB"],
              [16 * 1024 * 1024 * 1024, "16 GB"]
            ].map(([bytes, label]) => (
              <option key={bytes} value={bytes}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.approval_required}
            onChange={(event) => setDraft({ ...draft, approval_required: event.target.checked })}
          />
          <span>
            <strong>Require PC approval</strong>
            <small>Recommended. The phone waits until this PC approves the transfer.</small>
          </span>
        </label>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.remember_transfer_locations}
            onChange={(event) => setDraft({ ...draft, remember_transfer_locations: event.target.checked })}
          />
          <span>
            <strong>Remember locations for repeat actions</strong>
            <small>When disabled, future history keeps metadata only and cannot restart old transfers.</small>
          </span>
        </label>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.shell_integration}
            onChange={(event) => setDraft({ ...draft, shell_integration: event.target.checked })}
          />
          <span>
            <strong>Right-click &quot;Send with FluxDrop&quot;</strong>
            <small>Add a Send shortcut to the Windows Explorer menu for files and folders.</small>
          </span>
        </label>
        <label className="toggle-row">
          <input
            type="checkbox"
            checked={draft.global_hotkey}
            onChange={(event) => setDraft({ ...draft, global_hotkey: event.target.checked })}
          />
          <span>
            <strong>Global hotkey</strong>
            <small>Press Ctrl + Shift + D anywhere to bring FluxDrop to the front.</small>
          </span>
        </label>
      </div>
      {message ? <p className={`settings-message ${message.tone === "error" ? "settings-message-error" : ""}`}>{message.text}</p> : null}
    </section>
  );
}

function sameSettings(a: AppSettings, b: AppSettings) {
  return (
    a.expiration_minutes === b.expiration_minutes &&
    a.single_use === b.single_use &&
    a.approval_required === b.approval_required &&
    a.preferred_lan_ip === b.preferred_lan_ip &&
    a.max_upload_bytes === b.max_upload_bytes &&
    a.theme === b.theme &&
    a.shell_integration === b.shell_integration &&
    a.global_hotkey === b.global_hotkey &&
    a.remember_transfer_locations === b.remember_transfer_locations &&
    a.automatic_updates === b.automatic_updates
  );
}

function settingsRisks(settings: AppSettings) {
  const risks: string[] = [];
  if (!settings.approval_required) risks.push("PC approval is disabled.");
  if (!settings.single_use) risks.push("Links can be reused until they expire.");
  if (settings.expiration_minutes > 10) risks.push("Links stay open longer than the default window.");
  if (settings.max_upload_bytes > SECURE_DEFAULTS.max_upload_bytes) {
    risks.push("Phone uploads can exceed the default 2 GB limit.");
  }
  return risks;
}

function buildDiagnostics(
  settings: AppSettings,
  addresses: NetworkAddress[],
  dirty: boolean,
  riskItems: string[]
) {
  const lines = [
    "FluxDrop diagnostics",
    `Generated: ${new Date().toISOString()}`,
    `Unsaved settings: ${dirty ? "yes" : "no"}`,
    "",
    "Settings",
    `- Theme: ${settings.theme}`,
    `- Expiration: ${settings.expiration_minutes} minutes`,
    `- Single-use links: ${settings.single_use ? "enabled" : "disabled"}`,
    `- PC approval: ${settings.approval_required ? "required" : "disabled"}`,
    `- Preferred LAN IP: ${settings.preferred_lan_ip ?? "automatic"}`,
    `- Max upload: ${formatBytes(settings.max_upload_bytes)}`,
    `- Explorer integration: ${settings.shell_integration ? "enabled" : "disabled"}`,
    `- Global hotkey: ${settings.global_hotkey ? "enabled" : "disabled"}`,
    `- Remember transfer locations: ${settings.remember_transfer_locations ? "enabled" : "disabled"}`,
    `- Automatic updates: ${settings.automatic_updates ? "enabled" : "disabled"}`,
    "",
    "Detected LAN addresses",
    ...(addresses.length > 0
      ? addresses.map((address) => `- ${address.ip} | ${address.interface_name} | ${address.reason}`)
      : ["- none"]),
    "",
    "Security notes",
    ...(riskItems.length > 0 ? riskItems.map((risk) => `- ${risk}`) : ["- secure defaults active"])
  ];
  return lines.join("\n");
}
