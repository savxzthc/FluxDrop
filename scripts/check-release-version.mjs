import { readFile } from "node:fs/promises";

const root = new URL("../", import.meta.url);
const packageJson = JSON.parse(await readFile(new URL("package.json", root), "utf8"));
const tauriConfig = JSON.parse(await readFile(new URL("src-tauri/tauri.conf.json", root), "utf8"));
const cargoToml = await readFile(new URL("src-tauri/Cargo.toml", root), "utf8");
const cargoVersion = cargoToml.match(/^version\s*=\s*"([^"]+)"/m)?.[1];

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

console.log(`FluxDrop release version ${version} is consistent.`);
