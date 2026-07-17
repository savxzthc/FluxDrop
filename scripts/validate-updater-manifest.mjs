import { readFile } from "node:fs/promises";

const path = process.argv[2];
if (!path) throw new Error("Usage: npm run updater:validate -- <latest.json|--self-test>");

const manifest = path === "--self-test"
  ? {
      version: "0.4.0",
      pub_date: "2026-07-16T00:00:00Z",
      platforms: {
        "windows-x86_64": {
          signature: "trusted-comment:test-signature-placeholder",
          url: "https://github.com/savxzthc/FluxDrop/releases/download/v0.4.0/FluxDrop_0.4.0_x64-setup.exe"
        }
      }
    }
  : JSON.parse(await readFile(path, "utf8"));
if (typeof manifest.version !== "string" || !/^\d+\.\d+\.\d+(?:[-+].+)?$/.test(manifest.version)) {
  throw new Error("Updater manifest has an invalid version.");
}
if (typeof manifest.pub_date !== "string" || Number.isNaN(Date.parse(manifest.pub_date))) {
  throw new Error("Updater manifest has an invalid pub_date.");
}
const windows = manifest.platforms?.["windows-x86_64"];
if (!windows || typeof windows.signature !== "string" || windows.signature.trim().length < 20) {
  throw new Error("Updater manifest is missing the Windows updater signature.");
}
if (typeof windows.url !== "string" || !windows.url.startsWith("https://github.com/savxzthc/FluxDrop/releases/download/")) {
  throw new Error("Updater manifest has an invalid Windows download URL.");
}
console.log(`Validated signed updater metadata for FluxDrop ${manifest.version}.`);
