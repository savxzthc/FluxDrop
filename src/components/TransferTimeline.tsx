import type { ShareStatus } from "../lib/api";

type Direction = "send" | "receive";
type StepState = "pending" | "active" | "done" | "error";

interface TimelineStep {
  label: string;
  detail: string;
}

const SEND_STEPS: TimelineStep[] = [
  { label: "Link", detail: "QR is ready" },
  { label: "Phone", detail: "Page opened" },
  { label: "Approval", detail: "PC decision" },
  { label: "Transfer", detail: "Bytes moving" },
  { label: "Done", detail: "Link closed" }
];

const RECEIVE_STEPS: TimelineStep[] = [
  { label: "Link", detail: "Upload page ready" },
  { label: "Phone", detail: "Page opened" },
  { label: "Approval", detail: "PC decision" },
  { label: "Upload", detail: "File incoming" },
  { label: "Saved", detail: "Written to disk" }
];

export function TransferTimeline({ direction, status }: { direction: Direction; status: ShareStatus }) {
  const steps = direction === "send" ? SEND_STEPS : RECEIVE_STEPS;
  const activeIndex = timelineIndex(status.kind);
  const failed = failedStatus(status.kind);

  return (
    <ol className="transfer-timeline" aria-label={`${direction === "send" ? "Send" : "Receive"} transfer progress`}>
      {steps.map((step, index) => {
        const state = stepState(index, activeIndex, status.kind, failed);
        return (
          <li className={state} key={step.label}>
            <span>{index + 1}</span>
            <div>
              <strong>{step.label}</strong>
              <small>{step.detail}</small>
            </div>
          </li>
        );
      })}
    </ol>
  );
}

function timelineIndex(kind: ShareStatus["kind"]) {
  switch (kind) {
    case "PhoneConnected":
      return 1;
    case "AwaitingApproval":
    case "Approved":
    case "Denied":
      return 2;
    case "Downloading":
    case "Uploading":
      return 3;
    case "Completed":
      return 4;
    case "Expired":
    case "Cancelled":
    case "Error":
      return 0;
    default:
      return 0;
  }
}

function failedStatus(kind: ShareStatus["kind"]) {
  return kind === "Denied" || kind === "Expired" || kind === "Cancelled" || kind === "Error";
}

function stepState(index: number, activeIndex: number, kind: ShareStatus["kind"], failed: boolean): StepState {
  if (kind === "Completed") return "done";
  if (failed && index === activeIndex) return "error";
  if (index < activeIndex) return "done";
  if (index === activeIndex) return "active";
  return "pending";
}
