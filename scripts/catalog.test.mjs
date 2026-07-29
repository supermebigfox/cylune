import { execFileSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "vitest";

test("generator stores the parser-compatible preset base without @base", () => {
  const workspace = mkdtempSync(join(tmpdir(), "bambu-catalog-generator-"));
  const profileDirectory = join(workspace, "profiles");
  const outputDirectory = join(workspace, "src", "catalog");
  const colorFile = join(workspace, "colors.json");
  mkdirSync(profileDirectory, { recursive: true });
  mkdirSync(outputDirectory, { recursive: true });
  writeFileSync(
    join(profileDirectory, "Bambu PLA Basic @base.json"),
    JSON.stringify({
      name: "Bambu PLA Basic @base",
      filament_type: ["PLA"],
    }),
  );
  writeFileSync(
    colorFile,
    JSON.stringify({
      data: [{
        fila_id: "GFA00",
        fila_type: "PLA Basic",
        fila_color_code: "10100",
        fila_color_type: "单色",
        fila_color: ["#FFFFFF"],
        fila_color_name: { zh: "玉石白", en: "Jade White" },
      }],
    }),
  );

  try {
    const script = resolve(
      dirname(fileURLToPath(import.meta.url)),
      "catalog.mjs",
    );
    execFileSync(process.execPath, [script, colorFile, profileDirectory], {
      cwd: workspace,
    });
    const generated = JSON.parse(
      readFileSync(join(outputDirectory, "bambu.json"), "utf8"),
    );

    expect(generated.entries[0].presetBase).toBe("Bambu PLA Basic");
  } finally {
    rmSync(workspace, { recursive: true, force: true });
  }
});
