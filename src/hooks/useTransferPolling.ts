import { useEffect } from "react";
import {
  getReceiveStatus,
  getShareStatus,
  ReceiveInfo,
  ReceiveStatusInfo,
  ShareInfo,
  ShareStatusInfo
} from "../lib/api";

interface UseTransferPollingOptions {
  onReceiveStatus: (status: ReceiveStatusInfo | null) => void;
  onShareStatus: (status: ShareStatusInfo | null) => void;
  onTick: (now: number) => void;
  receive: ReceiveInfo | null;
  share: ShareInfo | null;
}

export function useTransferPolling({
  onReceiveStatus,
  onShareStatus,
  onTick,
  receive,
  share
}: UseTransferPollingOptions) {
  useEffect(() => {
    if (!share && !receive) return;

    const poll = () => {
      onTick(Date.now());
      if (share) getShareStatus().then(onShareStatus).catch(() => undefined);
      if (receive) getReceiveStatus().then(onReceiveStatus).catch(() => undefined);
    };

    poll();
    const timer = window.setInterval(poll, 1000);
    return () => window.clearInterval(timer);
  }, [onReceiveStatus, onShareStatus, onTick, receive, share]);
}
