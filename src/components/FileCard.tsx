import type { ShareInfo, ShareStatusInfo } from "../lib/api";
import { AppIcon } from "./AppIcon";

interface FileCardProps {
  share: ShareInfo;
  status: ShareStatusInfo | null;
  onChooseDifferent: () => void;
}

export function FileCard({ share, status, onChooseDifferent }: FileCardProps) {
  return (
    <section className="panel file-panel">
      <div className="panel-heading">
        <div className="panel-title-with-icon">
          <span className="feature-icon compact">
            <AppIcon name={share.is_archive ? "folder" : "files"} size={19} />
          </span>
          <div>
            <span className="eyebrow">{share.is_archive ? "Streaming archive" : "Selected file"}</span>
            <h2 className="file-name">{status?.file_name ?? share.file_name}</h2>
          </div>
        </div>
        <button className="subtle-button compact-button" type="button" onClick={onChooseDifferent}>
          Replace
        </button>
      </div>
      <dl className="meta-grid">
        <div>
          <dt>Size</dt>
          <dd>{status?.file_size_human ?? share.file_size_human}</dd>
        </div>
        <div>
          <dt>Type</dt>
          <dd>{status?.mime_type ?? share.mime_type}</dd>
        </div>
        {(status?.is_archive ?? share.is_archive) ? (
          <div>
            <dt>Files</dt>
            <dd>{status?.file_count ?? share.file_count}</dd>
          </div>
        ) : null}
      </dl>
    </section>
  );
}
