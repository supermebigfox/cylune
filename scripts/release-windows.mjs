import { spawnSync } from "node:child_process";
import { constants } from "node:fs";
import {
  lstat,
  mkdir,
  mkdtemp,
  open,
  readdir,
  realpath,
  rename,
  rm,
} from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { rustTargetDir } from "./rust.mjs";

const releaseRuns = new WeakMap();
const defaultFileSystem = Object.freeze({
  lstat,
  mkdir,
  mkdtemp,
  open,
  readdir,
  realpath,
  rename,
  rm,
});

export function createWindowsReleaseRun() {
  const run = Object.freeze({});
  releaseRuns.set(run, { outputs: new Map() });
  return run;
}

function releaseRunState(run) {
  const effectiveRun = run ?? createWindowsReleaseRun();
  const state = releaseRuns.get(effectiveRun);
  if (!state) throw new Error("Invalid Windows release run context");
  return state;
}

function withFileSystem(overrides) {
  return overrides ? { ...defaultFileSystem, ...overrides } : defaultFileSystem;
}

function sameFile(left, right) {
  return left.dev === right.dev && left.ino === right.ino;
}

async function snapshotDirectory(fileSystem, path, role, label) {
  const before = await fileSystem.lstat(path);
  if (before.isSymbolicLink()) {
    throw new Error(`Refusing symbolic link ${label} directory: ${path}`);
  }
  if (!before.isDirectory()) throw new Error(`Expected ${label} directory: ${path}`);
  const canonical = await fileSystem.realpath(path);
  const after = await fileSystem.lstat(path);
  if (!after.isDirectory() || after.isSymbolicLink() || !sameFile(before, after)) {
    throw new Error(`${label} directory changed while its identity was captured: ${path}`);
  }
  const snapshot = { canonical, identity: after };
  await fileSystem.afterDirectorySnapshot?.(role, path);
  return snapshot;
}

async function assertDirectoryIdentity(fileSystem, path, snapshot, label) {
  try {
    const [current, canonical] = await Promise.all([
      fileSystem.lstat(path),
      fileSystem.realpath(path),
    ]);
    if (
      current.isSymbolicLink() ||
      !current.isDirectory() ||
      !sameFile(current, snapshot.identity) ||
      canonical !== snapshot.canonical
    ) {
      throw new Error("identity mismatch");
    }
  } catch (cause) {
    throw new Error(`${label} directory changed after validation: ${path}`, { cause });
  }
}

async function openRegularFileWithoutFollowingLinks(fileSystem, path, directory, directoryPath) {
  await assertDirectoryIdentity(fileSystem, directoryPath, directory, "NSIS");
  const beforeOpen = await fileSystem.lstat(path);
  if (beforeOpen.isSymbolicLink()) {
    throw new Error(`Refusing symbolic link NSIS setup: ${path}`);
  }
  if (!beforeOpen.isFile()) {
    throw new Error(`Expected NSIS setup to be a regular file: ${path}`);
  }

  const noFollow = constants.O_NOFOLLOW ?? 0;
  let handle;
  try {
    handle = await fileSystem.open(path, constants.O_RDONLY | noFollow);
    const [opened, afterOpen] = await Promise.all([handle.stat(), fileSystem.lstat(path)]);
    await assertDirectoryIdentity(fileSystem, directoryPath, directory, "NSIS");
    if (
      !opened.isFile() ||
      afterOpen.isSymbolicLink() ||
      !sameFile(beforeOpen, opened) ||
      !sameFile(opened, afterOpen)
    ) {
      throw new Error(`NSIS setup changed while it was being opened: ${path}`);
    }
    return { handle, identity: opened };
  } catch (error) {
    await handle?.close();
    if (error?.code === "ELOOP") {
      throw new Error(`Refusing symbolic link NSIS setup: ${path}`, { cause: error });
    }
    throw error;
  }
}

async function copyBoundFile(source, destination) {
  const buffer = Buffer.allocUnsafe(64 * 1024);
  let position = 0;
  while (true) {
    const { bytesRead } = await source.read(buffer, 0, buffer.length, position);
    if (bytesRead === 0) break;
    let bytesWritten = 0;
    while (bytesWritten < bytesRead) {
      const result = await destination.write(
        buffer,
        bytesWritten,
        bytesRead - bytesWritten,
        position + bytesWritten,
      );
      bytesWritten += result.bytesWritten;
    }
    position += bytesRead;
  }
  await destination.sync();
}

async function removeSourceIfStillBound(fileSystem, path, identity, directory, directoryPath) {
  try {
    await assertDirectoryIdentity(fileSystem, directoryPath, directory, "NSIS");
    const current = await fileSystem.lstat(path);
    if (!current.isSymbolicLink() && current.isFile() && sameFile(current, identity)) {
      await assertDirectoryIdentity(fileSystem, directoryPath, directory, "NSIS");
      await fileSystem.rm(path);
    }
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }
}

export async function releaseWindowsBundle({
  bundleRoot,
  releaseRoot,
  run,
  fileSystem: fileSystemOverrides,
}) {
  const runState = releaseRunState(run);
  const fileSystem = withFileSystem(fileSystemOverrides);
  const nsisRoot = join(bundleRoot, "nsis");
  const publishedSetup = join(releaseRoot, "CYLUNE-Setup.exe");
  const releaseParent = dirname(releaseRoot);
  await fileSystem.mkdir(releaseParent, { recursive: true });
  const releaseParentDirectory = await snapshotDirectory(
    fileSystem,
    releaseParent,
    "release-parent",
    "release parent",
  );
  await assertDirectoryIdentity(
    fileSystem,
    releaseParent,
    releaseParentDirectory,
    "release parent",
  );

  const ownedOutput = runState.outputs.get(publishedSetup);
  if (ownedOutput) {
    const [currentRoot, currentOutput] = await Promise.all([
      fileSystem.lstat(releaseRoot),
      fileSystem.lstat(publishedSetup),
    ]);
    await assertDirectoryIdentity(
      fileSystem,
      releaseParent,
      releaseParentDirectory,
      "release parent",
    );
    if (
      !currentRoot.isSymbolicLink() &&
      currentRoot.isDirectory() &&
      sameFile(currentRoot, ownedOutput.root) &&
      !currentOutput.isSymbolicLink() &&
      currentOutput.isFile() &&
      sameFile(currentOutput, ownedOutput.file)
    ) {
      return publishedSetup;
    }
    runState.outputs.delete(publishedSetup);
  }

  try {
    await fileSystem.lstat(publishedSetup);
    throw new Error(`Refusing to overwrite release that already exists: ${publishedSetup}`);
  } catch (error) {
    if (error?.code !== "ENOENT") throw error;
  }

  const nsisDirectory = await snapshotDirectory(fileSystem, nsisRoot, "nsis", "NSIS");
  await assertDirectoryIdentity(fileSystem, nsisRoot, nsisDirectory, "NSIS");

  const setupFiles = (await fileSystem.readdir(nsisRoot)).filter((name) =>
    name.endsWith("-setup.exe"),
  );
  await assertDirectoryIdentity(fileSystem, nsisRoot, nsisDirectory, "NSIS");
  if (setupFiles.length !== 1) {
    throw new Error(`Expected exactly one NSIS setup in ${nsisRoot}, found ${setupFiles.length}`);
  }

  const sourceSetup = join(nsisRoot, setupFiles[0]);
  const { handle: source, identity } = await openRegularFileWithoutFollowingLinks(
    fileSystem,
    sourceSetup,
    nsisDirectory,
    nsisRoot,
  );
  await assertDirectoryIdentity(
    fileSystem,
    releaseParent,
    releaseParentDirectory,
    "release parent",
  );
  let stagingRoot = await fileSystem.mkdtemp(join(releaseParent, ".cylune-release-"));
  const stagingDirectory = await snapshotDirectory(
    fileSystem,
    stagingRoot,
    "staging",
    "staging release",
  );
  const stagingSetup = join(stagingRoot, "CYLUNE-Setup.exe");
  let staging;
  let published = false;
  try {
    await assertDirectoryIdentity(fileSystem, stagingRoot, stagingDirectory, "staging release");
    staging = await fileSystem.open(
      stagingSetup,
      constants.O_CREAT |
        constants.O_EXCL |
        constants.O_WRONLY |
        (constants.O_NOFOLLOW ?? 0),
      0o600,
    );
    const [openedStaging, pathStaging] = await Promise.all([
      staging.stat(),
      fileSystem.lstat(stagingSetup),
    ]);
    await assertDirectoryIdentity(fileSystem, stagingRoot, stagingDirectory, "staging release");
    if (
      !openedStaging.isFile() ||
      pathStaging.isSymbolicLink() ||
      !pathStaging.isFile() ||
      !sameFile(openedStaging, pathStaging)
    ) {
      throw new Error(`Staging setup changed while it was being opened: ${stagingSetup}`);
    }
    await copyBoundFile(source, staging);
    await staging.close();
    staging = undefined;
    await assertDirectoryIdentity(fileSystem, stagingRoot, stagingDirectory, "staging release");
    await fileSystem.beforePublish?.(stagingRoot, releaseRoot);
    await assertDirectoryIdentity(
      fileSystem,
      releaseParent,
      releaseParentDirectory,
      "release parent",
    );
    try {
      await fileSystem.lstat(releaseRoot);
      throw new Error(`Refusing to overwrite release that already exists: ${publishedSetup}`);
    } catch (error) {
      if (error?.code !== "ENOENT") throw error;
    }
    try {
      await fileSystem.rename(stagingRoot, releaseRoot);
    } catch (error) {
      if (["EEXIST", "ENOTEMPTY", "EPERM"].includes(error?.code)) {
        throw new Error(
          `Refusing to overwrite release that already exists: ${publishedSetup}`,
          { cause: error },
        );
      }
      throw error;
    }
    stagingRoot = undefined;
    await assertDirectoryIdentity(
      fileSystem,
      releaseParent,
      releaseParentDirectory,
      "release parent",
    );
    const [outputRootIdentity, outputFileIdentity] = await Promise.all([
      fileSystem.lstat(releaseRoot),
      fileSystem.lstat(publishedSetup),
    ]);
    runState.outputs.set(publishedSetup, {
      root: outputRootIdentity,
      file: outputFileIdentity,
    });
    published = true;
  } finally {
    const cleanupErrors = [];
    for (const result of await Promise.allSettled([staging?.close(), source.close()])) {
      if (result.status === "rejected") cleanupErrors.push(result.reason);
    }
    try {
      if (stagingRoot) {
        await fileSystem.beforeStagingCleanup?.(stagingRoot, releaseRoot);
        await assertDirectoryIdentity(
          fileSystem,
          releaseParent,
          releaseParentDirectory,
          "release parent",
        );
        await fileSystem.rm(stagingRoot, { recursive: true, force: true });
      }
    } catch (error) {
      cleanupErrors.push(error);
    }
    if (cleanupErrors.length === 1) throw cleanupErrors[0];
    if (cleanupErrors.length > 1) {
      throw new AggregateError(cleanupErrors, "Windows release cleanup failed");
    }
  }
  if (published) {
    await removeSourceIfStillBound(
      fileSystem,
      sourceSetup,
      identity,
      nsisDirectory,
      nsisRoot,
    );
  }
  return publishedSetup;
}

async function main() {
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  const target = rustTargetDir();
  const build = spawnSync("npm", ["run", "tauri", "build", "--", "--bundles", "nsis"], {
    cwd: root,
    env: { ...process.env, CARGO_TARGET_DIR: target },
    shell: process.platform === "win32",
    stdio: "inherit",
  });
  if (build.error) throw build.error;
  if (build.status !== 0) process.exit(build.status ?? 1);

  const run = createWindowsReleaseRun();
  await releaseWindowsBundle({
    bundleRoot: join(target, "release", "bundle"),
    releaseRoot: join(root, "发布-Windows"),
    run,
  });
}

const invokedPath = process.argv[1] ? resolve(process.argv[1]) : "";
if (invokedPath === fileURLToPath(import.meta.url)) {
  await main();
}
