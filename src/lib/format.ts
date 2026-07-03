export function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = bytes;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  return index === 0 ? `${bytes} B` : `${size.toFixed(1)} ${units[index]}`;
}

export function formatDuration(totalSeconds: number): string {
  const seconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(seconds / 60);
  const remainder = seconds % 60;
  return `${minutes}:${remainder.toString().padStart(2, "0")}`;
}

type TransferCopyDirection = "send" | "receive";

export function statusCopy(
  kind: string,
  message?: string,
  direction: TransferCopyDirection = "send"
): { label: string; detail: string; tone: string } {
  const receiving = direction === "receive";
  switch (kind) {
    case "Idle":
      return {
        label: "Ready to choose",
        detail: receiving
          ? "Choose a destination folder to create a local receive link."
          : "Choose files or a folder to create a local transfer link.",
        tone: "neutral"
      };
    case "Preparing":
      return { label: "Preparing", detail: "FluxDrop is validating the setup and starting the local server.", tone: "working" };
    case "Ready":
      return { label: "QR ready", detail: "Scan the QR code with your phone on the same Wi-Fi network.", tone: "ready" };
    case "Waiting":
      return { label: "Waiting for phone", detail: "The link is active. No phone has connected yet.", tone: "ready" };
    case "PhoneConnected":
      return {
        label: "Phone connected",
        detail: receiving ? "The phone opened the upload page and can choose a file." : "The phone opened the download page and is ready to download.",
        tone: "connected"
      };
    case "AwaitingApproval":
      return { label: "Awaiting approval", detail: "The phone is waiting for an explicit decision on this PC.", tone: "working" };
    case "Approved":
      return { label: "Approved", detail: receiving ? "The PC approved the phone upload." : "The PC approved the phone download.", tone: "connected" };
    case "Denied":
      return {
        label: "Denied",
        detail: receiving ? "The upload request was denied or timed out before approval." : "The download request was denied or timed out before approval.",
        tone: "error"
      };
    case "Downloading":
      return { label: "Downloading", detail: "The file is streaming directly from this PC to the phone.", tone: "working" };
    case "Uploading":
      return { label: "Uploading", detail: "The phone is streaming the approved file into a temporary file on this PC.", tone: "working" };
    case "Completed":
      return {
        label: "Complete",
        detail: receiving ? "The approved phone upload was saved on this PC." : "The file finished downloading and the one-time link is no longer valid.",
        tone: "done"
      };
    case "Expired":
      return { label: "Expired", detail: "The configured link window has ended.", tone: "error" };
    case "Cancelled":
      return {
        label: "Cancelled",
        detail: receiving ? "Receive mode was cancelled. The phone page will show a cancelled upload." : "The sender cancelled this link. The phone page will show a cancelled transfer.",
        tone: "error"
      };
    case "Error":
      return { label: "Needs attention", detail: message ?? "Something went wrong during this transfer.", tone: "error" };
    default:
      return { label: "Unknown state", detail: "FluxDrop received a status it does not recognize.", tone: "error" };
  }
}
