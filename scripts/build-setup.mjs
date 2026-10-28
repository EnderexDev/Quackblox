// Builds Quackblox-Setup.exe: the release quackblox.exe with the NSIS installer Tauri builds
// appended and a trailer (the installer's length, then "QBSETUP1") that setup.rs looks for at
// the end of its own file. Adapted from Quackify's pack-setup.mjs.
//
//   node scripts/build-setup.mjs            build, then pack (a local try: unsigned)
//   node scripts/build-setup.mjs --pack     pack the last build only
//
// release.mjs imports packSetup to build the signed one it publishes.
import { execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
export const release = join(root, "src-tauri", "target", "release");
export const bundle = join(release, "bundle", "nsis");
const MAGIC = Buffer.from("QBSETUP1", "ascii");

/** Packs the build of `version`; returns the NSIS installer's path and Quackblox-Setup.exe's. */
export function packSetup(version) {
  const nsisName = existsSync(bundle) && readdirSync(bundle).find((f) => f === `Quackblox_${version}_x64-setup.exe`);
  const exe = join(release, "quackblox.exe");
  if (!nsisName || !existsSync(exe)) throw new Error(`no quackblox.exe or NSIS installer for ${version} in ${release}`);
  const nsisPath = join(bundle, nsisName);
  const app = readFileSync(exe);
  const nsis = readFileSync(nsisPath);
  const length = Buffer.alloc(8);
  length.writeBigUInt64LE(BigInt(nsis.length));
  const out = join(bundle, "Quackblox-Setup.exe");
  writeFileSync(out, Buffer.concat([app, nsis, length, MAGIC]));
  console.log(`build-setup: ${out} (${((app.length + nsis.length) / 1048576).toFixed(1)} MB)`);
  return { nsis: nsisPath, setup: out };
}

// Run as a script, not imported by release.mjs.
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  // Without the signing key the updater artifacts cannot be signed: a local build skips them.
  if (!process.argv.includes("--pack"))
    execSync(`npx tauri build --bundles nsis --config "{\\"bundle\\":{\\"createUpdaterArtifacts\\":false}}"`, { cwd: root, stdio: "inherit" });
  const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
  try {
    packSetup(version);
  } catch (e) {
    console.error(`build-setup: ${e.message}`);
    process.exit(1);
  }
}
