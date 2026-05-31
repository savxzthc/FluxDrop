import type { NetworkAddress, ShareStatusInfo } from "../lib/api";

interface HelpCardProps {
  addresses: NetworkAddress[];
  status: ShareStatusInfo | null;
}

export function HelpCard({ addresses, status }: HelpCardProps) {
  return (
    <section className="panel help-panel">
      <details>
        <summary>LAN troubleshooting</summary>
        <div className="help-content">
          <p>Both devices must be on the same Wi-Fi network. Guest networks, VPNs, and client isolation can block phones from reaching this PC.</p>
          <p>If the phone cannot open the QR link, allow FluxDrop through Windows Defender Firewall for private networks and temporarily disconnect VPN software.</p>
          <dl className="status-grid">
            <div>
              <dt>Detected server</dt>
              <dd>{status?.local_address ?? "Not running"}</dd>
            </div>
            <div>
              <dt>Last phone IP</dt>
              <dd>{status?.client_ip ?? "None"}</dd>
            </div>
            <div>
              <dt>Last request</dt>
              <dd>{status?.last_request_status ?? "No requests yet"}</dd>
            </div>
          </dl>
          <ul className="address-list">
            {addresses.map((address) => (
              <li key={`${address.interface_name}-${address.ip}`}>
                <strong>{address.ip}</strong> <span>{address.interface_name}</span>
                <em>{address.reason}</em>
              </li>
            ))}
          </ul>
        </div>
      </details>
    </section>
  );
}
