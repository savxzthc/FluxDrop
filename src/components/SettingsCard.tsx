import { useEffect, useState } from "react";
import type { AppSettings, NetworkAddress } from "../lib/api";

interface SettingsCardProps {
  settings: AppSettings;
  addresses: NetworkAddress[];
  onSave: (settings: AppSettings) => Promise<void>;
}

export function SettingsCard({ settings, addresses, onSave }: SettingsCardProps) {
  const [draft, setDraft] = useState(settings);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  useEffect(() => setDraft(settings), [settings]);

  async function save() {
    setSaving(true);
    setMessage(null);
    try {
      await onSave(draft);
      setMessage("Settings saved.");
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  }

  return (
    <section className="panel settings-panel">
      <div className="panel-heading">
        <div>
          <span className="eyebrow">Defaults and appearance</span>
          <h2>FluxDrop preferences</h2>
        </div>
        <button className="secondary-button" type="button" onClick={() => void save()} disabled={saving}>
          {saving ? "Saving..." : "Save settings"}
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
              [4 * 1024 * 1024 * 1024, "4 GB"]
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
      {message ? <p className="settings-message">{message}</p> : null}
    </section>
  );
}
