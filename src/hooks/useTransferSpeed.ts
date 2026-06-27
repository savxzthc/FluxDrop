import { useCallback, useEffect, useMemo, useRef } from "react";

type SpeedSample = { time: number; bytes: number };

export function useTransferSpeed(bytesSent: number | undefined, tick: number) {
  const samples = useRef<SpeedSample[]>([]);

  useEffect(() => {
    if (bytesSent === undefined) return;
    const now = Date.now();
    samples.current.push({ time: now, bytes: bytesSent });
    samples.current = samples.current.filter((sample) => now - sample.time <= 3500);
  }, [bytesSent]);

  const speedBytesPerSecond = useMemo(() => {
    if (samples.current.length < 2) return 0;
    const first = samples.current[0];
    const last = samples.current[samples.current.length - 1];
    const elapsed = (last.time - first.time) / 1000;
    return elapsed > 0 ? Math.max(0, (last.bytes - first.bytes) / elapsed) : 0;
  }, [bytesSent, tick]);

  const resetSpeedSamples = useCallback(() => {
    samples.current = [];
  }, []);

  return { resetSpeedSamples, speedBytesPerSecond };
}
