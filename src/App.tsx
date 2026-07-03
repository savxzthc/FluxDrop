import { useEffect, useRef, useState } from "react";
import { AppIcon } from "./components/AppIcon";
import { ApprovalPrompt } from "./components/ApprovalPrompt";
import { DropZone } from "./components/DropZone";
import { FileCard } from "./components/FileCard";
import { HelpCard } from "./components/HelpCard";
import { HistoryCard } from "./components/HistoryCard";
import { QrCard } from "./components/QrCard";
import { ReceiveQrCard } from "./components/ReceiveQrCard";
import { ReceiveSetupCard } from "./components/ReceiveSetupCard";
import { ReceiveStatusCard } from "./components/ReceiveStatusCard";
import { SecurityCard } from "./components/SecurityCard";
import { SettingsCard } from "./components/SettingsCard";
import { StatusCard } from "./components/StatusCard";
import { ThemeToggle } from "./components/ThemeToggle";
import {
  receiveOptionsFromDraft,
  shareOptionsFromDraft,
  TransferOptionsCard,
  transferOptionsFromSettings,
  TransferOptionsDraft
} from "./components/TransferOptionsCard";
import { useTauriTransferEvents } from "./hooks/useTauriTransferEvents";
import { useThemePreference } from "./hooks/useThemePreference";
import { useTransferPolling } from "./hooks/useTransferPolling";
import { useTransferSpeed } from "./hooks/useTransferSpeed";
import {
  approveDownload,
  approveUpload,
  AppSettings,
  cancelReceive,
  cancelShare,
  clearTransferHistory,
  createShare,
  CreateShareOptions,
  denyDownload,
  denyUpload,
  forgetHistoryLocations,
  getNetworkAddresses,
  getReceiveStatus,
  getSettings,
  getShareStatus,
  getTransferHistory,
  HistoryEntry,
  NetworkAddress,
  ReceiveInfo,
  ReceiveStatusInfo,
  repeatTransfer,
  ShareInfo,
  ShareStatusInfo,
  StartReceiveOptions,
  startReceive,
  takePendingShellShare,
  updateSettings
} from "./lib/api";
import { formatBytes } from "./lib/format";

type AppView = "send" | "receive" | "history" | "settings";

const DEFAULT_SETTINGS: AppSettings = {
  expiration_minutes: 10,
  single_use: true,
  approval_required: true,
  preferred_lan_ip: null,
  max_upload_bytes: 2 * 1024 * 1024 * 1024,
  theme: "system",
  shell_integration: false,
  global_hotkey: false,
  remember_transfer_locations: true
};

const VIEW_COPY: Record<AppView, { eyebrow: string; title: string }> = {
  send: { eyebrow: "Send workspace", title: "Send to phone" },
  receive: { eyebrow: "Receive workspace", title: "Receive from phone" },
  history: { eyebrow: "Activity", title: "Transfer history" },
  settings: { eyebrow: "Preferences", title: "Settings" }
};

export default function App() {
  const [view, setView] = useState<AppView>("send");
  const [share, setShare] = useState<ShareInfo | null>(null);
  const [status, setStatus] = useState<ShareStatusInfo | null>(null);
  const [receive, setReceive] = useState<ReceiveInfo | null>(null);
  const [receiveStatus, setReceiveStatus] = useState<ReceiveStatusInfo | null>(null);
  const [addresses, setAddresses] = useState<NetworkAddress[]>([]);
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [historyBusyId, setHistoryBusyId] = useState<string | null>(null);
  const [historyClearing, setHistoryClearing] = useState(false);
  const [dragActive, setDragActive] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [isPreparing, setIsPreparing] = useState(false);
  const [addressesLoaded, setAddressesLoaded] = useState(false);
  const [addressesRefreshing, setAddressesRefreshing] = useState(false);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const [now, setNow] = useState(Date.now());
  const lastShellShare = useRef<{ key: string; at: number } | null>(null);
  const { resolvedTheme, setThemePreference, themePreference } = useThemePreference();
  const { resetSpeedSamples: resetSendSpeedSamples, speedBytesPerSecond: sendSpeedBytesPerSecond } =
    useTransferSpeed(status?.bytes_sent, now);
  const { resetSpeedSamples: resetReceiveSpeedSamples, speedBytesPerSecond: receiveSpeedBytesPerSecond } =
    useTransferSpeed(receiveStatus?.bytes_received, now);
  const activeAddress =
    addresses.find((address) => address.ip === settings.preferred_lan_ip) ??
    addresses.find((address) => address.preferred) ??
    addresses[0];
  const currentCopy = VIEW_COPY[view];

  useEffect(() => {
    getNetworkAddresses()
      .then(setAddresses)
      .catch(() => setAddresses([]))
      .finally(() => setAddressesLoaded(true));
    getSettings()
      .then((loaded) => {
        setSettings(loaded);
        setThemePreference(loaded.theme);
      })
      .catch(() => undefined);
    getTransferHistory().then(setHistory).catch(() => setHistory([]));
    takePendingShellShare()
      .then((paths) => {
        if (paths && paths.length > 0) handleShellPaths(paths);
      })
      .catch(() => undefined);
  }, []);

  useTransferPolling({
    onReceiveStatus: setReceiveStatus,
    onShareStatus: setStatus,
    onTick: setNow,
    receive,
    share
  });

  useTauriTransferEvents({
    onBeginShare: (paths) => void beginShare(paths),
    onFocusSend: () => setView("send"),
    onReceiveStatus: setReceiveStatus,
    onRefreshHistory: () => void refreshHistory(),
    onShareStatus: setStatus,
    onShellPaths: handleShellPaths,
    receiveActive: Boolean(receive),
    shareActive: Boolean(share),
    view
  });

  function handleShellPaths(paths: string[]) {
    if (!paths || paths.length === 0) return;
    const key = paths.join("|");
    const at = Date.now();
    if (lastShellShare.current && lastShellShare.current.key === key && at - lastShellShare.current.at < 4000) {
      return;
    }
    lastShellShare.current = { key, at };
    setView("send");
    void beginShare(paths);
  }

  async function beginShare(filePaths: string[], options?: CreateShareOptions) {
    setError(null);
    setIsPreparing(true);
    setStatus(null);
    resetSendSpeedSamples();
    try {
      const created = await createShare(filePaths, options);
      setReceive(null);
      setReceiveStatus(null);
      resetReceiveSpeedSamples();
      setShare(created);
      setView("send");
      setAddresses(await getNetworkAddresses());
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsPreparing(false);
    }
  }

  async function refreshHistory() {
    setHistory(await getTransferHistory());
  }

  async function beginReceive(destinationFolder: string, options?: StartReceiveOptions) {
    setError(null);
    setIsPreparing(true);
    setReceiveStatus(null);
    resetReceiveSpeedSamples();
    try {
      const created = await startReceive(destinationFolder, options);
      setShare(null);
      setStatus(null);
      resetSendSpeedSamples();
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
    resetSendSpeedSamples();
    await refreshHistory();
  }

  async function cancelCurrentReceive() {
    await cancelReceive();
    setReceive(null);
    setReceiveStatus(null);
    resetReceiveSpeedSamples();
    await refreshHistory();
  }

  async function repeatHistoryEntry(entry: HistoryEntry) {
    setHistoryBusyId(entry.id);
    setError(null);
    try {
      const repeated = await repeatTransfer(entry.id);
      if (repeated.direction === "send") {
        setReceive(null);
        setReceiveStatus(null);
        setStatus(null);
        resetReceiveSpeedSamples();
        resetSendSpeedSamples();
        setShare(repeated.transfer);
        setView("send");
      } else {
        setShare(null);
        setStatus(null);
        setReceiveStatus(null);
        resetSendSpeedSamples();
        resetReceiveSpeedSamples();
        setReceive(repeated.transfer);
        setView("receive");
      }
      setAddresses(await getNetworkAddresses());
      await refreshHistory();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setHistoryBusyId(null);
    }
  }

  async function clearHistory() {
    setHistoryClearing(true);
    setError(null);
    try {
      await clearTransferHistory();
      setHistory([]);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setHistoryClearing(false);
    }
  }

  async function forgetSavedHistoryLocations() {
    const result = await forgetHistoryLocations();
    setHistory(result.entries);
    return result.changed_count;
  }

  async function saveSettings(next: AppSettings) {
    const saved = await updateSettings(next);
    setSettings(saved);
    setThemePreference(saved.theme);
  }

  async function refreshAddresses() {
    setAddressesRefreshing(true);
    setError(null);
    try {
      setAddresses(await getNetworkAddresses());
      setAddressesLoaded(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setAddressesRefreshing(false);
    }
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
          <button
            className={view === "send" ? "active" : ""}
            type="button"
            aria-label="Send"
            onClick={() => setView("send")}
          >
            <AppIcon name="send" />
            <span>Send</span>
            {share ? <i className="nav-activity-dot" aria-label="Active send link" /> : null}
          </button>
          <button
            className={view === "receive" ? "active" : ""}
            type="button"
            aria-label="Receive"
            onClick={() => setView("receive")}
          >
            <AppIcon name="receive" />
            <span>Receive</span>
            {receive ? <i className="nav-activity-dot" aria-label="Active receive link" /> : null}
          </button>
          <button
            className={view === "history" ? "active" : ""}
            type="button"
            aria-label="History"
            onClick={() => setView("history")}
          >
            <AppIcon name="history" />
            <span>History</span>
          </button>
          <button
            className={view === "settings" ? "active" : ""}
            type="button"
            aria-label="Settings"
            onClick={() => setView("settings")}
          >
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
            <SettingsWorkspace
              settings={settings}
              addresses={addresses}
              onForgetHistoryLocations={forgetSavedHistoryLocations}
              onSave={saveSettings}
            />
          ) : null}

          {view === "history" ? (
            <HistoryWorkspace
              entries={history}
              busyId={historyBusyId}
              clearing={historyClearing}
              onRepeat={repeatHistoryEntry}
              onClear={clearHistory}
            />
          ) : null}

          {view === "send" && !share ? (
            <StartWorkspace
              direction="send"
              preparing={isPreparing}
              dropActive={dragActive}
              activeAddress={activeAddress}
              addresses={addresses}
              addressesLoaded={addressesLoaded}
              addressesRefreshing={addressesRefreshing}
              history={history}
              historyBusyId={historyBusyId}
              settings={settings}
              onDropActive={setDragActive}
              onPaths={beginShare}
              onError={setError}
              onRefreshAddresses={refreshAddresses}
              onRepeat={repeatHistoryEntry}
            />
          ) : null}

          {view === "receive" && !receive ? (
            <StartWorkspace
              direction="receive"
              preparing={isPreparing}
              activeAddress={activeAddress}
              addresses={addresses}
              addressesLoaded={addressesLoaded}
              addressesRefreshing={addressesRefreshing}
              history={history}
              historyBusyId={historyBusyId}
              settings={settings}
              onStartReceive={beginReceive}
              onError={setError}
              onRefreshAddresses={refreshAddresses}
              onRepeat={repeatHistoryEntry}
            />
          ) : null}

          {view === "send" && share ? (
            <SendWorkspace
              share={share}
              status={status}
              addresses={addresses}
              speedBytesPerSecond={sendSpeedBytesPerSecond}
              approvalBusy={approvalBusy}
              onDecision={decideDownload}
              onCancel={cancelCurrentShare}
            />
          ) : null}

          {view === "receive" && receive ? (
            <ReceiveWorkspace
              receive={receive}
              status={receiveStatus}
              addresses={addresses}
              speedBytesPerSecond={receiveSpeedBytesPerSecond}
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
  activeAddress?: NetworkAddress;
  addresses: NetworkAddress[];
  addressesLoaded: boolean;
  addressesRefreshing: boolean;
  dropActive?: boolean;
  history: HistoryEntry[];
  historyBusyId: string | null;
  settings: AppSettings;
  onDropActive?: (active: boolean) => void;
  onPaths?: (paths: string[], options?: CreateShareOptions) => void;
  onStartReceive?: (folder: string, options?: StartReceiveOptions) => void;
  onError: (message: string) => void;
  onRefreshAddresses: () => Promise<void>;
  onRepeat: (entry: HistoryEntry) => Promise<void>;
}

function StartWorkspace({
  direction,
  preparing,
  activeAddress,
  addresses,
  addressesLoaded,
  addressesRefreshing,
  dropActive = false,
  history,
  historyBusyId,
  settings,
  onDropActive,
  onPaths,
  onStartReceive,
  onError,
  onRefreshAddresses,
  onRepeat
}: StartWorkspaceProps) {
  const sending = direction === "send";
  const checkingNetwork = !addressesLoaded || addressesRefreshing;
  const startDisabled = preparing || checkingNetwork || !activeAddress;
  const disabledTitle = checkingNetwork ? "Checking LAN" : !activeAddress ? "No LAN adapter" : undefined;
  const disabledCopy = checkingNetwork
    ? "FluxDrop is looking for a private network address."
    : !activeAddress
      ? "Connect to Wi-Fi or Ethernet, then refresh addresses."
      : undefined;
  const [transferOptions, setTransferOptions] = useState<TransferOptionsDraft>(() =>
    transferOptionsFromSettings(settings)
  );

  useEffect(() => {
    setTransferOptions(transferOptionsFromSettings(settings));
  }, [settings]);

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
            <span>
              HTTPS, expiring token, {transferOptions.approval_required ? "PC approval" : "approval off"}
            </span>
          </div>
        </div>
      </section>

      <div className="start-grid">
        {sending && onDropActive && onPaths ? (
          <DropZone
            active={dropActive}
            disabled={startDisabled}
            disabledCopy={disabledCopy}
            disabledTitle={disabledTitle}
            error={null}
            onActiveChange={onDropActive}
            onPaths={(paths) => onPaths(paths, shareOptionsFromDraft(transferOptions))}
            onError={onError}
          />
        ) : null}
        {!sending && onStartReceive ? (
          <ReceiveSetupCard
            disabled={startDisabled}
            disabledCopy={disabledCopy}
            disabledTitle={disabledTitle}
            error={null}
            onStart={(folder) => onStartReceive(folder, receiveOptionsFromDraft(transferOptions))}
            onError={onError}
          />
        ) : null}
        <div className="start-side-stack">
          <TransferOptionsCard
            direction={direction}
            settings={settings}
            value={transferOptions}
            onChange={setTransferOptions}
          />
          <ReadinessPanel
            activeAddress={activeAddress}
            addresses={addresses}
            addressesLoaded={addressesLoaded}
            direction={direction}
            refreshing={addressesRefreshing}
            settings={settings}
            transferOptions={transferOptions}
            onRefresh={() => void onRefreshAddresses()}
          />
          <QuickGuide direction={direction} approvalRequired={transferOptions.approval_required} />
          <RecentTransfersPanel
            direction={direction}
            entries={history}
            busyId={historyBusyId}
            onRepeat={(entry) => void onRepeat(entry)}
          />
        </div>
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

interface ReadinessPanelProps {
  activeAddress?: NetworkAddress;
  addresses: NetworkAddress[];
  addressesLoaded: boolean;
  direction: "send" | "receive";
  refreshing: boolean;
  settings: AppSettings;
  transferOptions: TransferOptionsDraft;
  onRefresh: () => void;
}

function ReadinessPanel({
  activeAddress,
  addresses,
  addressesLoaded,
  direction,
  refreshing,
  settings,
  transferOptions,
  onRefresh
}: ReadinessPanelProps) {
  return (
    <aside className="panel readiness-panel">
      <div className="readiness-header">
        <div>
          <span className="eyebrow">Ready check</span>
          <h2>Transfer conditions</h2>
        </div>
        <button className="subtle-button compact-button" type="button" onClick={onRefresh} disabled={refreshing}>
          {refreshing ? "Checking..." : "Refresh"}
        </button>
      </div>
      <div className="readiness-list">
        <ReadinessRow
          ready={addressesLoaded && Boolean(activeAddress)}
          label="LAN adapter"
          value={
            !addressesLoaded
              ? "Checking network adapters"
              : activeAddress
                ? `${activeAddress.ip} on ${activeAddress.interface_name}`
                : "No private adapter found"
          }
        />
        <ReadinessRow
          ready={transferOptions.approval_required}
          label="PC approval"
          value={transferOptions.approval_required ? "Required for this link" : "Disabled for this link"}
          warning={!transferOptions.approval_required}
        />
        <ReadinessRow
          ready
          label="Link lifetime"
          value={`${transferOptions.expiration_minutes} minute${transferOptions.expiration_minutes === 1 ? "" : "s"}`}
        />
        {direction === "receive" ? (
          <ReadinessRow
            ready={transferOptions.max_upload_bytes <= 2 * 1024 * 1024 * 1024}
            label="Upload limit"
            value={formatBytes(transferOptions.max_upload_bytes)}
            warning={transferOptions.max_upload_bytes > 2 * 1024 * 1024 * 1024}
          />
        ) : null}
        <ReadinessRow
          ready={addressesLoaded && addresses.length > 0}
          label="Detected addresses"
          value={
            !addressesLoaded
              ? "Checking"
              : addresses.length > 0
                ? `${addresses.length} candidate${addresses.length === 1 ? "" : "s"}`
                : "None"
          }
        />
        <ReadinessRow
          ready={settings.shell_integration || settings.global_hotkey}
          label="Fast access"
          value={
            settings.shell_integration && settings.global_hotkey
              ? "Explorer and app focus enabled"
              : settings.shell_integration
                ? "Explorer send enabled"
                : settings.global_hotkey
                  ? "App focus enabled"
                  : "Optional shortcuts off"
          }
          warning={!settings.shell_integration && !settings.global_hotkey}
        />
        <ReadinessRow
          ready={settings.remember_transfer_locations}
          label="Repeat history"
          value={settings.remember_transfer_locations ? "Local paths remembered" : "Private metadata only"}
          warning={!settings.remember_transfer_locations}
        />
      </div>
    </aside>
  );
}

interface ReadinessRowProps {
  ready: boolean;
  label: string;
  value: string;
  warning?: boolean;
}

function ReadinessRow({ ready, label, value, warning = false }: ReadinessRowProps) {
  return (
    <div className="readiness-row">
      <span className={`readiness-dot ${ready ? "ready" : warning ? "warning" : ""}`} />
      <div>
        <strong>{label}</strong>
        <span>{value}</span>
      </div>
    </div>
  );
}

interface RecentTransfersPanelProps {
  direction: "send" | "receive";
  entries: HistoryEntry[];
  busyId: string | null;
  onRepeat: (entry: HistoryEntry) => void;
}

function RecentTransfersPanel({ direction, entries, busyId, onRepeat }: RecentTransfersPanelProps) {
  const recent = entries.filter((entry) => entry.direction === direction && entry.can_repeat).slice(0, 3);
  return (
    <aside className="panel recent-panel">
      <div className="panel-title-with-icon">
        <span className="feature-icon compact">
          <AppIcon name="history" size={18} />
        </span>
        <div>
          <span className="eyebrow">Repeat fast</span>
          <h2>Recent {direction === "send" ? "sends" : "receives"}</h2>
        </div>
      </div>
      {recent.length === 0 ? (
        <p className="recent-empty">
          {direction === "send"
            ? "Repeatable sent files appear here after your first transfer."
            : "Repeatable receive destinations appear here after your first upload."}
        </p>
      ) : (
        <div className="recent-list">
          {recent.map((entry) => (
            <button
              className="recent-transfer"
              key={entry.id}
              type="button"
              disabled={busyId !== null}
              onClick={() => onRepeat(entry)}
            >
              <span>
                <strong>{entry.file_name}</strong>
                <small>
                  {formatCompactHistoryDate(entry.finished_at)}
                  {entry.file_size_human ? ` | ${entry.file_size_human}` : ""}
                </small>
              </span>
              <AppIcon name="repeat" size={16} />
            </button>
          ))}
        </div>
      )}
    </aside>
  );
}

function formatCompactHistoryDate(value: string) {
  return new Intl.DateTimeFormat(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit"
  }).format(new Date(value));
}

function QuickGuide({
  approvalRequired,
  direction
}: {
  approvalRequired: boolean;
  direction: "send" | "receive";
}) {
  const sending = direction === "send";
  const steps = sending
    ? [
        ["Choose", "Select files, a folder, or drag them into FluxDrop."],
        ["Scan", "Open the QR code with your phone camera."],
        approvalRequired
          ? ["Approve", "Confirm the phone on this PC and transfer."]
          : ["Transfer", "The phone can download while the link is active."]
      ]
    : [
        ["Choose", "Pick the PC folder where uploads should arrive."],
        ["Scan", "Open the private upload page on your phone."],
        approvalRequired
          ? ["Approve", "Review the exact filename and size before writing."]
          : ["Upload", "The phone can upload after choosing a file within the size limit."]
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
          approvalDeadline={status.approval_deadline}
          busy={approvalBusy}
          onApprove={() => void onDecision(true)}
          onDeny={() => void onDecision(false)}
        />
      ) : null}
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Share is live</span>
          <h1>Ready for your phone.</h1>
          <p>
            {share.approval_required
              ? "Scan the code, approve the request here, and FluxDrop handles the rest."
              : "Scan the code and the phone can download while this link is active."}
          </p>
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
            <HelpCard addresses={addresses} serverAddress={`${share.local_ip}:${share.port}`} status={status} />
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
  addresses: NetworkAddress[];
  speedBytesPerSecond: number;
  approvalBusy: boolean;
  onDecision: (approved: boolean) => Promise<void>;
  onCancel: () => Promise<void>;
}

function ReceiveWorkspace({
  receive,
  status,
  addresses,
  speedBytesPerSecond,
  approvalBusy,
  onDecision,
  onCancel
}: ReceiveWorkspaceProps) {
  return (
    <>
      {status?.status.kind === "AwaitingApproval" ? (
        <ApprovalPrompt
          direction="upload"
          clientIp={status.client_ip}
          fileName={status.file_name ?? "Unknown file"}
          fileSizeHuman={status.file_size_human ?? "Unknown size"}
          approvalDeadline={status.approval_deadline}
          busy={approvalBusy}
          onApprove={() => void onDecision(true)}
          onDeny={() => void onDecision(false)}
        />
      ) : null}
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Receive link is live</span>
          <h1>Waiting for your phone.</h1>
          <p>
            {receive.approval_required
              ? "The file is written only after you approve its exact name and size."
              : "The phone can upload one file within the configured size limit."}
          </p>
        </div>
        <span className="live-badge">
          <i />
          Secure receive active
        </span>
      </section>
      <div className="transfer-layout">
        <div className="transfer-stack">
          <ReceiveStatusCard receive={receive} status={status} speedBytesPerSecond={speedBytesPerSecond} />
          <div className="transfer-secondary-grid">
            <SecurityCard
              direction="receive"
              expiresAt={status?.expires_at ?? receive.expires_at}
              onCancel={() => void onCancel()}
            />
            <HelpCard addresses={addresses} serverAddress={`${receive.local_ip}:${receive.port}`} status={status} />
          </div>
        </div>
        <ReceiveQrCard receive={receive} />
      </div>
    </>
  );
}

interface SettingsWorkspaceProps {
  settings: AppSettings;
  addresses: NetworkAddress[];
  onForgetHistoryLocations: () => Promise<number>;
  onSave: (settings: AppSettings) => Promise<void>;
}

function SettingsWorkspace({ settings, addresses, onForgetHistoryLocations, onSave }: SettingsWorkspaceProps) {
  return (
    <>
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Personalize FluxDrop</span>
          <h1>App settings.</h1>
          <p>Control security defaults, appearance, link behavior, and which LAN adapter FluxDrop uses.</p>
        </div>
      </section>
      <SettingsCard
        settings={settings}
        addresses={addresses}
        onForgetHistoryLocations={onForgetHistoryLocations}
        onSave={onSave}
      />
    </>
  );
}

interface HistoryWorkspaceProps {
  entries: HistoryEntry[];
  busyId: string | null;
  clearing: boolean;
  onRepeat: (entry: HistoryEntry) => Promise<void>;
  onClear: () => Promise<void>;
}

function HistoryWorkspace({ entries, busyId, clearing, onRepeat, onClear }: HistoryWorkspaceProps) {
  return (
    <>
      <section className="workspace-heading compact-heading">
        <div>
          <span className="eyebrow">Local activity</span>
          <h1>Your recent transfers.</h1>
          <p>Review outcomes or restart a transfer without rebuilding its setup from scratch.</p>
        </div>
      </section>
      <HistoryCard
        entries={entries}
        busyId={busyId}
        clearing={clearing}
        onRepeat={(entry) => void onRepeat(entry)}
        onClear={() => void onClear()}
      />
    </>
  );
}
