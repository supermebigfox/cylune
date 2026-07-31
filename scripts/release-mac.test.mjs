import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test } from "vitest";
import { publishMacBundles, releaseBundleRoot } from "./release-mac.mjs";

test("reads release bundles from the shared Rust cache directory", () => {
  expect(releaseBundleRoot({
    platform: "darwin",
    home: "/Users/robin",
    env: {},
  })).toBe("/Users/robin/Library/Caches/CYLUNE/rust/release/bundle");

  expect(releaseBundleRoot({
    platform: "darwin",
    home: "/Users/robin",
    env: { CARGO_TARGET_DIR: "/Volumes/Build/CYLUNE" },
  })).toBe("/Volumes/Build/CYLUNE/release/bundle");
});

test("publishes one formal app and removes the temporary Spotlight app", async () => {
  const workspace = mkdtempSync(join(tmpdir(), "cylune-release-"));
  const sourceApp = join(workspace, "target", "macos", "CYLUNE.app");
  const sourceDmg = join(workspace, "target", "dmg", "CYLUNE.dmg");
  const releaseApp = join(workspace, "发布", "CYLUNE.app");
  const releaseDmg = join(workspace, "发布", "CYLUNE.dmg");
  mkdirSync(join(sourceApp, "Contents"), { recursive: true });
  mkdirSync(join(workspace, "target", "dmg"), { recursive: true });
  writeFileSync(join(sourceApp, "Contents", "marker"), "signed-app");
  writeFileSync(sourceDmg, "signed-dmg");

  try {
    await publishMacBundles({ sourceApp, sourceDmg, releaseApp, releaseDmg });

    expect(readFileSync(join(releaseApp, "Contents", "marker"), "utf8")).toBe("signed-app");
    expect(readFileSync(releaseDmg, "utf8")).toBe("signed-dmg");
    expect(existsSync(sourceApp)).toBe(false);
    expect(existsSync(sourceDmg)).toBe(true);
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
