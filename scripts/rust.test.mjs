import { readFileSync } from "node:fs";
import { resolve, win32 } from "node:path";
import { expect, test } from "vitest";
import { rustTargetDir } from "./rust.mjs";

test("keeps Rust build artifacts in each operating system cache directory", () => {
  expect(rustTargetDir({
    platform: "darwin",
    home: "/Users/robin",
    env: {},
  })).toBe("/Users/robin/Library/Caches/CYLUNE/rust");

  expect(rustTargetDir({
    platform: "win32",
    home: "C:\\Users\\Robin",
    env: { LOCALAPPDATA: "C:\\Users\\Robin\\AppData\\Local" },
  })).toBe(win32.resolve("C:\\Users\\Robin\\AppData\\Local", "CYLUNE", "Cache", "rust"));

  expect(rustTargetDir({
    platform: "linux",
    home: "/home/robin",
    env: { XDG_CACHE_HOME: "/var/cache/robin" },
  })).toBe("/var/cache/robin/cylune/rust");
});

test("honors an explicit Cargo target directory", () => {
  expect(rustTargetDir({
    platform: "darwin",
    home: "/Users/robin",
    env: { CARGO_TARGET_DIR: "/Volumes/Build/CYLUNE" },
  })).toBe("/Volumes/Build/CYLUNE");
});

test("routes normal Tauri and Rust test commands through the cache launcher", () => {
  const packageJson = JSON.parse(readFileSync(resolve("package.json"), "utf8"));

  expect(packageJson.scripts.tauri).toBe("node scripts/rust.mjs tauri");
  expect(packageJson.scripts["test:rust"]).toBe("node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml");
});
