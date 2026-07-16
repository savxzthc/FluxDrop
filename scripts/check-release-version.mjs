import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const packageJson = JSON.parse(await readFile(new URL("package.json", root), "utf8"));
const tauriConfig = JSON.parse(await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"));
const cargoToml = await readFile(new URL("src-tauri/Cargo.toml", root), "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
const releaseWorkflow = await readFile(new URL(".github/workflows/release.yml", root), "utf8");

if (!cargoVersion) {
  throw new Error("Could not read the package version from src-tauri/Cargo.toml.");
}

const versions = {
  "package.json": packageJson.version,
  "src-tauri/tauri.conf.json": tauriConfig.version,
  "src-tauri/Cargo.toml": cargoVersion
};
const uniqueVersions = new Set(Object.values(versions));

if (uniqueVersions.size !== 1) {
  throw new Error(
    `Release versions do not match:\n${Object.entries(versions)
      .map(([file, version]) => `- ${file}: ${version}`)
      .join("\n")}`
  );
}

const version = packageJson.version;
const requestedTag = process.argv[2];
if (requestedTag && requestedTag !== `v${version}`) {
  throw new Error(`Release tag ${requestedTag} does not match application version v${version}.`);
}

if (tauriConfig.bundle?.createUpdaterArtifacts !== true) {
  throw new Error("Tauri updater artifacts must be enabled for releases.");
}
if (!tauriConfig.plugins?.updater?.endpoints?.some((endpoint) => endpoint.endsWith("/latest.json"))) {
  throw new Error("Tauri updater configuration must include a latest.json endpoint.");
}
for (const required of [
  "TAURI_SIGNING_PRIVATE_KEY",
  "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
  "TAURI_SIGNING_PUBLIC_KEY",
  "FLUXDROP_BUILD_FLAVOR: installed",
  "FLUXDROP_BUILD_FLAVOR: portable",
  "latest.json"
]) {
  if (!releaseWorkflow.includes(required)) {
    throw new Error(`Release workflow is missing updater requirement: ${required}`);
  }
}

console.log(`FluxDrop release version ${version} is consistent.`);
