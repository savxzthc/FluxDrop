import { open } from "@tauri-apps/plugin-dialog";

interface DropZoneProps {
  active: boolean;
  error: string | null;
  onActiveChange: (active: boolean) => void;
  onFilePath: (path: string) => void;
  onError: (message: string) => void;
}

export function DropZone({ active, error, onActiveChange, onFilePath, onError }: DropZoneProps) {
  async function chooseFile() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose a file to send with FluxDrop"
    });
    if (typeof selected === "string") {
      onFilePath(selected);
    }
  }

  function onDrop(event: React.DragEvent<HTMLDivElement>) {
    event.preventDefault();
    onActiveChange(false);
    const files = Array.from(event.dataTransfer.files);
    if (files.length !== 1) {
      onError("Drop exactly one file. Folders and multiple files are planned for later versions.");
      return;
    }

    const path = (files[0] as File & { path?: string }).path;
    if (!path) {
      onError("This drop did not expose a file path. Use Choose file if the webview blocks drag paths.");
      return;
    }
    onFilePath(path);
  }

  return (
    <section
      className={`drop-zone ${active ? "drop-zone-active" : ""}`}
      onDragEnter={(event) => {
        event.preventDefault();
        onActiveChange(true);
      }}
      onDragOver={(event) => {
        event.preventDefault();
        onActiveChange(true);
      }}
      onDragLeave={(event) => {
        event.preventDefault();
        onActiveChange(false);
      }}
      onDrop={onDrop}
    >
      <div>
        <p className="drop-title">{active ? "Drop to share" : "Drop one file here"}</p>
        <p className="drop-copy">or choose a file from this PC</p>
      </div>
      <button className="primary-button" type="button" onClick={chooseFile}>
        Choose file
      </button>
      {error ? <p className="inline-error">{error}</p> : null}
    </section>
  );
}
