// Publishes a Quackblox version that the installed apps update to by themselves (src/update.ts),
// adapted from Quackify's release.mjs:
//
//   npm run release -- 0.2.0 "What changed, one line or more"
//
// From a clean checkout up to date with master: puts the version in package.json, Cargo.toml
// and tauri.conf.json, builds the installer signed with the key in ~/.tauri/quackblox.key
// (TAURI_SIGNING_PRIVATE_KEY overrides it), packs Quackblox-Setup.exe, commits and pushes the
// version, and creates the GitHub release vX with Quackblox-Setup.exe (what people download),
// the NSIS installer and its signature (what the update window downloads) and latest.json, the
// file the apps read at releases/latest/download/latest.json.
// Without that key no update can be signed: the apps out there only trust its public half.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { bundle, packSetup } from "./build-setup.mjs";

const REPO = "EnderexDev/Quackblox";
const BRANCH = "master";
const root = join(import.meta.dirname, "..");
const [version, ...noteWords] = process.argv.slice(2);
const notes = noteWords.join(" ").trim();

const fail = (why) => {
  console.error(`release: ${why}`);
  process.exit(1);
};
// npm and npx are .cmd on Windows and need the shell; git and gh do not, and the notes keep their spaces.
const run = (cmd, args, opts = {}) => execFileSync(cmd, args, { cwd: root, stdio: "inherit", shell: cmd === "npm" || cmd === "npx", ...opts });
const read = (cmd, args) => execFileSync(cmd, args, { cwd: root, encoding: "utf8" }).trim();

if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) fail('usage: npm run release -- 0.2.0 "notes"');
// The public master has to be the code the installer is built from: the push at the end only
// goes through as a fast-forward, and checking it here saves a build that could not be published.
run("git", ["fetch", "origin", BRANCH]);
try {
  read("git", ["merge-base", "--is-ancestor", `origin/${BRANCH}`, "HEAD"]);
} catch {
  fail(`${BRANCH} on GitHub has commits this checkout lacks: merge them first`);
}
if (read("git", ["status", "--porcelain", "--untracked-files=no"])) fail("the tree has changes: commit or drop them first");
const keyPath = join(homedir(), ".tauri", "quackblox.key");
const key = process.env.TAURI_SIGNING_PRIVATE_KEY ?? (existsSync(keyPath) ? readFileSync(keyPath, "utf8").trim() : null);
if (!key) fail(`no signing key at ${keyPath}`);
const tag = `v${version}`;
if (read("git", ["tag", "--list", tag])) fail(`${tag} already exists`);

// The version, in the three files that carry it.
const edit = (file, change) => {
  const path = join(root, file);
  writeFileSync(path, change(readFileSync(path, "utf8")));
};
edit("package.json", (s) => s.replace(/"version": "[^"]+"/, `"version": "${version}"`));
edit("src-tauri/tauri.conf.json", (s) => s.replace(/"version": "[^"]+"/, `"version": "${version}"`));
edit("src-tauri/Cargo.toml", (s) => s.replace(/^version = "[^"]+"/m, `version = "${version}"`));

const VERSION_FILES = ["package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-tauri/Cargo.lock"];
/** Puts the old version back, so the next try does not stop at "the tree has changes". */
const undo = (why) => {
  run("git", ["checkout", "--", ...VERSION_FILES]);
  fail(`${why}; the version files are back as they were`);
};

try {
  run("npx", ["tauri", "build", "--bundles", "nsis"], {
    env: { ...process.env, TAURI_SIGNING_PRIVATE_KEY: key, TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "" },
  });
} catch {
  undo("the build failed");
}

let packed;
try {
  packed = packSetup(version);
} catch (e) {
  undo(e.message);
}
const exe = packed.nsis;
const setupName = `Quackblox_${version}_x64-setup.exe`;
if (!existsSync(`${exe}.sig`)) undo(`the build left no ${setupName}.sig in ${bundle}`);
const latest = join(bundle, "latest.json");
writeFileSync(
  latest,
  JSON.stringify(
    {
      version,
      notes,
      pub_date: new Date().toISOString(),
      platforms: {
        "windows-x86_64": {
          signature: readFileSync(`${exe}.sig`, "utf8").trim(),
          url: `https://github.com/${REPO}/releases/download/${tag}/${setupName}`,
        },
      },
    },
    null,
    2,
  ) + "\n",
);

// Cargo.lock carries the version too: the build has just written it.
run("git", ["add", ...VERSION_FILES]);
// A second try after a failed push or release finds the version already committed.
try {
  read("git", ["diff", "--cached", "--quiet"]);
} catch {
  try {
    run("git", ["commit", "-m", `Quackblox ${version}\n\nCo-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`]);
  } catch {
    run("git", ["reset", "-q", "--", ...VERSION_FILES]);
    undo("the commit failed");
  }
}
run("git", ["push", "origin", `HEAD:${BRANCH}`]);
// Neither draft nor prerelease: releases/latest skips both, and the apps would not see it.
run("gh", ["release", "create", tag, packed.setup, exe, `${exe}.sig`, latest, "-R", REPO, "--target", read("git", ["rev-parse", "HEAD"]), "--title", `Quackblox ${version}`, "--notes", notes || `Quackblox ${version}`]);
console.log(`release: ${tag} is out, the installed apps get it within 6 hours or from "Procurar atualização"`);
