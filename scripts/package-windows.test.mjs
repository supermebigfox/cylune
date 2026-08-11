import { spawnSync } from "node:child_process";
import { chmod, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import { expect, test } from "vitest";

async function fakeNpm(root) {
  const bin = join(root, "bin");
  const log = join(root, "npm.log");
  await mkdir(bin);
  const npm = join(bin, "npm");
  await writeFile(
    npm,
    '#!/bin/sh\nprintf "%s\\n" "$*" >> "$FAKE_NPM_LOG"\nexit "$FAKE_NPM_EXIT"\n',
  );
  await chmod(npm, 0o755);
  await writeFile(
    join(bin, "npm.cmd"),
    '@echo off\r\necho %*>>"%FAKE_NPM_LOG%"\r\nexit /b %FAKE_NPM_EXIT%\r\n',
  );
  return { bin, log };
}

function runWrapper({ bin, log, exitCode }) {
  const pathKey = Object.keys(process.env).find((name) => name.toLowerCase() === "path");
  return spawnSync(process.execPath, [join(process.cwd(), "scripts", "package-windows.mjs")], {
    cwd: process.cwd(),
    encoding: "utf8",
    env: {
      ...process.env,
      FAKE_NPM_EXIT: String(exitCode),
      FAKE_NPM_LOG: log,
      [pathKey ?? "PATH"]: `${bin}${delimiter}${process.env[pathKey ?? "PATH"] ?? ""}`,
    },
  });
}

test("Windows release stops after a failed build instead of publishing cached output", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-package-"));
  const fake = await fakeNpm(root);
  try {
    const result = runWrapper({ ...fake, exitCode: 86 });
    expect(result.status, result.stderr).toBe(86);
    expect((await readFile(fake.log, "utf8")).trim()).toBe(
      "run tauri build -- --bundles nsis",
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("Windows release performs exactly one build before the publish-only step", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-package-"));
  const fake = await fakeNpm(root);
  try {
    const result = runWrapper({ ...fake, exitCode: 0 });
    expect(result.status, result.stderr).toBe(0);
    expect((await readFile(fake.log, "utf8")).trim().split(/\r?\n/)).toEqual([
      "run tauri build -- --bundles nsis",
      "run publish:windows",
    ]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("package scripts expose a safe release and an explicit publish-only command", async () => {
  const packageJson = JSON.parse(await readFile(join(process.cwd(), "package.json"), "utf8"));
  expect(packageJson.scripts["release:windows"]).toBe("node scripts/package-windows.mjs");
  expect(packageJson.scripts["publish:windows"]).toBe("node scripts/release-windows.mjs");
});

test("release documentation never asks for a build before the build-and-publish wrapper", async () => {
  for (const path of [
    join(process.cwd(), "docs", "install-windows.md"),
    join(process.cwd(), "docs", "qa-windows-release.md"),
    join(process.cwd(), "docs", "superpowers", "plans", "2026-08-10-windows-port.md"),
  ]) {
    const document = await readFile(path, "utf8");
    expect(document).not.toMatch(
      /npm run tauri build -- --bundles nsis\s+npm run release:windows/,
    );
  }
});
