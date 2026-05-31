import { invoke } from "@tauri-apps/api/core";

export type ShareStatus =
  | { kind: "Idle" }
  | { kind: "Preparing" }
  | { kind: "Ready" }
  | { kind: "Waiting" }
  | { kind: "PhoneConnected" }
  | { kind: "AwaitingApproval" }
  | { kind: "Approved" }
  | { kind: "Denied" }
  | { kind: "Downloading" }
  | { kind: "Completed" }
  | { kind: "Expired" }
  | { kind: "Cancelled" }
  | { kind: "Error"; message: string };

export interface ShareInfo {
  id: string;
  token: string;
  file_name: string;
  file_size: number;
  file_size_human: string;
  mime_type: string;
  download_url: string;
  qr_svg: string;
  expires_at: string;
  local_ip: string;
  port: number;
  status: ShareStatus;
}

export interface ShareStatusInfo {
  file_name: string;
  file_size: number;
  file_size_human: string;
  mime_type: string;
  status: ShareStatus;
  bytes_sent: number;
  progress_percent: number;
  created_at: string;
  expires_at: string;
  download_started_at: string | null;
  download_finished_at: string | null;
  client_ip: string | null;
  local_address: string | null;
  last_request_status: string | null;
}

export interface NetworkAddress {
  interface_name: string;
  ip: string;
  preferred: boolean;
  reason: string;
}

export function createShare(filePath: string): Promise<ShareInfo> {
  return invoke("create_share", { filePath });
}

export function cancelShare(): Promise<void> {
  return invoke("cancel_share");
}

export function getShareStatus(): Promise<ShareStatusInfo | null> {
  return invoke("get_share_status");
}

export function getNetworkAddresses(): Promise<NetworkAddress[]> {
  return invoke("get_network_addresses");
}
