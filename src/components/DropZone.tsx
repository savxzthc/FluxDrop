import { useRef } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AppIcon } from "./AppIcon";

interface DropZoneProps {
  active: boolean;
  disabled?: boolean;
  disabledCopy?: string;
  disabledTitle?: string;
  error: string | null;
  onActiveChange: (active: boolean) => void;
  onPaths: (paths: string[]) => void;
  onError: (message: string) => void;
}

export function DropZone({
  active,
  disabled = false,
  disabledCopy = "FluxDrop is inspecting your selection.",
  disabledTitle = "Preparing link",
  error,
  onActiveChange,
  onPaths,
  onError
}: DropZoneProps) {
  const dragDepth = useRef(0);

  async function chooseFiles() {
    if (disabled) return;
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        title: "Choose files to send with FluxDrop"
      });
      if (typeof selected === "string") {
        onPaths([selected]);
      } else if (Array.isArray(selected) && selected.length > 0) {
        onPaths(selected);
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  async function chooseFolder() {
    if (disabled) return;
    try {
      const selected = await open({
        multiple: false,
        directory: true,
        title: "Choose a folder to send as a ZIP"
      });
      if (typeof selected === "string") {
        onPaths([selected]);
      }
    } catch (err) {
      onError(err instanceof Error ? err.message : String(err));
    }
  }

  function onDragEnter(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    if (disabled) return;
    dragDepth.current += 1;
    onActiveChange(true);
  }

  function onDragOver(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    if (!disabled) onActiveChange(true);
  }

  function onDragLeave(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    if (disabled) return;
    dragDepth.current = Math.max(0, dragDepth.current - 1);
    if (dragDepth.current === 0) onActiveChange(false);
  }

  function onDrop(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    dragDepth.current = 0;
    onActiveChange(false);
    if (disabled) return;
    const files = Array.from(event.dataTransfer.files);
    if (files.length === 0) {
      onError("Drop at least one file or folder.");
      return;
    }

    const paths = files.map((file) => (file as File & { path?: string }).path).filter((path): path is string => Boolean(path));
    if (paths.length !== files.length) {
      onError("This drop did not expose a file path. Use Choose file if the webview blocks drag paths.");
      return;
    }
    onPaths(paths);
  }

  return (
    <section
      className={`drop-zone ${active ? "drop-zone-active" : ""} ${disabled ? "drop-zone-disabled" : ""}`}
      aria-disabled={disabled}
      onDragEnter={onDragEnter}
      onDragOver={onDragOver}
      onDragLeave={onDragLeave}
      onDrop={onDrop}
    >
      <div className="drop-visual">
        <span className="drop-icon">
          <AppIcon name="files" size={34} />
        </span>
        <p className="drop-title">{disabled ? disabledTitle : active ? "Drop to share" : "Drop files or a folder"}</p>
        <p className="drop-copy">{disabled ? disabledCopy : "Drag items here, or choose them from this PC."}</p>
      </div>
      <div className="drop-actions">
        <button className="primary-button" type="button" onClick={chooseFiles} disabled={disabled}>
          <AppIcon name="files" size={18} />
          Choose files
        </button>
        <button className="subtle-button" type="button" onClick={chooseFolder} disabled={disabled}>
          <AppIcon name="folder" size={18} />
          Choose folder
        </button>
      </div>
      <div className="drop-footnote">
        <span>Folders and multiple files become one streaming ZIP</span>
        <span>Large files stay memory efficient</span>
      </div>
      {error ? <p className="inline-error">{error}</p> : null}
    </section>
  );
}
