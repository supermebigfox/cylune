import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

const exec = promisify(execFile);

export const MAC_NATIVE_REFERENCE = "d640d92";
const MAC_NATIVE_PATH = "src-tauri/native/mac";

async function git(cwd, args, context) {
  try {
    const { stdout } = await exec("git", args, {
      cwd,
      encoding: "utf8",
      maxBuffer: 1024 * 1024,
    });
    return stdout.trim();
  } catch (cause) {
    const detail =
      typeof cause?.stderr === "string" && cause.stderr.trim()
        ? cause.stderr.trim()
        : cause instanceof Error
          ? cause.message
          : String(cause);
    throw new Error(`${context}: ${detail}`, { cause });
  }
}

export async function checkMacNativeSeal({
  cwd = process.cwd(),
  reference = MAC_NATIVE_REFERENCE,
} = {}) {
  const tree = await git(
    cwd,
    ["rev-parse", `${reference}:${MAC_NATIVE_PATH}`],
    `Cannot resolve Mac native reference ${reference}`,
  );
  const committed = await git(
    cwd,
    ["diff", "--name-status", reference, "HEAD", "--", MAC_NATIVE_PATH],
    `Cannot compare HEAD for ${MAC_NATIVE_PATH} with ${reference}`,
  );
  const indexed = await git(
    cwd,
    ["diff", "--cached", "--name-status", "HEAD", "--", MAC_NATIVE_PATH],
    `Cannot compare the index for ${MAC_NATIVE_PATH} with HEAD`,
  );
  const unstaged = await git(
    cwd,
    ["diff", "--name-status", "--", MAC_NATIVE_PATH],
    `Cannot compare the worktree for ${MAC_NATIVE_PATH} with the index`,
  );
  const untracked = await git(
    cwd,
    ["ls-files", "--others", "--exclude-standard", "--", MAC_NATIVE_PATH],
    `Cannot enumerate untracked paths under ${MAC_NATIVE_PATH}`,
  );
  const ignored = await git(
    cwd,
    ["ls-files", "--others", "--ignored", "--exclude-standard", "--", MAC_NATIVE_PATH],
    `Cannot enumerate ignored paths under ${MAC_NATIVE_PATH}`,
  );

  if (committed || indexed || unstaged || untracked || ignored) {
    const paths = [committed, indexed, unstaged, untracked, ignored]
      .filter(Boolean)
      .join("\n");
    throw new Error(
      `${MAC_NATIVE_PATH} differs from ${reference}${paths ? `:\n${paths}` : ""}`,
    );
  }

  return { reference, tree };
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  checkMacNativeSeal()
    .then(({ reference, tree }) => {
      process.stdout.write(`Mac native seal matches ${reference} (${tree})\n`);
    })
    .catch((error) => {
      process.stderr.write(`${error instanceof Error ? error.message : error}\n`);
      process.exitCode = 1;
    });
}
