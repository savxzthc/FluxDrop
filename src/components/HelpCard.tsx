import { useState } from "react";
import type { FirewallDiagnostic, NetworkAddress } from "../lib/api";
import { copyTextToClipboard } from "../lib/clipboard";

interface HelpStatus {
  client_ip: string | null;
  last_request_status: string | null;
  local_address: string | null;
}

interface HelpCardProps {
  addresses: NetworkAddress[];
  serverAddress: string;
  status: HelpStatus | null;
  firewall: FirewallDiagnostic | null;
  firewallRepairing: boolean;
  onRepairFirewall: () => Promise<void>;
}

export function HelpCard({ addresses, serverAddress, status, firewall, firewallRepairing, onRepairFirewall }: HelpCardProps) {
  const [copyState, setCopyState] = useState<"idle" | "copied" | "failed">("idle");

  async function copyNetworkDetails() {
    const copied = await copyTextToClipboard(buildNetworkDetails(addresses, serverAddress, status));
    setCopyState(copied ? "copied" : "failed");
    window.setTimeout(() => setCopyState("idle"), 1800);
  }

  return (
    <section className="panel help-panel">
      <details>
        <summary>LAN troubleshooting</summary>
        <div className="help-content">
          <p>Both devices must be on the same Wi-Fi network. Guest networks, VPNs, and client isolation can block phones from reaching this PC.</p>
          <p>If the phone cannot open the QR link, allow FluxDrop through Windows Defender Firewall for private networks and temporarily disconnect VPN software.</p>
          <p><strong>Firewall:</strong> {firewall?.message ?? "Checking Windows Firewall policy."}</p>
          {firewall?.repair_available && firewall.state !== "public_network" ? (
            <button className="subtle-button compact-button" type="button" onClick={() => void onRepairFirewall()} disabled={firewallRepairing}>
              {firewallRepairing ? "Waiting for UAC..." : "Repair private firewall rule"}
            </button>
          ) : null}
          <dl className="status-grid">
            <div>
              <dt>Detected server</dt>
              <dd>{status?.local_address ?? serverAddress}</dd>
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
          <div className="help-actions">
            <button className="subtle-button compact-button" type="button" onClick={() => void copyNetworkDetails()}>
              Copy network details
            </button>
            <span className={copyState === "failed" ? "copy-status-error" : ""} aria-live="polite">
              {copyState === "copied" ? "Copied." : copyState === "failed" ? "Copy failed." : ""}
            </span>
          </div>
          <ol className="help-checklist">
            <li>Confirm both devices are on the same private Wi-Fi, not a guest network.</li>
            <li>Allow FluxDrop through Windows Defender Firewall for private networks.</li>
            <li>Disable VPN or client isolation if the phone can scan but cannot open the page.</li>
          </ol>
          <ul className="address-list">
            {addresses.length > 0 ? (
              addresses.map((address) => (
                <li key={`${address.interface_name}-${address.ip}`}>
                  <strong>{address.ip}</strong> <span>{address.interface_name}</span>
                  <em>{address.reason}</em>
                </li>
              ))
            ) : (
              <li>
                <strong>No private LAN address detected</strong>
                <span>Connect to Wi-Fi or choose another adapter in Settings.</span>
              </li>
            )}
          </ul>
        </div>
      </details>
    </section>
  );
}

function buildNetworkDetails(
  addresses: NetworkAddress[],
  serverAddress: string,
  status: HelpStatus | null
) {
  const lines = [
    "FluxDrop LAN troubleshooting",
    `Generated: ${new Date().toISOString()}`,
    "",
    "Live transfer",
    `- Server: ${status?.local_address ?? serverAddress}`,
    `- Phone IP: ${status?.client_ip ?? "none"}`,
    `- Last request: ${status?.last_request_status ?? "no requests yet"}`,
    "",
    "Detected LAN addresses",
    ...(addresses.length > 0
      ? addresses.map((address) => `- ${address.ip} | ${address.interface_name} | ${address.reason}`)
      : ["- none"]),
    "",
    "Checks",
    "- Same private Wi-Fi, not guest Wi-Fi",
    "- Windows Defender Firewall allows FluxDrop on private networks",
    "- VPN and client isolation are disabled while testing"
  ];
  return lines.join("\n");
}
