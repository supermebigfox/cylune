import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { homedir } from "node:os";
import { posix, win32 } from "node:path";
import { fileURLToPath } from "node:url";

export function rustTargetDir({
  platform = process.platform,
  home = homedir(),
  env = process.env,
} = {}) {
  const paths = platform === "win32" ? win32 : posix;
  if (env.CARGO_TARGET_DIR) return paths.resolve(env.CARGO_TARGET_DIR);
  if (platform === "darwin") {
    return paths.join(home, "Library", "Caches", "CYLUNE", "rust");
  }
  if (platform === "win32") {
    const local = env.LOCALAPPDATA || paths.join(home, "AppData", "Local");
    return paths.join(local, "CYLUNE", "Cache", "rust");
  }
  const cache = env.XDG_CACHE_HOME || paths.join(home, ".cache");
  return paths.join(cache, "cylune", "rust");
}

export function rustEnvironment(options = {}) {
  const env = options.env ?? process.env;
  const target = rustTargetDir({ ...options, env });
  mkdirSync(target, { recursive: true });
  return { ...env, CARGO_TARGET_DIR: target };
}

function main() {
  const [command, ...args] = process.argv.slice(2);
  if (!command) {
    console.error("Usage: node scripts/rust.mjs <command> [...args]");
    process.exit(2);
  }
  const result = spawnSync(command, args, {
    env: rustEnvironment(),
    shell: process.platform === "win32",
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  process.exit(result.status ?? 1);
}

const invokedPath = process.argv[1] ? fileURLToPath(import.meta.url) === process.argv[1] : false;
if (invokedPath) main();
