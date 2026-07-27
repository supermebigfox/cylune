import { expect, it, vi } from "vitest";
import { createTauriApi } from "./tauri";

it("passes Rust command names and snake-case payloads through the typed adapter", async () => {
  const invoke = vi.fn(async () => ({ job_id: "job-1" }));
  const api = createTauriApi(invoke);

  await api.mountSpool(3, "spool-blue");
  await api.settleJob("job-1", { kind: "failed", stop_layer: 18 });
  await api.confirmNewPrint("hash-1");

  expect(invoke).toHaveBeenNthCalledWith(1, "mount_spool", {
    slotNumber: 3,
    spoolId: "spool-blue",
  });
  expect(invoke).toHaveBeenNthCalledWith(2, "settle_job", {
    jobId: "job-1",
    outcome: { kind: "failed", stop_layer: 18 },
  });
  expect(invoke).toHaveBeenNthCalledWith(3, "confirm_new_print", {
    sourceHash: "hash-1",
  });
});

it("provides deterministic browser data without activating demo mode in Tauri", async () => {
  const browserApi = createTauriApi(undefined, {});
  const tauriInvoke = vi.fn(async () => []);
  const tauriApi = createTauriApi(tauriInvoke, { __TAURI_INTERNALS__: {} });

  expect((await browserApi.listSpools()).length).toBeGreaterThan(4);
  await tauriApi.listSpools();
  expect(tauriInvoke).toHaveBeenCalledWith("list_spools", undefined);
});
