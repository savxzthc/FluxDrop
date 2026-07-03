import { useEffect, useRef, useState } from "react";
import { formatDuration } from "../lib/format";

interface ExpiryMeterProps {
  createdAt?: string;
  expiresAt: string;
  label: string;
}

export function ExpiryMeter({ createdAt, expiresAt, label }: ExpiryMeterProps) {
  const [now, setNow] = useState(() => Date.now());
  const createdMs = createdAt ? Date.parse(createdAt) : Number.NaN;
  const expiresMs = Date.parse(expiresAt);
  const initialSeconds = useRef(1);
  const valid = Number.isFinite(expiresMs);
  const hasCreatedAt = Number.isFinite(createdMs) && createdMs < expiresMs;
  const secondsRemaining = valid ? Math.max(0, Math.floor((expiresMs - now) / 1000)) : 0;
  const totalSeconds =
    valid && hasCreatedAt ? Math.max(1, Math.floor((expiresMs - createdMs) / 1000)) : initialSeconds.current;
  const progress = valid ? Math.max(0, Math.min(100, (secondsRemaining / totalSeconds) * 100)) : 0;
  const tone = !valid || secondsRemaining === 0 ? "expired" : secondsRemaining < 60 ? "soon" : "active";
  const value = valid
    ? secondsRemaining > 0
      ? `Expires in ${formatDuration(secondsRemaining)}`
      : "Expired"
    : "Expiration unavailable";

  useEffect(() => {
    initialSeconds.current = valid && !hasCreatedAt ? Math.max(1, Math.floor((expiresMs - Date.now()) / 1000)) : 1;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [createdAt, expiresAt, expiresMs, hasCreatedAt, valid]);

  return (
    <div className={`expiry-meter ${tone}`} aria-label={`${label}: ${value}`}>
      <div>
        <span>{label}</span>
        <strong>{value}</strong>
      </div>
      <i aria-hidden="true">
        <b style={{ width: `${progress}%` }} />
      </i>
    </div>
  );
}
