import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

function runNpm(root, args) {
  const result = spawnSync("npm", args, {
    cwd: root,
    env: process.env,
    shell: process.platform === "win32",
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const buildStatus = runNpm(root, [
    "run",
    "tauri",
    "build",
    "--",
    "--bundles",
    "nsis",
  ]);
  if (buildStatus !== 0) return buildStatus;
  return runNpm(root, ["run", "publish:windows"]);
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  process.exitCode = main();
}
