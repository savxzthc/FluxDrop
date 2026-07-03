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
  | { kind: "Uploading" }
  | { kind: "Completed" }
  | { kind: "Expired" }
  | { kind: "Cancelled" }
  | { kind: "Error"; message: string };

export interface ShareInfo {
  id: string;
  file_name: string;
  file_size: number;
  file_size_human: string;
  mime_type: string;
  file_count: number;
  is_archive: boolean;
  download_url: string;
  qr_svg: string;
  created_at: string;
  expires_at: string;
  local_ip: string;
  port: number;
  status: ShareStatus;
  approval_required: boolean;
}

export interface ShareStatusInfo {
  file_name: string;
  file_size: number;
  file_size_human: string;
  mime_type: string;
  file_count: number;
  is_archive: boolean;
  status: ShareStatus;
  bytes_sent: number;
  progress_percent: number;
  created_at: string;
  expires_at: string;
  download_started_at: string | null;
  download_finished_at: string | null;
  client_ip: string | null;
  approval_deadline: string | null;
  approval_timed_out: boolean;
  local_address: string | null;
  last_request_status: string | null;
}

export interface NetworkAddress {
  interface_name: string;
  ip: string;
  preferred: boolean;
  reason: string;
}

export interface ReceiveInfo {
  id: string;
  upload_url: string;
  qr_svg: string;
  destination_folder_name: string;
  created_at: string;
  expires_at: string;
  local_ip: string;
  port: number;
  max_upload_bytes: number;
  max_upload_size_human: string;
  status: ShareStatus;
  approval_required: boolean;
}

export interface ReceiveStatusInfo {
  file_name: string | null;
  file_size: number | null;
  file_size_human: string | null;
  mime_type: string | null;
  status: ShareStatus;
  bytes_received: number;
  progress_percent: number;
  created_at: string;
  expires_at: string;
  approval_deadline: string | null;
  approval_timed_out: boolean;
  client_ip: string | null;
  local_address: string | null;
  destination_folder_name: string;
  last_request_status: string | null;
}

export interface AppSettings {
  expiration_minutes: number;
  single_use: boolean;
  approval_required: boolean;
  preferred_lan_ip: string | null;
  max_upload_bytes: number;
  theme: "system" | "light" | "dark";
  shell_integration: boolean;
  global_hotkey: boolean;
  remember_transfer_locations: boolean;
}

export interface CreateShareOptions {
  expiration_minutes?: number;
  single_use?: boolean;
  approval_required?: boolean;
}

export interface StartReceiveOptions {
  expiration_minutes?: number;
  approval_required?: boolean;
  max_upload_bytes?: number;
}

export type TransferDirection = "send" | "receive";
export type TransferOutcome = "completed" | "denied" | "timed_out" | "cancelled" | "expired" | "failed";

export interface HistoryEntry {
  id: string;
  direction: TransferDirection;
  file_name: string;
  file_size: number | null;
  file_size_human: string | null;
  file_count: number;
  is_archive: boolean;
  mime_type: string | null;
  created_at: string;
  finished_at: string;
  client_ip: string | null;
  outcome: TransferOutcome;
  can_repeat: boolean;
  repeat_unavailable_reason: string | null;
}

export interface ForgetHistoryLocationsResult {
  changed_count: number;
  entries: HistoryEntry[];
}

export type RepeatedTransfer =
  | { direction: "send"; transfer: ShareInfo }
  | { direction: "receive"; transfer: ReceiveInfo };

export function createShare(filePaths: string[], options?: CreateShareOptions): Promise<ShareInfo> {
  return invoke("create_share", { filePaths, options: options ?? null });
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

export function approveDownload(): Promise<void> {
  return invoke("approve_download");
}

export function denyDownload(): Promise<void> {
  return invoke("deny_download");
}

export function getSettings(): Promise<AppSettings> {
  return invoke("get_settings");
}

export function updateSettings(newSettings: AppSettings): Promise<AppSettings> {
  return invoke("update_settings", { newSettings });
}

export function startReceive(destinationFolder: string, options?: StartReceiveOptions): Promise<ReceiveInfo> {
  return invoke("start_receive", { destinationFolder, options: options ?? null });
}

export function getReceiveStatus(): Promise<ReceiveStatusInfo | null> {
  return invoke("get_receive_status");
}

export function cancelReceive(): Promise<void> {
  return invoke("cancel_receive");
}

export function approveUpload(): Promise<void> {
  return invoke("approve_upload");
}

export function denyUpload(): Promise<void> {
  return invoke("deny_upload");
}

export function getTransferHistory(): Promise<HistoryEntry[]> {
  return invoke("get_transfer_history");
}

export function clearTransferHistory(): Promise<void> {
  return invoke("clear_transfer_history");
}

export function forgetHistoryLocations(): Promise<ForgetHistoryLocationsResult> {
  return invoke("forget_history_locations");
}

export function repeatTransfer(historyId: string): Promise<RepeatedTransfer> {
  return invoke("repeat_transfer", { historyId });
}

export function takePendingShellShare(): Promise<string[] | null> {
  return invoke("take_pending_shell_share");
}
