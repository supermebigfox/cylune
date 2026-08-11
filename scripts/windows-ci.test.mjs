import { access, readFile } from "node:fs/promises";
import { join } from "node:path";
import { describe, expect, test } from "vitest";

const root = process.cwd();

describe("Windows release CI policy", () => {
  test("tag builds never leave an unsigned NSIS bundle beside the signed one", async () => {
    const workflow = await readFile(
      join(root, ".github", "workflows", "windows.yml"),
      "utf8",
    );
    const previewStep = workflow.match(
      /- name: Build unsigned preview NSIS bundle([\s\S]*?)(?=\n      - name:)/,
    )?.[1];

    expect(previewStep).toBeTruthy();
    expect(previewStep).toContain("!startsWith(github.ref, 'refs/tags/')");
  });

  test("Windows runner compiles native tests and both HLSL shader entry points", async () => {
    const [workflow, gate, shaderGate] = await Promise.all([
      readFile(join(root, ".github", "workflows", "windows.yml"), "utf8"),
      readFile(join(root, "scripts", "win-native.ps1"), "utf8"),
      readFile(
        join(root, "src-tauri", "native", "windows", "hlsl_compile_test.cc"),
        "utf8",
      ),
    ]);

    expect(workflow).toContain("uses: ilammy/msvc-dev-cmd@v1");
    expect(workflow).toContain("./scripts/win-native.ps1");
    for (const fixture of [
      "callback_guard_test.cc",
      "capture_state_test.cc",
      "drop_state_test.cc",
      "drop_target_test.cc",
      "pet_bridge_test.cc",
      "render_state_test.cc",
      "window_state_test.cc",
      "animation_test.cc",
    ]) {
      expect(gate).toContain(fixture);
    }
    expect(gate).toContain("hlsl_compile_test.cc");
    expect(gate).toContain("BlackHole.hlsl");
    expect(gate).toContain("d3dcompiler.lib");
    expect(shaderGate).toContain('"vs_main", "vs_5_0"');
    expect(shaderGate).toContain('"ps_main", "ps_5_0"');
    expect(shaderGate).toContain("D3DCompileFromFile");
  });

  test("Windows installer inherits the CYLUNE identity and ships required resources", async () => {
    const [base, windows, packageJson] = await Promise.all([
      readFile(join(root, "src-tauri", "tauri.conf.json"), "utf8").then(JSON.parse),
      readFile(join(root, "src-tauri", "tauri.windows.conf.json"), "utf8").then(JSON.parse),
      readFile(join(root, "package.json"), "utf8").then(JSON.parse),
    ]);

    expect(base.productName).toBe("CYLUNE");
    expect(base.identifier).toBe("com.robin.cylune");
    expect(base.version).toBe(packageJson.version);
    expect(base.version).toMatch(/^\d+\.\d+\.\d+$/);
    expect(base.bundle.icon).toContain("icons/icon.ico");
    await expect(access(join(root, "src-tauri", "icons", "icon.ico"))).resolves.toBeUndefined();
    await expect(access(join(root, "THIRD_PARTY_NOTICES.md"))).resolves.toBeUndefined();
    expect(windows.bundle.resources).toEqual({
      "../THIRD_PARTY_NOTICES.md": "THIRD_PARTY_NOTICES.md",
    });
    expect(windows.bundle.windows.webviewInstallMode).toEqual({
      type: "downloadBootstrapper",
      silent: true,
    });
    expect(windows.bundle.windows.nsis).toMatchObject({
      installMode: "currentUser",
      installerIcon: "icons/icon.ico",
      uninstallerIcon: "icons/icon.ico",
      languages: ["SimpChinese", "English"],
      displayLanguageSelector: true,
    });
  });
});
