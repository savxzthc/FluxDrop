import { open } from "@tauri-apps/plugin-dialog";
import { AppIcon } from "./AppIcon";

interface ReceiveSetupCardProps {
  disabled?: boolean;
  disabledCopy?: string;
  disabledTitle?: string;
  error: string | null;
  onStart: (folder: string) => void;
  onError: (message: string) => void;
}

export function ReceiveSetupCard({
  disabled = false,
  disabledCopy = "FluxDrop is opening the local upload server.",
  disabledTitle = "Preparing receive link",
  error,
  onStart,
  onError
}: ReceiveSetupCardProps) {
  async function chooseDestination() {
    if (disabled) return;
    try {
      const selected = await open({
        multiple: false,
        directory: true,
        title: "Choose where phone uploads should be saved"
      });
      if (typeof selected === "string") onStart(selected);
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  return (
    <section className={`drop-zone receive-setup ${disabled ? "drop-zone-disabled" : ""}`} aria-disabled={disabled}>
      <div className="drop-visual">
        <span className="drop-icon receive-icon">
          <AppIcon name="receive" size={34} />
        </span>
        <p className="drop-title">{disabled ? disabledTitle : "Choose a destination folder"}</p>
        <p className="drop-copy">{disabled ? disabledCopy : "FluxDrop creates a private upload page for your phone."}</p>
      </div>
      <div className="drop-actions">
        <button className="primary-button" type="button" onClick={() => void chooseDestination()} disabled={disabled}>
          <AppIcon name="folder" size={18} />
          Choose destination
        </button>
      </div>
      <div className="drop-footnote">
        <span>You approve the complete file manifest and total size</span>
        <span>Partial uploads are cleaned automatically</span>
      </div>
      {error ? <p className="inline-error">{error}</p> : null}
    </section>
  );
}
