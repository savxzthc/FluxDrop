import type { ShareInfo, ShareStatusInfo } from "../lib/api";

interface FileCardProps {
  share: ShareInfo;
  status: ShareStatusInfo | null;
  onChooseDifferent: () => void;
}

export function FileCard({ share, status, onChooseDifferent }: FileCardProps) {
  return (
    <section className="panel">
      <div className="panel-heading">
        <span className="eyebrow">Selected file</span>
        <button className="subtle-button" type="button" onClick={onChooseDifferent}>
          Choose different file
        </button>
      </div>
      <h2 className="file-name">{status?.file_name ?? share.file_name}</h2>
      <dl className="meta-grid">
        <div>
          <dt>Size</dt>
          <dd>{status?.file_size_human ?? share.file_size_human}</dd>
        </div>
        <div>
          <dt>Type</dt>
          <dd>{status?.mime_type ?? share.mime_type}</dd>
        </div>
      </dl>
    </section>
  );
}
