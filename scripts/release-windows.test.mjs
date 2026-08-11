import {
  access,
  mkdtemp,
  mkdir,
  readFile,
  readdir,
  rename,
  rm,
  symlink,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { expect, test } from "vitest";
import { releaseWindowsBundle } from "./release-windows.mjs";

test("publishes the one NSIS setup without leaving target artifacts", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const setup = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  const nearbyArtifact = join(bundleRoot, "nsis", "installer.nsi");
  const msiArtifact = join(bundleRoot, "msi", "CYLUNE_0.1.0_x64.msi");
  const bundleManifest = join(bundleRoot, "manifest.json");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await mkdir(join(bundleRoot, "msi"), { recursive: true });
  await writeFile(setup, "fixture");
  await writeFile(nearbyArtifact, "preserve");
  await writeFile(msiArtifact, "preserve-msi");
  await writeFile(bundleManifest, "preserve-manifest");

  try {
    const published = await releaseWindowsBundle({ bundleRoot, releaseRoot });

    expect(published).toBe(join(releaseRoot, "CYLUNE-Setup.exe"));
    expect(await readFile(published, "utf8")).toBe("fixture");
    await expect(access(setup)).rejects.toThrow();
    expect(await readFile(nearbyArtifact, "utf8")).toBe("preserve");
    expect(await readFile(msiArtifact, "utf8")).toBe("preserve-msi");
    expect(await readFile(bundleManifest, "utf8")).toBe("preserve-manifest");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a missing NSIS setup", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });

  try {
    await expect(
      releaseWindowsBundle({ bundleRoot, releaseRoot: join(root, "发布-Windows") }),
    ).rejects.toThrow(/Expected exactly one NSIS setup.*found 0/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects multiple NSIS setups without moving either one", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const first = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  const second = join(bundleRoot, "nsis", "CYLUNE_0.1.1_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await writeFile(first, "first");
  await writeFile(second, "second");

  try {
    await expect(
      releaseWindowsBundle({ bundleRoot, releaseRoot: join(root, "发布-Windows") }),
    ).rejects.toThrow(/Expected exactly one NSIS setup.*found 2/);
    expect(await readFile(first, "utf8")).toBe("first");
    expect(await readFile(second, "utf8")).toBe("second");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a symlinked NSIS setup without reading or deleting its target", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const target = join(root, "outside-setup.exe");
  const setup = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await writeFile(target, "outside");
  await symlink(target, setup, "file");

  try {
    await expect(
      releaseWindowsBundle({ bundleRoot, releaseRoot: join(root, "发布-Windows") }),
    ).rejects.toThrow(/symbolic link/);
    expect(await readFile(target, "utf8")).toBe("outside");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("rejects a symlinked NSIS directory without reading or deleting its target", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const externalNsisRoot = join(root, "external-nsis");
  const setup = join(externalNsisRoot, "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(bundleRoot, { recursive: true });
  await mkdir(externalNsisRoot, { recursive: true });
  await writeFile(setup, "outside");
  await symlink(externalNsisRoot, join(bundleRoot, "nsis"), "dir");

  try {
    await expect(
      releaseWindowsBundle({ bundleRoot, releaseRoot: join(root, "发布-Windows") }),
    ).rejects.toThrow(/symbolic link/);
    expect(await readFile(setup, "utf8")).toBe("outside");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("refuses to overwrite an existing release and leaves the source untouched", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const setup = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  const published = join(releaseRoot, "CYLUNE-Setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await mkdir(releaseRoot, { recursive: true });
  await writeFile(setup, "new-release");
  await writeFile(published, "existing-release");

  try {
    await expect(releaseWindowsBundle({ bundleRoot, releaseRoot })).rejects.toThrow(
      /already exists/,
    );
    expect(await readFile(published, "utf8")).toBe("existing-release");
    expect(await readFile(setup, "utf8")).toBe("new-release");
    expect(await readdir(releaseRoot)).toEqual(["CYLUNE-Setup.exe"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("allows only one concurrent publisher to claim the release path", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const setup = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  const fixture = Buffer.alloc(64 * 1024, 0x43);
  await writeFile(setup, fixture);

  try {
    const results = await Promise.allSettled([
      releaseWindowsBundle({ bundleRoot, releaseRoot }),
      releaseWindowsBundle({ bundleRoot, releaseRoot }),
    ]);

    expect(results.filter(({ status }) => status === "fulfilled")).toHaveLength(1);
    expect(results.filter(({ status }) => status === "rejected")).toHaveLength(1);
    expect(await readFile(join(releaseRoot, "CYLUNE-Setup.exe"))).toEqual(fixture);
    expect(await readdir(releaseRoot)).toEqual(["CYLUNE-Setup.exe"]);
    await expect(access(setup)).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}, 20_000);

test("configures a bilingual current-user NSIS installer with bundled notices", async () => {
  const config = JSON.parse(
    await readFile(join(process.cwd(), "src-tauri", "tauri.windows.conf.json"), "utf8"),
  );

  expect(config.bundle.resources).toEqual({
    "../THIRD_PARTY_NOTICES.md": "THIRD_PARTY_NOTICES.md",
  });
  expect(config.bundle.windows.webviewInstallMode).toEqual({
    type: "downloadBootstrapper",
    silent: true,
  });
  expect(config.bundle.windows.nsis).toEqual({
    installMode: "currentUser",
    installerIcon: "icons/icon.ico",
    uninstallerIcon: "icons/icon.ico",
    languages: ["SimpChinese", "English"],
    displayLanguageSelector: true,
  });
});

test("recognizes only an output owned by the same in-memory release run", async () => {
  const module = await import("./release-windows.mjs");
  expect(typeof module.createWindowsReleaseRun).toBe("function");
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const setup = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await writeFile(setup, "same-run");

  try {
    const run = module.createWindowsReleaseRun();
    const first = await releaseWindowsBundle({ bundleRoot, releaseRoot, run });
    const recovered = await releaseWindowsBundle({ bundleRoot, releaseRoot, run });

    expect(recovered).toBe(first);
    await expect(
      releaseWindowsBundle({ bundleRoot, releaseRoot, run: Object.freeze({}) }),
    ).rejects.toThrow(/release run/);
    expect(await readFile(first, "utf8")).toBe("same-run");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("fails safely when the NSIS directory changes after its identity snapshot", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const nsisRoot = join(bundleRoot, "nsis");
  const heldNsisRoot = join(bundleRoot, "held-nsis");
  const externalRoot = join(root, "external-nsis");
  const source = join(nsisRoot, "CYLUNE_0.1.0_x64-setup.exe");
  const external = join(externalRoot, "CYLUNE_9.9.9_x64-setup.exe");
  await mkdir(nsisRoot, { recursive: true });
  await mkdir(externalRoot, { recursive: true });
  await writeFile(source, "owned");
  await writeFile(external, "external");
  let swapped = false;

  try {
    await expect(
      releaseWindowsBundle({
        bundleRoot,
        releaseRoot: join(root, "发布-Windows"),
        fileSystem: {
          async afterDirectorySnapshot(role) {
            if (role !== "nsis" || swapped) return;
            swapped = true;
            await rename(nsisRoot, heldNsisRoot);
            await symlink(
              externalRoot,
              nsisRoot,
              process.platform === "win32" ? "junction" : "dir",
            );
          },
        },
      }),
    ).rejects.toThrow(/NSIS directory changed/);
    expect(await readFile(join(heldNsisRoot, "CYLUNE_0.1.0_x64-setup.exe"), "utf8")).toBe(
      "owned",
    );
    expect(await readFile(external, "utf8")).toBe("external");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("fails without external writes when a symlink appears at the final publish boundary", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const externalRoot = join(root, "external-release");
  const source = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await mkdir(externalRoot, { recursive: true });
  await writeFile(source, "owned");
  let swapped = false;

  try {
    await expect(
      releaseWindowsBundle({
        bundleRoot,
        releaseRoot,
        fileSystem: {
          async beforePublish() {
            if (swapped) return;
            swapped = true;
            await symlink(
              externalRoot,
              releaseRoot,
              process.platform === "win32" ? "junction" : "dir",
            );
          },
        },
      }),
    ).rejects.toThrow(/release.*already exists|release path changed/i);
    expect(await readdir(externalRoot)).toEqual([]);
    expect(await readFile(source, "utf8")).toBe("owned");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("cleans only sibling staging when the release path changes after publish failure", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-win-release-"));
  const bundleRoot = join(root, "bundle");
  const releaseRoot = join(root, "发布-Windows");
  const heldReleaseRoot = join(root, "held-release");
  const externalRoot = join(root, "external-release");
  const source = join(bundleRoot, "nsis", "CYLUNE_0.1.0_x64-setup.exe");
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await mkdir(externalRoot, { recursive: true });
  await writeFile(source, "owned");
  await writeFile(join(externalRoot, "sentinel.txt"), "external");

  try {
    await expect(
      releaseWindowsBundle({
        bundleRoot,
        releaseRoot,
        fileSystem: {
          async beforePublish() {
            await mkdir(releaseRoot);
            await writeFile(join(releaseRoot, "sentinel.txt"), "existing");
          },
          async beforeStagingCleanup() {
            await rename(releaseRoot, heldReleaseRoot);
            await symlink(
              externalRoot,
              releaseRoot,
              process.platform === "win32" ? "junction" : "dir",
            );
          },
        },
      }),
    ).rejects.toThrow(/already exists/);
    expect(await readFile(join(heldReleaseRoot, "sentinel.txt"), "utf8")).toBe("existing");
    expect(await readFile(join(externalRoot, "sentinel.txt"), "utf8")).toBe("external");
    expect((await readdir(root)).filter((name) => name.startsWith(".cylune-release-"))).toEqual(
      [],
    );
    expect(await readFile(source, "utf8")).toBe("owned");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("verifies the Cargo binary that the NSIS bundle actually contains", async () => {
  const cargoManifest = await readFile(join(process.cwd(), "src-tauri", "Cargo.toml"), "utf8");
  const packageSection = cargoManifest.match(/\[package\]([\s\S]*?)(?:\n\[|$)/)?.[1];
  const binaryName = packageSection?.match(/^name\s*=\s*"([^"]+)"/m)?.[1];
  expect(binaryName).toBeTruthy();

  const workflow = await readFile(
    join(process.cwd(), ".github", "workflows", "windows.yml"),
    "utf8",
  );
  expect(workflow).toContain(`(Join-Path $targetRoot "${binaryName}.exe")`);
  expect(workflow).not.toContain('(Join-Path $targetRoot "CYLUNE.exe")');
});
