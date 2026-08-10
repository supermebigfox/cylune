import { spawnSync } from "node:child_process";
import { copyFile, mkdir, readdir, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { rustTargetDir } from "./rust.mjs";

export async function releaseWindowsBundle({ bundleRoot, releaseRoot }) {
  const nsisRoot = join(bundleRoot, "nsis");
  const setupFiles = (await readdir(nsisRoot)).filter((name) => name.endsWith("-setup.exe"));
  if (setupFiles.length !== 1) {
    throw new Error(`Expected exactly one NSIS setup in ${nsisRoot}, found ${setupFiles.length}`);
  }

  const sourceSetup = join(nsisRoot, setupFiles[0]);
  const publishedSetup = join(releaseRoot, "CYLUNE-Setup.exe");
  await mkdir(dirname(publishedSetup), { recursive: true });
  await copyFile(sourceSetup, publishedSetup);
  await rm(sourceSetup);
  return publishedSetup;
}

async function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const target = rustTargetDir();
  const build = spawnSync("npm", ["run", "tauri", "build", "--", "--bundles", "nsis"], {
    cwd: root,
    env: { ...process.env, CARGO_TARGET_DIR: target },
    shell: process.platform === "win32",
    stdio: "inherit",
  });
  if (build.error) throw build.error;
  if (build.status !== 0) process.exit(build.status ?? 1);

  await releaseWindowsBundle({
    bundleRoot: join(target, "release", "bundle"),
    releaseRoot: join(root, "发布-Windows"),
  });
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  await main();
}
