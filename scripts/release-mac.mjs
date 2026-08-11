import { spawnSync } from "node:child_process";
import { cp, copyFile, mkdir, rm } from "node:fs/promises";
import { dirname, join, posix, resolve, win32 } from "node:path";
import { fileURLToPath } from "node:url";
import { rustTargetDir } from "./rust.mjs";

export function releaseBundleRoot(options = {}) {
  const paths = options.platform === "win32" ? win32 : posix;
  return paths.join(rustTargetDir(options), "release", "bundle");
}

export async function publishMacBundles({ sourceApp, sourceDmg, releaseApp, releaseDmg }) {
  await mkdir(dirname(releaseApp), { recursive: true });
  await rm(releaseApp, { recursive: true, force: true });
  await cp(sourceApp, releaseApp, { recursive: true });
  if (sourceDmg && releaseDmg) await copyFile(sourceDmg, releaseDmg);
  await rm(sourceApp, { recursive: true, force: true });
}

async function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const target = rustTargetDir();
  const build = spawnSync("npm", ["run", "tauri", "build", "--", "--bundles", "app"], {
    cwd: root,
    env: { ...process.env, CARGO_TARGET_DIR: target },
    stdio: "inherit",
  });
  if (build.status !== 0) process.exit(build.status ?? 1);

  const bundle = releaseBundleRoot();
  const sourceApp = join(bundle, "macos", "CYLUNE.app");
  const release = join(root, "发布");
  await publishMacBundles({
    sourceApp,
    releaseApp: join(release, "CYLUNE.app"),
  });
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  await main();
}
