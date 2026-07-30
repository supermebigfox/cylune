import { spawnSync } from "node:child_process";
import { cp, copyFile, mkdir, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

export async function publishMacBundles({ sourceApp, sourceDmg, releaseApp, releaseDmg }) {
  await mkdir(dirname(releaseApp), { recursive: true });
  await rm(releaseApp, { recursive: true, force: true });
  await cp(sourceApp, releaseApp, { recursive: true });
  if (sourceDmg && releaseDmg) await copyFile(sourceDmg, releaseDmg);
  await rm(sourceApp, { recursive: true, force: true });
}

async function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const build = spawnSync("npm", ["run", "tauri", "build", "--", "--bundles", "app"], {
    cwd: root,
    stdio: "inherit",
  });
  if (build.status !== 0) process.exit(build.status ?? 1);

  const bundle = join(root, "src-tauri", "target", "release", "bundle");
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
