import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import { AppIcon } from "./components/AppIcon";
import { ApprovalPrompt } from "./components/ApprovalPrompt";
import { DropZone } from "./components/DropZone";
import { FileCard } from "./components/FileCard";
import { HelpCard } from "./components/HelpCard";
import { QrCard } from "./components/QrCard";
import { ReceiveQrCard } from "./components/ReceiveQrCard";
import { ReceiveSetupCard } from "./components/ReceiveSetupCard";
import { ReceiveStatusCard } from "./components/ReceiveStatusCard";
import { SecurityCard } from "./components/SecurityCard";
import { SettingsCard } from "./components/SettingsCard";
import { StatusCard } from "./components/StatusCard";
import { ThemeToggle } from "./components/ThemeToggle";
import {
  approveDownload,
  approveUpload,
  AppSettings,
  cancelReceive,
  cancelShare,
  createShare,
  denyDownload,
  denyUpload,
  getNetworkAddresses,
  getReceiveStatus,
  getSettings,
  getShareStatus,
  NetworkAddress,
  ReceiveInfo,
  ReceiveStatusInfo,
  ShareInfo,
  ShareStatusInfo,
  startReceive,
  updateSettings
} from "./lib/api";

type SpeedSample = { time: number; bytes: number };
type AppView = "send" | "receive" | "settings";

const DEFAULT_SETTINGS: AppSettings = {
  expiration_minutes: 10,
  single_use: true,
  approval_required: true,
  preferred_lan_ip: null,
  max_upload_bytes: 2 * 1024 * 1024 * 1024,
  theme: "system"
};

const VIEW_COPY: Record<AppView, { eyebrow: string; title: string }> = {
  send: { eyebrow: "Send workspace", title: "Send to phone" },
  receive: { eyebrow: "Receive workspace", title: "Receive from phone" },
  settings: { eyebrow: "Preferences", title: "Settings" }
};

export default function App() {
  const storedTheme = window.localStorage.getItem("fluxdrop-theme");
  const [view, setView] = useState<AppView>("send");
  const [share, setShare] = useState<ShareInfo | null>(null);
  const [status, setStatus] = useState<ShareStatusInfo | null>(null);
  const [receive, setReceive] = useState<ReceiveInfo | null>(null);
  const [receiveStatus, setReceiveStatus] = useState<ReceiveStatusInfo | null>(null);
  const [addresses, setAddresses] = useState<NetworkAddress[]>([]);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [dragActive, setDragActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const [now, setNow] = useState(Date.now());
  const [systemDark, setSystemDark] = useState(() => window.matchMedia("(prefers-color-scheme: dark)").matches);
  const [themePreference, setThemePreference] = useState<AppSettings["theme"]>(
    storedTheme === "dark" || storedTheme === "light" || storedTheme === "system" ? storedTheme : "system"
  );
  const samples = useRef<SpeedSample[]>([]);
  const resolvedTheme =
    themePreference === "dark" || themePreference === "light"
      ? themePreference
      : systemDark
        ? "dark"
        : "light";
  const activeAddress =
    addresses.find((address) => address.ip === settings.preferred_lan_ip) ??
    addresses.find((address) => address.preferred) ??
    addresses[0];
  const currentCopy = VIEW_COPY[view];

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const updateSystemTheme = (event: MediaQueryListEvent) => setSystemDark(event.matches);
    setSystemDark(query.matches);
    query.addEventListener("change", updateSystemTheme);
    return () => query.removeEventListener("change", updateSystemTheme);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.style.colorScheme = resolvedTheme;
    window.localStorage.setItem("fluxdrop-theme", themePreference);
  }, [resolvedTheme, themePreference]);

  useEffect(() => {
    getNetworkAddresses().then(setAddresses).catch(() => setAddresses([]));
    getSettings()
      .then((loaded) => {
        setSettings(loaded);
        setThemePreference(loaded.theme);
      })
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    const timer = window.setInterval(() => {
      setNow(Date.now());
      if (share) getShareStatus().then(setStatus).catch(() => undefined);
      if (receive) getReceiveStatus().then(setReceiveStatus).catch(() => undefined);
    }, 1000);
    return () => window.clearInterval(timer);
  }, [share, receive]);

  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    const subscribe = async () => {
      for (const eventName of [
        "phone_connected",
        "approval_requested",
        "download_approved",
        "download_denied",
        "download_timed_out",
        "download_started",
        "progress_updated",
        "download_completed",
        "share_expired",
        "share_cancelled"
      ]) {
        const unlisten = await listen<ShareStatusInfo>(eventName, (event) => setStatus(event.payload));
        unlisteners.push(unlisten);
      }
      for (const eventName of [
        "upload_phone_connected",
        "upload_approval_requested",
        "upload_approved",
        "upload_denied",
        "upload_timed_out",
        "upload_started",
        "upload_progress",
        "upload_completed",
        "upload_interrupted",
        "receive_expired",
        "receive_cancelled"
      ]) {
        const unlisten = await listen<ReceiveStatusInfo>(eventName, (event) => setReceiveStatus(event.payload));
        unlisteners.push(unlisten);
      }
      const dropUnlisten = await listen<string[] | { paths?: string[] }>("tauri://drag-drop", (event) => {
        if (view !== "send" || share || receive) return;
        const payload = event.payload;
        const paths = Array.isArray(payload) ? payload : payload.paths ?? [];
        if (paths.length > 0) void beginShare(paths);
      });
      unlisteners.push(dropUnlisten);
    };
    void subscribe().catch(() => undefined);
    return () => unlisteners.forEach((unlisten) => unlisten());
  }, [view, share, receive]);

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

  async function beginShare(filePaths: string[]) {
    setError(null);
    setIsPreparing(true);
    setStatus(null);
    samples.current = [];
    try {
      const created = await createShare(filePaths);
      setReceive(null);
      setReceiveStatus(null);
      setShare(created);
      setView("send");
      setAddresses(await getNetworkAddresses());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPreparing(false);
    }
  }

  async function beginReceive(destinationFolder: string) {
    setError(null);
    setIsPreparing(true);
    setReceiveStatus(null);
    try {
      const created = await startReceive(destinationFolder);
      setShare(null);
      setStatus(null);
      setReceive(created);
      setView("receive");
      setAddresses(await getNetworkAddresses());
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

  async function cancelCurrentReceive() {
    await cancelReceive();
    setReceive(null);
    setReceiveStatus(null);
  }

  async function saveSettings(next: AppSettings) {
    const saved = await updateSettings(next);
    setSettings(saved);
    setThemePreference(saved.theme);
  }

  async function toggleTheme() {
    const previousTheme = themePreference;
    const nextTheme = resolvedTheme === "dark" ? "light" : "dark";
    setThemePreference(nextTheme);
    setError(null);
    try {
      await saveSettings({ ...settings, theme: nextTheme });
    } catch (err) {
      setThemePreference(previousTheme);
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function decideDownload(approved: boolean) {
    setApprovalBusy(true);
    setError(null);
    try {
      await (approved ? approveDownload() : denyDownload());
      setStatus(await getShareStatus());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setApprovalBusy(false);
    }
  }

  async function decideUpload(approved: boolean) {
    setApprovalBusy(true);
    setError(null);
    try {
      await (approved ? approveUpload() : denyUpload());
      setReceiveStatus(await getReceiveStatus());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setApprovalBusy(false);
    }
  }

  return (
    <div className="desktop-app">
      <aside className="app-sidebar">
        <div className="app-brand">
          <div className="app-brand-icon" aria-hidden="true">
            <span />
            <span />
          </div>
          <div>
            <strong>FluxDrop</strong>
            <small>Local transfer</small>
          </div>
        </div>

        <nav className="sidebar-nav" aria-label="Primary navigation">
          <button className={view === "send" ? "active" : ""} type="button" onClick={() => setView("send")}>
            <AppIcon name="send" />
            <span>Send</span>
            {share ? <i className="nav-activity-dot" aria-label="Active send link" /> : null}
          </button>
          <button className={view === "receive" ? "active" : ""} type="button" onClick={() => setView("receive")}>
            <AppIcon name="receive" />
            <span>Receive</span>
            {receive ? <i className="nav-activity-dot" aria-label="Active receive link" /> : null}
          </button>
          <button className={view === "settings" ? "active" : ""} type="button" onClick={() => setView("settings")}>
            <AppIcon name="settings" />
            <span>Settings</span>
          </button>
        </nav>

        <div className="sidebar-spacer" />
        <div className="sidebar-security">
          <span className="sidebar-security-icon">
            <AppIcon name="shield" size={17} />
          </span>
          <div>
            <strong>Local and encrypted</strong>
            <small>Files never touch the cloud.</small>
          </div>
        </div>
        <ThemeToggle dark={resolvedTheme === "dark"} onToggle={() => void toggleTheme()} />
      </aside>

      <section className="app-main">
        <header className="app-toolbar">
          <div>
            <span>{currentCopy.eyebrow}</span>
            <strong>{currentCopy.title}</strong>
          </div>
          <div className="network-indicator">
            <i className={activeAddress ? "online" : ""} />
            <span>{activeAddress ? activeAddress.ip : "Finding LAN"}</span>
          </div>
        </header>

        <main className="app-content">
          {error ? (
            <div className="app-alert" role="alert">
              <strong>FluxDrop needs your attention</strong>
              <span>{error}</span>
              <button type="button" onClick={() => setError(null)} aria-label="Dismiss error">
                Close
              </button>
            </div>
          ) : null}

          {view === "settings" ? (
            <SettingsWorkspace settings={settings} addresses={addresses} onSave={saveSettings} />
          ) : null}

          {view === "send" && !share ? (
            <StartWorkspace
              direction="send"
              preparing={isPreparing}
              dropActive={dragActive}
              onDropActive={setDragActive}
              onPaths={beginShare}
              onError={setError}
            />
          ) : null}

          {view === "receive" && !receive ? (
            <StartWorkspace
              direction="receive"
              preparing={isPreparing}
              onStartReceive={beginReceive}
              onError={setError}
            />
          ) : null}

          {view === "send" && share ? (
            <SendWorkspace
              share={share}
              status={status}
              addresses={addresses}
              speedBytesPerSecond={speedBytesPerSecond}
              approvalBusy={approvalBusy}
              onDecision={decideDownload}
              onCancel={cancelCurrentShare}
            />
          ) : null}

          {view === "receive" && receive ? (
            <ReceiveWorkspace
              receive={receive}
              status={receiveStatus}
              approvalBusy={approvalBusy}
              onDecision={decideUpload}
              onCancel={cancelCurrentReceive}
            />
          ) : null}
        </main>
      </section>
    </div>
  );
}

interface StartWorkspaceProps {
  direction: "send" | "receive";
  preparing: boolean;
  dropActive?: boolean;
  onDropActive?: (active: boolean) => void;
  onPaths?: (paths: string[]) => void;
  onStartReceive?: (folder: string) => void;
  onError: (message: string) => void;
}

function StartWorkspace({
  direction,
  preparing,
  dropActive = false,
  onDropActive,
  onPaths,
  onStartReceive,
  onError
}: StartWorkspaceProps) {
  const sending = direction === "send";
  return (
    <>
      <section className="workspace-heading">
        <div>
          <span className="eyebrow">{sending ? "PC to phone" : "Phone to PC"}</span>
          <h1>{sending ? "Send anything in a few seconds." : "Bring a file onto this PC."}</h1>
          <p>
            {sending
              ? "Choose files or a folder. FluxDrop creates a private link for your phone on this network."
              : "Choose where the file should land. Your phone gets a private upload page with PC approval."}
          </p>
        </div>
        <div className="workspace-trust-badge">
          <AppIcon name="shield" />
          <div>
            <strong>Protected transfer</strong>
            <span>HTTPS, expiring token, PC approval</span>
          </div>
        </div>
      </section>

      <div className="start-grid">
        {sending && onDropActive && onPaths ? (
          <DropZone
            active={dropActive}
            error={null}
            onActiveChange={onDropActive}
            onPaths={onPaths}
            onError={onError}
          />
        ) : null}
        {!sending && onStartReceive ? (
          <ReceiveSetupCard error={null} onStart={onStartReceive} onError={onError} />
        ) : null}
        <QuickGuide direction={direction} />
      </div>
      {preparing ? (
        <div className="preparing-card">
          <span className="preparing-spinner" />
          <div>
            <strong>Preparing your secure link</strong>
            <span>Inspecting files and starting the local server...</span>
          </div>
        </div>
      ) : null}
    </>
  );
}

function QuickGuide({ direction }: { direction: "send" | "receive" }) {
  const sending = direction === "send";
  const steps = sending
    ? [
        ["Choose", "Select files, a folder, or drag them into FluxDrop."],
        ["Scan", "Open the QR code with your phone camera."],
        ["Approve", "Confirm the phone on this PC and transfer."]
      ]
    : [
        ["Choose", "Pick the PC folder where uploads should arrive."],
        ["Scan", "Open the private upload page on your phone."],
        ["Approve", "Review the exact filename and size before writing."]
      ];

  return (
    <aside className="quick-guide">
      <div className="quick-guide-title">
        <span className="feature-icon">
          <AppIcon name="sparkles" />
        </span>
        <div>
          <span className="eyebrow">Simple by design</span>
          <h2>How it works</h2>
        </div>
      </div>
      <ol className="step-list">
        {steps.map(([title, copy], index) => (
          <li key={title}>
            <span>{index + 1}</span>
            <div>
              <strong>{title}</strong>
              <p>{copy}</p>
            </div>
          </li>
        ))}
      </ol>
      <div className="privacy-strip">
        <AppIcon name="wifi" size={18} />
        <span>Direct over your local network. No account or cloud upload.</span>
      </div>
    </aside>
  );
}

interface SendWorkspaceProps {
  share: ShareInfo;
  status: ShareStatusInfo | null;
  addresses: NetworkAddress[];
  speedBytesPerSecond: number;
  approvalBusy: boolean;
  onDecision: (approved: boolean) => Promise<void>;
  onCancel: () => Promise<void>;
}

function SendWorkspace({
  share,
  status,
  addresses,
  speedBytesPerSecond,
  approvalBusy,
  onDecision,
  onCancel
}: SendWorkspaceProps) {
  return (
    <>
      {status?.status.kind === "AwaitingApproval" ? (
        <ApprovalPrompt
          direction="download"
          clientIp={status.client_ip}
          fileName={status.file_name || share.file_name}
          fileSizeHuman={status.file_size_human || share.file_size_human}
          busy={approvalBusy}
          onApprove={() => void onDecision(true)}
          onDeny={() => void onDecision(false)}
        />
      ) : null}
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Share is live</span>
          <h1>Ready for your phone.</h1>
          <p>Scan the code, approve the request here, and FluxDrop handles the rest.</p>
        </div>
        <span className="live-badge">
          <i />
          Secure link active
        </span>
      </section>
      <div className="transfer-layout">
        <div className="transfer-stack">
          <FileCard share={share} status={status} onChooseDifferent={() => void onCancel()} />
          <StatusCard share={share} status={status} speedBytesPerSecond={speedBytesPerSecond} />
          <div className="transfer-secondary-grid">
            <SecurityCard direction="send" expiresAt={status?.expires_at ?? share.expires_at} onCancel={() => void onCancel()} />
            <HelpCard addresses={addresses} status={status} />
          </div>
        </div>
        <QrCard share={share} />
      </div>
    </>
  );
}

interface ReceiveWorkspaceProps {
  receive: ReceiveInfo;
  status: ReceiveStatusInfo | null;
  approvalBusy: boolean;
  onDecision: (approved: boolean) => Promise<void>;
  onCancel: () => Promise<void>;
}

function ReceiveWorkspace({ receive, status, approvalBusy, onDecision, onCancel }: ReceiveWorkspaceProps) {
  return (
    <>
      {status?.status.kind === "AwaitingApproval" ? (
        <ApprovalPrompt
          direction="upload"
          clientIp={status.client_ip}
          fileName={status.file_name ?? "Unknown file"}
          fileSizeHuman={status.file_size_human ?? "Unknown size"}
          busy={approvalBusy}
          onApprove={() => void onDecision(true)}
          onDeny={() => void onDecision(false)}
        />
      ) : null}
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Receive link is live</span>
          <h1>Waiting for your phone.</h1>
          <p>The file is written only after you approve its exact name and size.</p>
        </div>
        <span className="live-badge">
          <i />
          Secure receive active
        </span>
      </section>
      <div className="transfer-layout">
        <div className="transfer-stack">
          <ReceiveStatusCard receive={receive} status={status} />
          <SecurityCard
            direction="receive"
            expiresAt={status?.expires_at ?? receive.expires_at}
            onCancel={() => void onCancel()}
          />
        </div>
        <ReceiveQrCard receive={receive} />
      </div>
    </>
  );
}

interface SettingsWorkspaceProps {
  settings: AppSettings;
  addresses: NetworkAddress[];
  onSave: (settings: AppSettings) => Promise<void>;
}

function SettingsWorkspace({ settings, addresses, onSave }: SettingsWorkspaceProps) {
  return (
    <>
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Personalize FluxDrop</span>
          <h1>App settings.</h1>
          <p>Control security defaults, appearance, link behavior, and which LAN adapter FluxDrop uses.</p>
        </div>
      </section>
      <SettingsCard settings={settings} addresses={addresses} onSave={onSave} />
    </>
  );
}
