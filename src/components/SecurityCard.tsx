interface SecurityCardProps {
  direction?: "send" | "receive";
  expiresAt: string;
  onCancel: () => void;
}

export function SecurityCard({ direction = "send", expiresAt, onCancel }: SecurityCardProps) {
  const secondsRemaining = Math.max(0, Math.floor((Date.parse(expiresAt) - Date.now()) / 1000));
  const minutes = Math.floor(secondsRemaining / 60);
  const seconds = secondsRemaining % 60;

  return (
    <section className={`panel expiry-panel ${secondsRemaining < 60 ? "expiry-warning" : ""}`}>
      <div className="panel-heading">
        <span className="eyebrow">Link expiration</span>
        <button className="danger-button" type="button" onClick={onCancel}>
          {direction === "receive" ? "Stop receive mode" : "Cancel link"}
        </button>
      </div>
      <p className="countdown">
        {minutes}:{seconds.toString().padStart(2, "0")}
      </p>
      <p>
        This link works only on your local network and expires automatically.{" "}
        {direction === "receive"
          ? "Incoming files require the configured approval decision and size limit."
          : "Anyone on the same network who obtains the link must still pass the configured approval step."}
      </p>
    </section>
  );
}
