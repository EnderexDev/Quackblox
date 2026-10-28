// Builds Quackblox-Setup.exe: the release quackblox.exe with the NSIS installer Tauri builds
// appended and a trailer (the installer's length, then "QBSETUP1") that setup.rs looks for at
// the end of its own file. Adapted from Quackify's pack-setup.mjs.
//
//   node scripts/build-setup.mjs            build, then pack
//   node scripts/build-setup.mjs --pack     pack the last build only
import { execSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const release = join(root, "src-tauri", "target", "release");
const bundle = join(release, "bundle", "nsis");
const MAGIC = Buffer.from("QBSETUP1", "ascii");

if (!process.argv.includes("--pack")) execSync("npx tauri build --bundles nsis", { cwd: root, stdio: "inherit" });

const { version } = JSON.parse(readFileSync(join(root, "src-tauri", "tauri.conf.json"), "utf8"));
const nsisName = readdirSync(bundle).find((f) => f.startsWith(`Quackblox_${version}_`) && f.endsWith("-setup.exe"));
const exe = join(release, "quackblox.exe");
if (!nsisName || !existsSync(exe)) {
  console.error(`build-setup: no quackblox.exe or NSIS installer for ${version} in ${release}`);
  process.exit(1);
}

const app = readFileSync(exe);
const nsis = readFileSync(join(bundle, nsisName));
const length = Buffer.alloc(8);
length.writeBigUInt64LE(BigInt(nsis.length));
const out = join(bundle, "Quackblox-Setup.exe");
writeFileSync(out, Buffer.concat([app, nsis, length, MAGIC]));
console.log(`build-setup: ${out} (${((app.length + nsis.length) / 1048576).toFixed(1)} MB)`);
