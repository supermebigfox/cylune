import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
import { afterEach, expect, test } from "vitest";
import { checkMacNativeSeal } from "./check-mac-native-seal.mjs";

const roots = [];

afterEach(async () => {
  await Promise.all(
    roots.splice(0).map((root) => rm(root, { recursive: true, force: true })),
  );
});

function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

async function repository() {
  const root = await mkdtemp(join(tmpdir(), "cylune-mac-seal-"));
  roots.push(root);
  git(root, "init", "--quiet");
  git(root, "config", "user.name", "Seal Test");
  git(root, "config", "user.email", "seal@example.invalid");
  await mkdir(join(root, "src-tauri/native/mac"), { recursive: true });
  await mkdir(join(root, "src-tauri/native/windows"), { recursive: true });
  await writeFile(join(root, "src-tauri/native/mac/pet.mm"), "sealed\n");
  await writeFile(join(root, "src-tauri/native/windows/window.cpp"), "base\n");
  git(root, "add", ".");
  git(root, "commit", "--quiet", "-m", "seal");
  return { root, reference: git(root, "rev-parse", "HEAD") };
}

test("passes when only Windows native files differ from the reference", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, "src-tauri/native/windows/window.cpp"), "changed\n");

  await expect(checkMacNativeSeal({ cwd: root, reference })).resolves.toEqual({
    reference,
    tree: git(root, "rev-parse", `${reference}:src-tauri/native/mac`),
  });
});

test("fails when a tracked Mac native file differs from the reference", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, "src-tauri/native/mac/pet.mm"), "changed\n");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac differs",
  );
});

test("fails when a Mac native change is staged", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, "src-tauri/native/mac/pet.mm"), "staged\n");
  git(root, "add", "src-tauri/native/mac/pet.mm");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac differs",
  );
});

test("fails when a committed Mac native mutation leaves a clean worktree", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, "src-tauri/native/mac/pet.mm"), "committed\n");
  git(root, "add", "src-tauri/native/mac/pet.mm");
  git(root, "commit", "--quiet", "-m", "mutate sealed Mac native file");
  expect(git(root, "status", "--short")).toBe("");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac differs",
  );
});

test("fails when a sealed index masks a committed Mac native mutation", async () => {
  const { root, reference } = await repository();
  const path = join(root, "src-tauri/native/mac/pet.mm");
  await writeFile(path, "committed\n");
  git(root, "add", "src-tauri/native/mac/pet.mm");
  git(root, "commit", "--quiet", "-m", "mutate sealed Mac native file");
  await writeFile(path, "sealed\n");
  git(root, "add", "src-tauri/native/mac/pet.mm");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac differs",
  );
});

test("fails when an unstaged restore hides a staged Mac native change", async () => {
  const { root, reference } = await repository();
  const path = join(root, "src-tauri/native/mac/pet.mm");
  await writeFile(path, "staged\n");
  git(root, "add", "src-tauri/native/mac/pet.mm");
  await writeFile(path, "sealed\n");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac differs",
  );
});

test("fails when a Mac native file is deleted", async () => {
  const { root, reference } = await repository();
  await rm(join(root, "src-tauri/native/mac/pet.mm"));

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "src-tauri/native/mac/pet.mm",
  );
});

test("fails when an untracked Mac native path is present", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, "src-tauri/native/mac/untracked.h"), "new\n");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "untracked.h",
  );
});

test("fails when an ignored Mac native path is present", async () => {
  const { root, reference } = await repository();
  await writeFile(join(root, ".gitignore"), "src-tauri/native/mac/ignored.h\n");
  await writeFile(join(root, "src-tauri/native/mac/ignored.h"), "ignored\n");

  await expect(checkMacNativeSeal({ cwd: root, reference })).rejects.toThrow(
    "ignored.h",
  );
});

test("reports a Git error separately from a seal difference", async () => {
  const { root } = await repository();

  await expect(
    checkMacNativeSeal({ cwd: root, reference: "missing-reference" }),
  ).rejects.toThrow("Cannot resolve Mac native reference missing-reference");
});

test("reports a clear error outside a Git repository", async () => {
  const root = await mkdtemp(join(tmpdir(), "cylune-mac-seal-nonrepo-"));
  roots.push(root);

  await expect(checkMacNativeSeal({ cwd: root })).rejects.toThrow(
    "Cannot resolve Mac native reference d640d92",
  );
});
