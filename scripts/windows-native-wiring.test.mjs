import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const windowSource = readFileSync(
  resolve("src-tauri/native/windows/window.cpp"),
  "utf8",
);
const captureSource = readFileSync(
  resolve("src-tauri/native/windows/capture.cpp"),
  "utf8",
);
const rendererSource = readFileSync(
  resolve("src-tauri/native/windows/renderer.cpp"),
  "utf8",
);
const shaderSource = readFileSync(
  resolve("src-tauri/native/windows/BlackHole.hlsl"),
  "utf8",
);
const buildScript = readFileSync(resolve("src-tauri/build.rs"), "utf8");

function section(start, end) {
  const first = windowSource.indexOf(start);
  const last = windowSource.indexOf(end, first + start.length);
  expect(first).toBeGreaterThanOrEqual(0);
  expect(last).toBeGreaterThan(first);
  return windowSource.slice(first, last);
}

function lastSection(start, end) {
  const first = windowSource.lastIndexOf(start);
  const last = windowSource.indexOf(end, first + start.length);
  expect(first).toBeGreaterThanOrEqual(0);
  expect(last).toBeGreaterThan(first);
  return windowSource.slice(first, last);
}

describe("Windows desktop capture wiring", () => {
  it("embeds the Common Controls v6 manifest into Rust test executables", () => {
    expect(buildScript).not.toContain("cargo:rustc-link-arg-tests=");
    expect(buildScript).not.toContain("cargo:rustc-link-arg=/MANIFEST:EMBED");
    expect(buildScript).toContain(
      "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:",
    );
    expect(buildScript).toContain("Microsoft.Windows.Common-Controls");
    expect(buildScript).toContain("version='6.0.0.0'");
    expect(buildScript).toContain("processorArchitecture='amd64'");
  });

  it("creates visual panes lazily instead of requiring every monitor renderer", () => {
    const createPanes = section(
      "bool createVisualPanes()",
      "bool retireCapture()",
    );
    expect(createPanes).not.toContain("BlackHoleRenderer::create(");

    const activatePane = section(
      "bool activateVisualPane(",
      "void initializeRenderer(",
    );
    expect(activatePane).toContain("renderer = activePane->renderer.get()");
    expect(activatePane).toContain("BlackHoleRenderer::create(");
  });

  it("limits the protected visual window to the effect bounds", () => {
    const createPanes = section(
      "bool createVisualPanes()",
      "bool retireCapture()",
    );
    expect(createPanes).not.toContain("monitor.physical.left, monitor.physical.top,\n                                  width, height");

    const position = section("bool positionWindow()", "bool resetPosition()");
    expect(position).toContain("VisualEffectBounds(");
    expect(position).toContain("positionVisualPane(");
  });

  it("maps the cropped effect surface back into the live desktop texture", () => {
    expect(shaderSource).toContain("float2 captureOrigin");
    expect(shaderSource).toContain("float2 captureScale");
    expect(shaderSource).toContain("captureUV(");
    expect(rendererSource).toContain("params.captureOrigin[0]");
    expect(rendererSource).toContain("params.captureScale[0]");
  });

  it("retries the complete render pipeline with the system WARP renderer", () => {
    expect(rendererSource).toContain("D3D_DRIVER_TYPE_WARP");
    expect(rendererSource).toContain(
      "initialize(window, hlslSource, monitor, true)",
    );
  });

  it("joins capture before OLE revoke and DComp/window destruction", () => {
    const ownerStop = section("if (stopRequested) {", "DWORD timeout = INFINITE");
    const captureStop = ownerStop.indexOf("retireCapture()");
    const oleRevoke = ownerStop.indexOf("revokeDropTarget(window)");
    const rendererRelease = ownerStop.indexOf("releaseRenderersAfterCaptureStop(true)");
    const visualDestroy = ownerStop.indexOf("destroyVisualPanes()");
    expect(captureStop).toBeGreaterThanOrEqual(0);
    expect(oleRevoke).toBeGreaterThan(captureStop);
    expect(rendererRelease).toBeGreaterThan(oleRevoke);
    expect(visualDestroy).toBeGreaterThan(rendererRelease);
  });

  it("does not publish capability-unavailable for hide or suspend", () => {
    const hide = section("void hideWindow()", "void recoverForDisplays(");
    expect(hide).toContain("requestCapturePause(true)");
    expect(hide).not.toContain("PET_CAPTURE_UNAVAILABLE");

    const power = section("case WM_POWERBROADCAST:", "case WM_ERASEBKGND:");
    expect(power).toContain("requestCapturePause(true)");
    expect(power).not.toContain("PET_CAPTURE_UNAVAILABLE");
  });

  it("rejects frame identity mismatches and classifies DEVICE_RESET as device loss", () => {
    expect(windowSource).toContain("CaptureFrameMatchesOwner(ownerIdentity, frameIdentity)");
    expect(captureSource).toContain("CaptureEvent::DeviceReset");
  });

  it("keeps the last desktop texture when duplication reports no new frame", () => {
    const timeoutStart = captureSource.indexOf(
      "if (acquired == DXGI_ERROR_WAIT_TIMEOUT)",
    );
    const accessLostStart = captureSource.indexOf(
      "if (acquired == DXGI_ERROR_ACCESS_LOST",
      timeoutStart,
    );
    expect(timeoutStart).toBeGreaterThanOrEqual(0);
    expect(accessLostStart).toBeGreaterThan(timeoutStart);
    const timeoutBranch = captureSource.slice(timeoutStart, accessLostStart);
    expect(timeoutBranch).toContain("CaptureEvent::Timeout");
    expect(timeoutBranch).not.toContain("clearFrameLocked()");
  });

  it("keeps unexpected NCDESTROY on the owner stop path", () => {
    const destroy = lastSection("case WM_NCDESTROY:", "default:");
    expect(destroy).toContain("inputWindowGone = true");
    expect(destroy).toContain("SetEvent(stopEvent)");
    expect(destroy).not.toContain("shutdownRenderer(");
    expect(destroy).not.toContain("PostQuitMessage(");
  });
});
