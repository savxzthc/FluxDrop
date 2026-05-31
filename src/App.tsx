import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import { DropZone } from "./components/DropZone";
import { FileCard } from "./components/FileCard";
import { HelpCard } from "./components/HelpCard";
import { QrCard } from "./components/QrCard";
import { SecurityCard } from "./components/SecurityCard";
import { StatusCard } from "./components/StatusCard";
import { cancelShare, createShare, getNetworkAddresses, getShareStatus, NetworkAddress, ShareInfo, ShareStatusInfo } from "./lib/api";

type SpeedSample = { time: number; bytes: number };

export default function App() {
  const [share, setShare] = useState<ShareInfo | null>(null);
  const [status, setStatus] = useState<ShareStatusInfo | null>(null);
  const [addresses, setAddresses] = useState<NetworkAddress[]>([]);
  const [dragActive, setDragActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [now, setNow] = useState(Date.now());
  const samples = useRef<SpeedSample[]>([]);

  useEffect(() => {
    getNetworkAddresses().then(setAddresses).catch(() => setAddresses([]));
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setNow(Date.now());
      if (share) {
        getShareStatus().then(setStatus).catch(() => undefined);
      }
    }, 1000);
    return () => window.clearInterval(timer);
  }, [share]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const subscribe = async () => {
      for (const eventName of ["phone_connected", "download_started", "progress_updated", "download_completed", "share_expired", "share_cancelled"]) {
        const unlisten = await listen<ShareStatusInfo>(eventName, (event) => setStatus(event.payload));
        unlisteners.push(unlisten);
      }
      const dropUnlisten = await listen<string[] | { paths?: string[] }>("tauri://drag-drop", (event) => {
        const payload = event.payload;
        const paths = Array.isArray(payload) ? payload : payload.paths ?? [];
        if (paths.length === 1) void beginShare(paths[0]);
      });
      unlisteners.push(dropUnlisten);
    };
    void subscribe();
    return () => unlisteners.forEach((unlisten) => unlisten());
  }, []);

  useEffect(() => {
    if (!status) return;
    samples.current.push({ time: Date.now(), bytes: status.bytes_sent });
    samples.current = samples.current.filter((sample) => Date.now() - sample.time <= 3500);
  }, [status?.bytes_sent]);

  const speedBytesPerSecond = useMemo(() => {
    if (samples.current.length < 2) return 0;
    const first = samples.current[0];
    const last = samples.current[samples.current.length - 1];
    const elapsed = (last.time - first.time) / 1000;
    return elapsed > 0 ? Math.max(0, (last.bytes - first.bytes) / elapsed) : 0;
  }, [status?.bytes_sent, now]);

  async function beginShare(filePath: string) {
    setError(null);
    setIsPreparing(true);
    setStatus(null);
    samples.current = [];
    try {
      const created = await createShare(filePath);
      setShare(created);
      setStatus(null);
      const freshAddresses = await getNetworkAddresses();
      setAddresses(freshAddresses);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPreparing(false);
    }
  }

  async function cancelCurrentShare() {
    await cancelShare();
    setShare(null);
    setStatus(null);
    samples.current = [];
  }

  if (!share) {
    return (
      <main className="app-shell home-shell">
        <section className="brand-block">
          <p className="brand-mark">FluxDrop</p>
          <h1>Send files privately over local Wi-Fi.</h1>
          <p>Choose a file, scan the QR code with your phone, and download directly from this PC.</p>
        </section>
        <DropZone
          active={dragActive}
          error={error}
          onActiveChange={setDragActive}
          onFilePath={beginShare}
          onError={setError}
        />
        {isPreparing ? <p className="preparing-note">Preparing local transfer link...</p> : null}
        <footer className="footer-note">
          <span>No app. No account. No cloud.</span>
          <span>Both devices must be on the same Wi-Fi network.</span>
        </footer>
      </main>
    );
  }

  return (
    <main className="app-shell send-shell">
      <header className="top-bar">
        <div>
          <p className="brand-mark compact">FluxDrop</p>
          <h1>Send this file to your phone</h1>
        </div>
      </header>
      <div className="send-grid">
        <div className="left-stack">
          <FileCard share={share} status={status} onChooseDifferent={cancelCurrentShare} />
          <StatusCard share={share} status={status} speedBytesPerSecond={speedBytesPerSecond} />
          <SecurityCard expiresAt={status?.expires_at ?? share.expires_at} onCancel={cancelCurrentShare} />
          <HelpCard addresses={addresses} status={status} />
        </div>
        <QrCard share={share} />
      </div>
    </main>
  );
}
