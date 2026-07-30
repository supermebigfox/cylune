# CYLUNE Cancel Import Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let users discard an accidentally imported, unsettled print task without changing filament balances, and keep only the published CYLUNE app visible to Spotlight.

**Architecture:** Add a transactional `PrintService::discard_pending_job` operation and expose it through one Tauri command. The React job page asks for confirmation, calls the command through `TauriApi`, then clears the active preview. A release helper copies signed bundles to `发布` and removes the temporary `.app` from `target` after the DMG has been built.

**Tech Stack:** Rust, rusqlite, Tauri 2, React 18, TypeScript, Vitest, Node.js filesystem APIs.

## Global Constraints

- Only an unsettled job where `outcome IS NULL` may be discarded.
- Discarding never writes ledger events, changes spool balances, or changes AMS slots.
- `parse_cache` remains available for fast re-import.
- Simplified Chinese, Traditional Chinese, and English copy must stay aligned.
- The published macOS test artifact remains `发布/CYLUNE.app`; the temporary bundled app must not remain indexed after publishing.

---

### Task 1: Transactional backend discard

**Files:**
- Modify: `src-tauri/src/imports.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/imports.rs`

**Interfaces:**
- Produces: `PrintService::discard_pending_job(&mut self, job_id: Uuid) -> Result<()>`
- Produces: Tauri command `discard_pending_job(job_id, state, runtime) -> Result<()>`

- [ ] **Step 1: Write the failing service tests**

Add tests that import the existing sliced fixture, confirm mappings, record spool balances/ledger count/cache count, discard the pending job, then assert job and mapping counts are zero while balances, ledger, slots, and parse cache are unchanged. Add a second test that settles a job and asserts discard returns `invalid_job` without mutations.

- [ ] **Step 2: Run the tests and verify RED**

Run: `cargo test discard_pending_job -- --nocapture`
Expected: compilation fails because `discard_pending_job` does not exist.

- [ ] **Step 3: Implement the transaction**

Implement the method with this order inside one SQLite transaction:

```rust
let outcome = transaction
    .query_row(
        "SELECT outcome FROM print_jobs WHERE job_id = ?1",
        [job_id.to_string()],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()?;
if outcome != Some(None) {
    return Err(AppError::InvalidJob);
}
transaction.execute("DELETE FROM job_mappings WHERE job_id = ?1", [job_id.to_string()])?;
transaction.execute(
    "DELETE FROM app_settings WHERE setting_key = 'pending_job_id' AND setting_value = ?1",
    [job_id.to_string()],
)?;
transaction.execute("DELETE FROM print_jobs WHERE job_id = ?1", [job_id.to_string()])?;
transaction.commit()?;
```

Expose a command that refreshes `PetRuntime` with the new `pending_summary()` and register it in `tauri::generate_handler!`.

- [ ] **Step 4: Run backend tests and verify GREEN**

Run: `cargo test discard_pending_job -- --nocapture`
Expected: both discard tests pass.

---

### Task 2: API contract and localized job-page action

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/lib/tauri.test.ts`
- Modify: `src/features/jobs/Job.tsx`
- Modify: `src/features/jobs/Job.test.tsx`
- Modify: `src/i18n/locales/zh-CN.json`
- Modify: `src/i18n/locales/zh-TW.json`
- Modify: `src/i18n/locales/en.json`

**Interfaces:**
- Consumes: native command `discard_pending_job`
- Produces: `TauriApi.discardPendingJob(jobId: string): Promise<void>`
- Produces: `Job` callback `onDiscard(jobId: string): boolean | void | Promise<boolean | void>`

- [ ] **Step 1: Write failing component and API tests**

Add a Job test that stubs `window.confirm` to `true`, clicks “取消此次导入”, and expects `onDiscard("job-mask")`. Add a negative test with `window.confirm` returning `false`, and a settled-state test asserting the button is absent. Extend the Tauri client test to expect `invoke("discard_pending_job", { jobId: "job-mask" })`.

- [ ] **Step 2: Run targeted frontend tests and verify RED**

Run: `npm test -- --run src/features/jobs/Job.test.tsx src/lib/tauri.test.ts`
Expected: tests fail because the API and button do not exist.

- [ ] **Step 3: Implement API, button, and translations**

Add the API method to demo and native clients. Add `onDiscard` to `Job`, import `Trash`, and render the secondary action only when `!settled && preview.state !== "new_print_confirmation_required"`:

```tsx
<button
  className="ghost full"
  disabled={busy}
  onClick={() => window.confirm(copy("jobs.discardConfirm")) && onDiscard(preview.job_id)}
>
  <Trash size={17} />{copy("jobs.discardImport")}
</button>
```

Use copy:

- zh-CN: `取消此次导入` / `确定取消此次导入吗？不会扣减任何耗材。`
- zh-TW: `取消此次匯入` / `確定取消此次匯入嗎？不會扣減任何耗材。`
- en: `Discard this import` / `Discard this import? No filament will be deducted.`

- [ ] **Step 4: Run targeted tests and verify GREEN**

Run: `npm test -- --run src/features/jobs/Job.test.tsx src/lib/tauri.test.ts`
Expected: all targeted tests pass.

---

### Task 3: Application state reset

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

**Interfaces:**
- Consumes: `TauriApi.discardPendingJob(jobId)`
- Produces: `actions.discard(jobId)` that clears the active preview after native success.

- [ ] **Step 1: Write the failing application test**

Import `demoPreview`, navigate to the job page, confirm discard, and assert the API receives `demoPreview.job_id`, the filename disappears, the empty-job message appears, and inventory refresh methods run again. Add a rejection test asserting the preview remains visible.

- [ ] **Step 2: Run the App test and verify RED**

Run: `npm test -- --run src/App.test.tsx`
Expected: failure because `DesktopApp` does not pass an `onDiscard` action.

- [ ] **Step 3: Implement the state transition**

Add:

```ts
discard: (jobId: string) => runAction("discard", async () => {
  await apiClient.discardPendingJob(jobId);
  setPreview(null);
  setSettled(false);
  setResult(null);
  await loadInventory();
}),
```

Pass it to `<Job onDiscard={actions.discard} />`. Do not clear the preview before the API succeeds.

- [ ] **Step 4: Run App tests and verify GREEN**

Run: `npm test -- --run src/App.test.tsx`
Expected: all App tests pass.

---

### Task 4: Publish one searchable CYLUNE app

**Files:**
- Create: `scripts/release-mac.mjs`
- Create: `scripts/release-mac.test.mjs`
- Modify: `package.json`

**Interfaces:**
- Produces: `publishMacBundles({ sourceApp, sourceDmg, releaseApp, releaseDmg })`
- Produces: `npm run release:mac`

- [ ] **Step 1: Write the failing release-helper test**

In a temporary directory create mock source app/DMG files and an old published app. Call `publishMacBundles`, then assert the published files equal the source files and `sourceApp` no longer exists while `sourceDmg` remains.

- [ ] **Step 2: Run the helper test and verify RED**

Run: `npm test -- --run scripts/release-mac.test.mjs`
Expected: import fails because `release-mac.mjs` does not exist.

- [ ] **Step 3: Implement the release helper**

Use `node:fs/promises` `rm`, `mkdir`, `cp`, and `copyFile`. When run directly, execute `npm run tauri build`, copy bundles to `发布`, then remove only `src-tauri/target/release/bundle/macos/CYLUNE.app`. Do not remove the DMG build source.

Add package script:

```json
"release:mac": "node scripts/release-mac.mjs"
```

- [ ] **Step 4: Run helper tests and verify GREEN**

Run: `npm test -- --run scripts/release-mac.test.mjs`
Expected: release-helper test passes.

---

### Task 5: Full verification and packaging

**Files:**
- Verify all modified files above.

**Interfaces:**
- Consumes all preceding tasks.
- Produces the signed `发布/CYLUNE.app` and `发布/CYLUNE.dmg` test artifacts.

- [ ] **Step 1: Run full test and format suites**

Run: `cargo fmt -- --check && cargo test`
Run: `npm test -- --run && npm run build`
Expected: zero failures.

- [ ] **Step 2: Build and publish once**

Run: `npm run release:mac`
Expected: signed app and valid DMG exist in `发布`; temporary bundled app is absent.

- [ ] **Step 3: Verify artifacts and Spotlight result**

Run: `codesign --verify --deep --strict 发布/CYLUNE.app`
Run: `hdiutil verify 发布/CYLUNE.dmg`
Run: `mdfind 'kMDItemCFBundleIdentifier == "com.robin.cylune"'`
Expected: signature and DMG are valid; only `发布/CYLUNE.app` is returned.

- [ ] **Step 4: Commit implementation**

```bash
git add docs/superpowers src-tauri/src/imports.rs src-tauri/src/lib.rs src/lib/tauri.ts src/lib/tauri.test.ts src/features/jobs/Job.tsx src/features/jobs/Job.test.tsx src/App.tsx src/App.test.tsx src/i18n/locales scripts package.json
git commit -m "feat: allow discarding pending imports"
```
