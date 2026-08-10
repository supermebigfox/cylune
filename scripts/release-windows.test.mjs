import { access, mkdtemp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
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
  await mkdir(join(bundleRoot, "nsis"), { recursive: true });
  await writeFile(setup, "fixture");
  await writeFile(nearbyArtifact, "preserve");

  try {
    const published = await releaseWindowsBundle({ bundleRoot, releaseRoot });

    expect(published).toBe(join(releaseRoot, "CYLUNE-Setup.exe"));
    expect(await readFile(published, "utf8")).toBe("fixture");
    await expect(access(setup)).rejects.toThrow();
    expect(await readFile(nearbyArtifact, "utf8")).toBe("preserve");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
