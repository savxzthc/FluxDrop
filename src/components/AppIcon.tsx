import type { ReactNode } from "react";

interface AppIconProps {
  name:
    | "send"
    | "receive"
    | "settings"
    | "shield"
    | "wifi"
    | "files"
    | "folder"
    | "phone"
    | "check"
    | "copy"
    | "sparkles"
    | "history"
    | "repeat"
    | "trash";
  size?: number;
}

const paths: Record<AppIconProps["name"], ReactNode> = {
  send: (
    <>
      <path d="M4 4h16v12H4z" />
      <path d="M8 20h8M12 16v4M12 12V6m0 0L9.5 8.5M12 6l2.5 2.5" />
    </>
  ),
  receive: (
    <>
      <rect x="7" y="2.5" width="10" height="19" rx="2" />
      <path d="M10 18.5h4M12 6v7m0 0l-2.5-2.5M12 13l2.5-2.5" />
    </>
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.7 1.7 0 0 0 .34 1.88l.06.06-2.83 2.83-.06-.06A1.7 1.7 0 0 0 15 19.4a1.7 1.7 0 0 0-1 .6 1.7 1.7 0 0 0-.4 1.1V21h-4v-.1A1.7 1.7 0 0 0 8.6 19.4a1.7 1.7 0 0 0-1.88.34l-.06.06-2.83-2.83.06-.06A1.7 1.7 0 0 0 4.6 15a1.7 1.7 0 0 0-.6-1 1.7 1.7 0 0 0-1.1-.4H3v-4h.1A1.7 1.7 0 0 0 4.6 8.6a1.7 1.7 0 0 0-.34-1.88l-.06-.06 2.83-2.83.06.06A1.7 1.7 0 0 0 9 4.6a1.7 1.7 0 0 0 1-.6 1.7 1.7 0 0 0 .4-1.1V3h4v.1A1.7 1.7 0 0 0 15.4 4.6a1.7 1.7 0 0 0 1.88-.34l.06-.06 2.83 2.83-.06.06A1.7 1.7 0 0 0 19.4 9c.14.36.35.7.6 1 .28.31.67.49 1.1.5h.1v4h-.1a1.7 1.7 0 0 0-1.7.5Z" />
    </>
  ),
  shield: <path d="M12 3 5 6v5c0 4.6 2.9 8.4 7 10 4.1-1.6 7-5.4 7-10V6l-7-3Zm-3 9 2 2 4-4" />,
  wifi: <path d="M5 9.5a11 11 0 0 1 14 0M8 13a6.5 6.5 0 0 1 8 0M10.5 16.5a2.5 2.5 0 0 1 3 0M12 20h.01" />,
  files: (
    <>
      <path d="M8 3h7l4 4v12a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2Z" />
      <path d="M15 3v5h4M3 8v10a2 2 0 0 0 2 2" />
    </>
  ),
  folder: <path d="M3 6.5h7l2 2h9v9.5a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V6.5Zm0 4h18" />,
  phone: (
    <>
      <rect x="7" y="2.5" width="10" height="19" rx="2" />
      <path d="M10.5 5h3M11 18.5h2" />
    </>
  ),
  check: <path d="m5 12 4 4L19 6" />,
  copy: (
    <>
      <rect x="8" y="8" width="11" height="13" rx="2" />
      <path d="M5 16H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1" />
    </>
  ),
  sparkles: <path d="m12 3 1.2 3.8L17 8l-3.8 1.2L12 13l-1.2-3.8L7 8l3.8-1.2L12 3Zm6 10 .7 2.3L21 16l-2.3.7L18 19l-.7-2.3L15 16l2.3-.7L18 13ZM5 14l.8 2.2L8 17l-2.2.8L5 20l-.8-2.2L2 17l2.2-.8L5 14Z" />,
  history: (
    <>
      <path d="M3.5 12a8.5 8.5 0 1 0 2.2-5.7L3.5 8.5" />
      <path d="M3.5 4.5v4h4M12 7.5V12l3 2" />
    </>
  ),
  repeat: <path d="M20 7h-9a6 6 0 0 0-6 6v4m0 0-3-3m3 3 3-3M14 3l3-3m0 0 3 3m-3-3v4" />,
  trash: (
    <>
      <path d="M4 7h16M9 3h6l1 4H8l1-4ZM7 7l1 14h8l1-14M10 11v6M14 11v6" />
    </>
  )
};

export function AppIcon({ name, size = 20 }: AppIconProps) {
  return (
    <svg
      className="app-icon"
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      {paths[name]}
    </svg>
  );
}
