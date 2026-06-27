import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef } from "react";
import { ReceiveStatusInfo, ShareStatusInfo } from "../lib/api";

type AppView = "send" | "receive" | "history" | "settings";

const SHARE_EVENTS = [
  "phone_connected",
  "approval_requested",
  "download_approved",
  "download_denied",
  "download_timed_out",
  "download_started",
  "progress_updated",
  "download_completed",
  "download_interrupted",
  "share_expired",
  "share_cancelled"
];

const TERMINAL_SHARE_EVENTS = new Set([
  "download_denied",
  "download_timed_out",
  "download_completed",
  "share_expired",
  "share_cancelled",
  "download_interrupted"
]);

const RECEIVE_EVENTS = [
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
];

const TERMINAL_RECEIVE_EVENTS = new Set([
  "upload_denied",
  "upload_timed_out",
  "upload_completed",
  "upload_interrupted",
  "receive_expired",
  "receive_cancelled"
]);

interface TransferEventCallbacks {
  onBeginShare: (paths: string[]) => void;
  onFocusSend: () => void;
  onReceiveStatus: (status: ReceiveStatusInfo) => void;
  onRefreshHistory: () => void;
  onShareStatus: (status: ShareStatusInfo) => void;
  onShellPaths: (paths: string[]) => void;
  receiveActive: boolean;
  shareActive: boolean;
  view: AppView;
}

export function useTauriTransferEvents(callbacks: TransferEventCallbacks) {
  const callbacksRef = useRef(callbacks);
  callbacksRef.current = callbacks;

  useEffect(() => {
    let cancelled = false;
    const unlisteners: Array<() => void> = [];

    const track = (unlisten: () => void) => {
      if (cancelled) {
        unlisten();
        return;
      }
      unlisteners.push(unlisten);
    };

    const subscribe = async () => {
      for (const eventName of SHARE_EVENTS) {
        track(
          await listen<ShareStatusInfo>(eventName, (event) => {
            callbacksRef.current.onShareStatus(event.payload);
            if (TERMINAL_SHARE_EVENTS.has(eventName)) callbacksRef.current.onRefreshHistory();
          })
        );
      }

      for (const eventName of RECEIVE_EVENTS) {
        track(
          await listen<ReceiveStatusInfo>(eventName, (event) => {
            callbacksRef.current.onReceiveStatus(event.payload);
            if (TERMINAL_RECEIVE_EVENTS.has(eventName)) callbacksRef.current.onRefreshHistory();
          })
        );
      }

      track(
        await listen<string[] | { paths?: string[] }>("tauri://drag-drop", (event) => {
          const { receiveActive, shareActive, view } = callbacksRef.current;
          if (view !== "send" || shareActive || receiveActive) return;
          const payload = event.payload;
          const paths = Array.isArray(payload) ? payload : payload.paths ?? [];
          if (paths.length > 0) callbacksRef.current.onBeginShare(paths);
        })
      );

      track(
        await listen<string[]>("shell_share", (event) => {
          const paths = Array.isArray(event.payload) ? event.payload : [];
          if (paths.length > 0) callbacksRef.current.onShellPaths(paths);
        })
      );

      track(await listen("shell_focus", () => callbacksRef.current.onFocusSend()));
    };

    void subscribe().catch(() => undefined);
    return () => {
      cancelled = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);
}
