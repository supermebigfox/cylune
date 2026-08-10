#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif
#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include "window.h"

#include "callback_guard.h"
#include "drop_target.h"
#include "renderer.h"
#include "render_state.h"
#include "window_state.h"

#include <windows.h>

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstdint>
#include <limits>
#include <mutex>
#include <new>
#include <string>
#include <utility>
#include <vector>

#include <process.h>

#ifndef WDA_EXCLUDEFROMCAPTURE
#define WDA_EXCLUDEFROMCAPTURE 0x00000011
#endif

namespace {

constexpr wchar_t kWindowClassName[] = L"CYLUNE.DesktopPet.Window";
constexpr UINT kMessageApply = WM_APP + 1;
constexpr UINT kMessageShow = WM_APP + 2;
constexpr UINT kMessageHide = WM_APP + 3;
constexpr UINT kMessageReset = WM_APP + 4;
constexpr UINT kMessageShutdown = WM_APP + 5;
constexpr UINT kMessageFinishDrop = WM_APP + 6;
constexpr uint32_t kCallbackClicked = 1;
constexpr uint32_t kCallbackMoved = 2;
constexpr uint32_t kCallbackDisplayChanged = 6;
constexpr uint32_t kCallbackSleep = 9;
constexpr uint32_t kCallbackWake = 10;
constexpr double kDragThreshold = 4.0;
constexpr DWORD kShutdownTimeoutMilliseconds = 2000;

struct FinishDropCommand {
  uint64_t generation;
  uint32_t result;
};

uint64_t DisplayId(const wchar_t *device) {
  uint64_t hash = 1469598103934665603ULL;
  if (device != nullptr) {
    while (*device != L'\0') {
      hash ^= static_cast<uint16_t>(*device++);
      hash *= 1099511628211ULL;
    }
  }
  return hash == 0 ? 1 : hash;
}

double MonitorScale(HWND probe, const RECT &rect) {
  if (probe == nullptr ||
      !SetWindowPos(probe, nullptr, rect.left, rect.top, 1, 1,
                    SWP_NOACTIVATE | SWP_NOZORDER)) {
    return 1.0;
  }
  const UINT dpi = GetDpiForWindow(probe);
  return dpi == 0 ? 1.0 : static_cast<double>(dpi) / 96.0;
}

bool ApplyInputRegion(HWND window, int physicalSide) {
  const PixelRegionBounds bounds = PetInputRegionBounds(physicalSide);
  HRGN region = CreateEllipticRgn(bounds.left, bounds.top, bounds.right,
                                  bounds.bottom);
  if (region == nullptr) return false;
  if (SetWindowRgn(window, region, TRUE) != 0) return true;
  DeleteObject(region);
  return false;
}

bool ConfigureWindowProtection(HWND window) {
  return SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE) != 0;
}

struct MonitorSnapshot {
  HMONITOR monitor;
  RECT physical;
  DisplayInfo logical;
  bool primary;
};

struct MonitorCollectionContext {
  std::vector<MonitorSnapshot> *monitors;
  HWND dpiProbe;
};

BOOL CALLBACK CollectMonitor(HMONITOR monitor, HDC, LPRECT,
                             LPARAM contextValue) {
  auto *context =
      reinterpret_cast<MonitorCollectionContext *>(contextValue);
  MONITORINFOEXW info{};
  info.cbSize = sizeof(info);
  if (!GetMonitorInfoW(monitor, &info)) return TRUE;
  const RECT rect = info.rcMonitor;
  const double scale = MonitorScale(context->dpiProbe, rect);
  context->monitors->push_back(
      {monitor,
       rect,
       {DisplayId(info.szDevice), static_cast<double>(rect.left),
        static_cast<double>(rect.top),
        static_cast<double>(rect.right - rect.left) / scale,
        static_cast<double>(rect.bottom - rect.top) / scale, scale,
        static_cast<double>(rect.left), static_cast<double>(rect.top)},
       (info.dwFlags & MONITORINFOF_PRIMARY) != 0});
  return TRUE;
}

std::vector<MonitorSnapshot> MonitorSnapshots(HWND dpiProbe) {
  std::vector<MonitorSnapshot> monitors;
  MonitorCollectionContext context{&monitors, dpiProbe};
  EnumDisplayMonitors(nullptr, nullptr, CollectMonitor,
                      reinterpret_cast<LPARAM>(&context));
  std::stable_sort(monitors.begin(), monitors.end(),
                   [](const MonitorSnapshot &lhs, const MonitorSnapshot &rhs) {
                     return lhs.primary && !rhs.primary;
                   });
  return monitors;
}

const MonitorSnapshot *FindMonitor(const std::vector<MonitorSnapshot> &monitors,
                                   uint64_t displayId) {
  const auto found =
      std::find_if(monitors.begin(), monitors.end(),
                   [displayId](const MonitorSnapshot &monitor) {
                     return monitor.logical.id == displayId;
                   });
  return found == monitors.end() ? nullptr : &*found;
}

std::vector<DisplayInfo> DisplayInfos(
    const std::vector<MonitorSnapshot> &monitors) {
  std::vector<DisplayInfo> displays;
  displays.reserve(monitors.size());
  for (const MonitorSnapshot &monitor : monitors) {
    displays.push_back(monitor.logical);
  }
  return displays;
}

bool ValidConfig(PetConfig config) {
  const bool validFps =
      config.fps == 0 || config.fps == 30 || config.fps == 60;
  return config.abi_version == 1 && config.mode <= 1 &&
         config.effective_mode <= 1 && config.has_position <= 1 &&
         std::isfinite(config.size) && config.size >= 120.0 &&
         config.size <= 900.0 &&
         (!config.has_position ||
          (std::isfinite(config.x) && std::isfinite(config.y))) &&
         validFps && config.visible <= 1 && config.reduce_motion <= 1 &&
         config.request_permission <= 1 && config.visual_style <= 1;
}

int Rounded(double value) {
  if (!std::isfinite(value)) return 0;
  const double bounded = std::clamp(
      value, static_cast<double>(std::numeric_limits<int>::min()),
      static_cast<double>(std::numeric_limits<int>::max()));
  return static_cast<int>(std::lround(bounded));
}

} // namespace

struct PetWindow::Impl {
  Impl(PetCallback callbackValue, const char *hlslSource)
      : callback(callbackValue),
        shaderSource(hlslSource == nullptr ? "" : hlslSource) {}

  ~Impl() {
    if (stopEvent != nullptr) (void)CloseHandle(stopEvent);
  }

  PetCallback callback;
  std::string shaderSource;
  std::atomic<HANDLE> ownerHandle{nullptr};
  HANDLE stopEvent = nullptr;
  std::mutex readyMutex;
  std::condition_variable readyCondition;
  bool ready = false;
  bool created = false;
  std::atomic<HWND> hwnd{nullptr};
  std::atomic<DWORD> ownerThreadId{0};
  std::atomic<bool> stopping{false};
  std::mutex shutdownMutex;
  std::mutex commandMutex;
  std::unique_ptr<PetConfig> pendingApply;
  std::vector<FinishDropCommand> pendingDropFinishes;

  std::vector<MonitorSnapshot> monitors;
  Placement placement{0, 0.0, 0.0, 300.0};
  bool visible = false;
  bool sleeping = false;
  bool dragging = false;
  bool dragMoved = false;
  bool windowDestroyed = false;
  bool inputRegionValid = false;
  PetDropTarget *dropTarget = nullptr;
  bool dropTargetRegistered = false;
  PetDropVisualState dropVisualState = PetDropVisualState::Idle;
  uint32_t targetFps = 30;
  std::unique_ptr<BlackHoleRenderer> renderer;
  RenderState renderState;
  std::atomic<uint32_t> nativeRendererState{PET_RENDERER_UNAVAILABLE};
  std::chrono::steady_clock::time_point lastFrameTime{};
  std::chrono::steady_clock::time_point nextFrameTime{};
  HWND dpiProbe = nullptr;
  POINT dragCursorOrigin{};
  RECT dragWindowOrigin{};

  static unsigned __stdcall ThreadEntry(void *context) {
    std::unique_ptr<std::shared_ptr<Impl>> keepAlive(
        static_cast<std::shared_ptr<Impl> *>(context));
    (*keepAlive)->threadMain();
    return 0;
  }

  static void DropVisualChanged(void *context, PetDropVisualState state) {
    auto *self = static_cast<Impl *>(context);
    if (self == nullptr || self->stopping.load(std::memory_order_acquire)) {
      return;
    }
    self->dropVisualState = state;
    self->renderState.setVisualState(self->renderVisualState(state));
    self->targetFps = self->renderState.targetFps(60);
    self->resetRenderClock();
  }

  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wParam,
                                     LPARAM lParam) {
    Impl *self = reinterpret_cast<Impl *>(
        GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
      const auto *create = reinterpret_cast<CREATESTRUCTW *>(lParam);
      self = static_cast<Impl *>(create->lpCreateParams);
      SetLastError(ERROR_SUCCESS);
      const LONG_PTR previous = SetWindowLongPtrW(
          window, GWLP_USERDATA, reinterpret_cast<LONG_PTR>(self));
      if (previous == 0 && GetLastError() != ERROR_SUCCESS) return FALSE;
    }
    return self == nullptr ? DefWindowProcW(window, message, wParam, lParam)
                           : self->handleMessage(window, message, wParam,
                                                 lParam);
  }

  void signalReady(bool success) {
    {
      std::lock_guard<std::mutex> lock(readyMutex);
      created = success;
      ready = true;
    }
    readyCondition.notify_one();
  }

  static RenderVisualState renderVisualState(PetDropVisualState state) {
    switch (state) {
      case PetDropVisualState::Hover:
        return RenderVisualState::Hover;
      case PetDropVisualState::WaitingForAck:
        return RenderVisualState::WaitingForAck;
      case PetDropVisualState::SwallowAndSuccessJet:
        return RenderVisualState::SwallowAndSuccessJet;
      case PetDropVisualState::SwallowAndEject:
        return RenderVisualState::SwallowAndEject;
      case PetDropVisualState::Idle:
      default:
        return RenderVisualState::Idle;
    }
  }

  void resetRenderClock() {
    const auto now = std::chrono::steady_clock::now();
    lastFrameTime = now;
    nextFrameTime = now;
  }

  void updateRendererAvailability() {
    nativeRendererState.store(
        renderer != nullptr && renderer->available() ? PET_RENDERER_READY
                                                     : PET_RENDERER_UNAVAILABLE,
        std::memory_order_release);
  }

  void initializeRenderer(HWND window) {
    renderer = BlackHoleRenderer::create(window, shaderSource.c_str());
    if (renderer != nullptr && renderer->available()) {
      RECT client{};
      if (!GetClientRect(window, &client) || client.right <= client.left ||
          client.bottom <= client.top ||
          !renderer->resize(static_cast<uint32_t>(client.right - client.left),
                            static_cast<uint32_t>(client.bottom - client.top))) {
        renderer->shutdown();
      }
    }
    if (renderer != nullptr) renderer->setVisible(false);
    updateRendererAvailability();
    resetRenderClock();
  }

  void shutdownRenderer() {
    nativeRendererState.store(PET_RENDERER_UNAVAILABLE,
                              std::memory_order_release);
    if (renderer != nullptr) {
      renderer->setVisible(false);
      renderer->shutdown();
      renderer.reset();
    }
  }

  void resizeRenderer(uint32_t width, uint32_t height) {
    if (renderer == nullptr || !renderer->available()) return;
    if (!renderer->resize(width, height)) {
      updateRendererAvailability();
      if (HWND window = hwnd.load(std::memory_order_relaxed)) {
        (void)ShowWindow(window, SW_HIDE);
      }
    }
  }

  DWORD renderWaitMilliseconds(
      std::chrono::steady_clock::time_point now) const {
    if (nativeRendererState.load(std::memory_order_acquire) !=
            PET_RENDERER_READY ||
        renderState.targetFps(60) == 0) {
      return INFINITE;
    }
    if (nextFrameTime <= now) return 0;
    const auto remaining = nextFrameTime - now;
    const auto milliseconds =
        std::chrono::duration_cast<std::chrono::milliseconds>(remaining)
            .count();
    if (milliseconds <= 0) return 1;
    return milliseconds > static_cast<int64_t>(MAXDWORD)
               ? MAXDWORD
               : static_cast<DWORD>(milliseconds);
  }

  void renderFrameIfDue(std::chrono::steady_clock::time_point now) {
    if (nativeRendererState.load(std::memory_order_acquire) !=
            PET_RENDERER_READY ||
        renderer == nullptr || renderState.targetFps(60) == 0) {
      lastFrameTime = now;
      nextFrameTime = now;
      return;
    }
    if (nextFrameTime > now) return;
    const double elapsed =
        std::chrono::duration<double>(now - lastFrameTime).count();
    lastFrameTime = now;
    renderState.advance(elapsed);
    if (renderState.visualState() == RenderVisualState::Idle &&
        (dropVisualState == PetDropVisualState::SwallowAndSuccessJet ||
         dropVisualState == PetDropVisualState::SwallowAndEject)) {
      dropVisualState = PetDropVisualState::Idle;
    }
    const RenderFrameState &state = renderState.frame();
    RendererFrame frame{};
    frame.animationTime = state.animationTime;
    const MonitorSnapshot *monitor = FindMonitor(monitors, placement.displayId);
    const double scale = monitor == nullptr ? 1.0 : monitor->logical.scale;
    frame.visualDiameterPixels = renderState.visualDiameter() * scale;
    frame.rotationRate = static_cast<float>(state.rotationRate);
    frame.shaderStyle = state.shaderStyle;
    frame.ingestProgress = static_cast<float>(state.ingestProgress);
    frame.ejectProgress = static_cast<float>(state.ejectProgress);
    frame.pullGain = static_cast<float>(state.pullGain);
    frame.successJetProgress =
        static_cast<float>(state.successJetProgress);
    frame.pendingCount = state.pendingCount;
    if (!renderer->render(frame)) {
      updateRendererAvailability();
      if (nativeRendererState.load(std::memory_order_acquire) !=
          PET_RENDERER_READY) {
        if (HWND window = hwnd.load(std::memory_order_relaxed)) {
          (void)ShowWindow(window, SW_HIDE);
        }
        return;
      }
    }
    targetFps = renderState.targetFps(60);
    nextFrameTime = NextRenderDeadline(std::chrono::steady_clock::now(),
                                       targetFps);
  }

  bool detachWindowUserData(HWND window) {
    SetLastError(ERROR_SUCCESS);
    const LONG_PTR previous = SetWindowLongPtrW(window, GWLP_USERDATA, 0);
    return previous != 0 || GetLastError() == ERROR_SUCCESS;
  }

  bool attemptWindowDestroy(HWND window, uint32_t &attempts,
                            DWORD &retryDelay) {
    ++attempts;
    const bool destroyed = DestroyWindow(window) != 0;
    const OwnerDestroyDecision decision =
        NextOwnerDestroyDecision(attempts, destroyed, windowDestroyed);
    if (decision.action == OwnerDestroyAction::Complete) return true;
    if (decision.action == OwnerDestroyAction::RetryAfterDelay) {
      retryDelay = decision.delayMilliseconds;
      return false;
    }
    if (detachWindowUserData(window)) {
      hwnd.store(nullptr, std::memory_order_release);
      return true;
    }
    retryDelay = 200;
    return false;
  }

  void runOwnerLoop(HWND window, bool initialized) {
    bool stopRequested =
        !initialized || stopping.load(std::memory_order_acquire) ||
        WaitForSingleObject(stopEvent, 0) == WAIT_OBJECT_0;
    bool stopUiApplied = false;
    uint32_t destroyAttempts = 0;
    ULONGLONG nextDestroyAttempt = 0;

    while (!windowDestroyed) {
      const ULONGLONG now = GetTickCount64();
      if (stopRequested && now >= nextDestroyAttempt) {
        if (!stopUiApplied) {
          revokeDropTarget(window);
          hideWindow();
          shutdownRenderer();
          stopUiApplied = true;
        }
        DWORD retryDelay = 0;
        if (attemptWindowDestroy(window, destroyAttempts, retryDelay)) break;
        nextDestroyAttempt = GetTickCount64() + retryDelay;
      }

      DWORD timeout = INFINITE;
      if (stopRequested) {
        const ULONGLONG current = GetTickCount64();
        const ULONGLONG remaining = nextDestroyAttempt > current
                                        ? nextDestroyAttempt - current
                                        : 0;
        timeout = remaining > MAXDWORD ? MAXDWORD
                                       : static_cast<DWORD>(remaining);
      } else {
        const auto frameNow = std::chrono::steady_clock::now();
        renderFrameIfDue(frameNow);
        timeout = renderWaitMilliseconds(frameNow);
      }
      const DWORD handleCount = stopRequested ? 0 : 1;
      const HANDLE *handles = stopRequested ? nullptr : &stopEvent;
      const DWORD waitResult = MsgWaitForMultipleObjectsEx(
          handleCount, handles, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
      if (!stopRequested && waitResult == WAIT_OBJECT_0) {
        stopping.store(true, std::memory_order_release);
        stopRequested = true;
        nextDestroyAttempt = 0;
        continue;
      }
      if (waitResult == WAIT_OBJECT_0 + handleCount) {
        MSG message{};
        while (PeekMessageW(&message, nullptr, 0, 0, PM_REMOVE)) {
          if (message.message == WM_QUIT ||
              (message.hwnd == nullptr &&
               message.message == kMessageShutdown)) {
            stopping.store(true, std::memory_order_release);
            stopRequested = true;
            nextDestroyAttempt = 0;
          } else {
            (void)TranslateMessage(&message);
            (void)DispatchMessageW(&message);
          }
        }
        continue;
      }
      if (waitResult == WAIT_TIMEOUT) continue;
      stopping.store(true, std::memory_order_release);
      stopRequested = true;
      if (timeout != 0) Sleep(std::min<DWORD>(timeout, 10));
    }
  }

  void threadMain() {
    ownerThreadId.store(GetCurrentThreadId(), std::memory_order_release);
    const DPI_AWARENESS_CONTEXT previousDpiContext =
        SetThreadDpiAwarenessContext(
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    if (previousDpiContext == nullptr) {
      signalReady(false);
      return;
    }
    const HRESULT apartment = OleInitialize(nullptr);
    const bool apartmentInitialized = SUCCEEDED(apartment);
    if (!apartmentInitialized) {
      signalReady(false);
      return;
    }

    WNDCLASSEXW windowClass{};
    windowClass.cbSize = sizeof(windowClass);
    windowClass.lpfnWndProc = WindowProc;
    windowClass.hInstance = GetModuleHandleW(nullptr);
    windowClass.hCursor = LoadCursorW(nullptr, IDC_HAND);
    windowClass.lpszClassName = kWindowClassName;
    const ATOM atom = RegisterClassExW(&windowClass);
    if (atom == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS) {
      signalReady(false);
      if (apartmentInitialized) OleUninitialize();
      return;
    }
    if (WaitForSingleObject(stopEvent, 0) == WAIT_OBJECT_0) {
      signalReady(false);
      ownerThreadId.store(0, std::memory_order_release);
      OleUninitialize();
      return;
    }

    const DWORD style = WS_POPUP;
    const DWORD exStyle =
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP;
    HWND window = CreateWindowExW(
        exStyle, kWindowClassName, L"CYLUNE Desktop Pet", style, 0, 0, 300,
        300, nullptr, nullptr, GetModuleHandleW(nullptr), this);
    if (window == nullptr) {
      signalReady(false);
      if (apartmentInitialized) OleUninitialize();
      return;
    }
    hwnd.store(window, std::memory_order_release);
    dpiProbe = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE, L"STATIC", L"", WS_POPUP, 0, 0,
        1, 1, nullptr, nullptr, GetModuleHandleW(nullptr), nullptr);
    monitors = MonitorSnapshots(dpiProbe);
    const bool stopRequested =
        WaitForSingleObject(stopEvent, 0) == WAIT_OBJECT_0;
    const bool windowInitialized =
        !stopRequested && dpiProbe != nullptr &&
        ConfigureWindowProtection(window) && !monitors.empty() &&
        resetPosition();
    if (windowInitialized) initializeRenderer(window);
    const bool initialized = windowInitialized && registerDropTarget(window);
    signalReady(initialized);
    runOwnerLoop(window, initialized);
    revokeDropTarget(window);
    shutdownRenderer();
    if (dpiProbe != nullptr) {
      (void)DestroyWindow(dpiProbe);
      dpiProbe = nullptr;
    }
    hwnd.store(nullptr, std::memory_order_release);
    ownerThreadId.store(0, std::memory_order_release);
    OleUninitialize();
  }

  bool post(UINT message, LPARAM lParam = 0) {
    if (stopping.load(std::memory_order_acquire)) return false;
    HWND window = hwnd.load(std::memory_order_acquire);
    return window != nullptr && PostMessageW(window, message, 0, lParam) != 0;
  }

  bool postApply(PetConfig config) {
    std::unique_ptr<PetConfig> command(new (std::nothrow) PetConfig(config));
    if (command == nullptr) return false;
    std::lock_guard<std::mutex> lock(commandMutex);
    if (stopping.load(std::memory_order_acquire)) return false;
    pendingApply = std::move(command);
    HWND window = hwnd.load(std::memory_order_acquire);
    if (window != nullptr && PostMessageW(window, kMessageApply, 0, 0)) {
      return true;
    }
    pendingApply.reset();
    return false;
  }

  bool signalStop() {
    stopping.store(true, std::memory_order_release);
    return stopEvent != nullptr && SetEvent(stopEvent) != 0;
  }

  bool registerDropTarget(HWND window) {
    if (dropTarget != nullptr || dropTargetRegistered) return false;
    PetDropTarget *candidate = PetDropTarget::create(
        window, callback, &Impl::DropVisualChanged, this, &stopping);
    if (candidate == nullptr) return false;
    const HRESULT registered = RegisterDragDrop(window, candidate);
    if (FAILED(registered)) {
      candidate->deactivate();
      (void)candidate->Release();
      return false;
    }
    dropTarget = candidate;
    dropTargetRegistered = true;
    return true;
  }

  void revokeDropTarget(HWND window) {
    PetDropTarget *target = dropTarget;
    dropTarget = nullptr;
    if (target == nullptr) {
      dropTargetRegistered = false;
      return;
    }
    target->deactivate();
    if (dropTargetRegistered && window != nullptr) {
      (void)RevokeDragDrop(window);
    }
    dropTargetRegistered = false;
    (void)target->Release();
    dropVisualState = PetDropVisualState::Idle;
    renderState.setVisualState(RenderVisualState::Idle);
    targetFps = renderState.targetFps(60);
  }

  bool postFinishDrop(uint64_t generation, uint32_t result) {
    if (generation == 0 ||
        (result != PET_DROP_ACCEPTED && result != PET_DROP_REJECTED)) {
      return false;
    }
    std::lock_guard<std::mutex> lock(commandMutex);
    if (stopping.load(std::memory_order_acquire)) return false;
    try {
      pendingDropFinishes.push_back({generation, result});
    } catch (...) {
      return false;
    }
    HWND window = hwnd.load(std::memory_order_acquire);
    if (window != nullptr &&
        PostMessageW(window, kMessageFinishDrop, 0, 0) != 0) {
      return true;
    }
    pendingDropFinishes.pop_back();
    return false;
  }

  void emit(uint32_t kind, double x = 0.0, double y = 0.0,
            uint64_t displayId = 0) const noexcept {
    InvokePetCallbackNoThrow(callback, kind, nullptr, x, y, displayId);
  }

  Placement clamp(LogicalPoint origin, double size,
                  uint64_t preferredDisplay = 0) const {
    return ClampPetOrigin(origin, size, DisplayInfos(monitors),
                          preferredDisplay);
  }

  void invalidateInputRegion() {
    inputRegionValid = false;
    dragging = false;
    dragMoved = false;
    renderState.setVisible(false);
    resetRenderClock();
    if (renderer != nullptr) renderer->setVisible(false);
    (void)ReleaseCapture();
    if (HWND window = hwnd.load(std::memory_order_relaxed)) {
      (void)ShowWindow(window, SW_HIDE);
    }
  }

  bool positionWindow() {
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr) return false;
    const MonitorSnapshot *monitor = FindMonitor(monitors, placement.displayId);
    if (monitor == nullptr) return false;
    const LogicalPoint physical =
        LogicalToPhysical({placement.x, placement.y}, monitor->logical);
    const int side =
        std::max(1, Rounded(placement.size * monitor->logical.scale));
    if (!SetWindowPos(window, HWND_TOPMOST, Rounded(physical.x),
                      Rounded(physical.y), side, side,
                      SWP_NOACTIVATE | SWP_NOOWNERZORDER)) {
      invalidateInputRegion();
      return false;
    }
    resizeRenderer(static_cast<uint32_t>(side), static_cast<uint32_t>(side));
    inputRegionValid = ApplyInputRegion(window, side);
    if (!inputRegionValid) invalidateInputRegion();
    return inputRegionValid;
  }

  bool resetPosition() {
    const double size = placement.size;
    placement = clamp(
        {std::numeric_limits<double>::quiet_NaN(),
         std::numeric_limits<double>::quiet_NaN()},
        size);
    return positionWindow();
  }

  void applyConfig(PetConfig config) {
    monitors = MonitorSnapshots(dpiProbe);
    visible = config.visible != 0;
    if (config.has_position != 0) {
      placement = clamp({config.x, config.y}, config.size, config.display_id);
    } else if (placement.displayId != 0) {
      placement = clamp({placement.x, placement.y}, config.size,
                        placement.displayId);
    } else {
      placement = clamp(
          {std::numeric_limits<double>::quiet_NaN(),
           std::numeric_limits<double>::quiet_NaN()},
          config.size);
    }
    if (!positionWindow()) {
      invalidateInputRegion();
      return;
    }
    RenderConfig rendererConfig{};
    rendererConfig.fps = config.fps;
    rendererConfig.visible = visible && !sleeping;
    rendererConfig.size = placement.size;
    rendererConfig.pendingCount = config.pending_count;
    rendererConfig.visualStyle = config.visual_style;
    renderState.apply(rendererConfig);
    renderState.setVisualState(renderVisualState(dropVisualState));
    targetFps = renderState.targetFps(60);
    resetRenderClock();
    visible ? showWindow() : hideWindow();
  }

  void showWindow() {
    visible = true;
    renderState.setVisible(!sleeping);
    resetRenderClock();
    if (renderer != nullptr) renderer->setVisible(!sleeping);
    if (!PetWindowMayShow(visible, sleeping, inputRegionValid) ||
        nativeRendererState.load(std::memory_order_acquire) !=
            PET_RENDERER_READY) {
      if (HWND window = hwnd.load(std::memory_order_relaxed)) {
        (void)ShowWindow(window, SW_HIDE);
      }
      return;
    }
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr) return;
    (void)ShowWindow(window, SW_SHOWNOACTIVATE);
    if (!SetWindowPos(window, HWND_TOPMOST, 0, 0, 0, 0,
                      SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE |
                          SWP_NOOWNERZORDER)) {
      (void)ShowWindow(window, SW_HIDE);
    }
  }

  void hideWindow() {
    if (dropTarget != nullptr) dropTarget->cancelHover();
    dragging = false;
    dragMoved = false;
    visible = false;
    renderState.setVisible(false);
    resetRenderClock();
    if (renderer != nullptr) renderer->setVisible(false);
    if (HWND window = hwnd.load(std::memory_order_relaxed)) {
      (void)ReleaseCapture();
      (void)ShowWindow(window, SW_HIDE);
    }
  }

  void recoverForDisplays(bool emitChange) {
    RECT rect{};
    HWND window = hwnd.load(std::memory_order_relaxed);
    const bool haveRect = window != nullptr && GetWindowRect(window, &rect);
    const uint64_t priorDisplay = placement.displayId;
    monitors = MonitorSnapshots(dpiProbe);
    LogicalPoint origin{placement.x, placement.y};
    if (haveRect) {
      HMONITOR active = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
      const auto found = std::find_if(
          monitors.begin(), monitors.end(),
          [active](const MonitorSnapshot &monitor) {
            return monitor.monitor == active;
          });
      if (found != monitors.end()) {
        origin = PhysicalToLogical(
            {static_cast<double>(rect.left), static_cast<double>(rect.top)},
            found->logical);
      }
    }
    uint64_t targetDisplay = priorDisplay;
    if (haveRect) {
      const HMONITOR active = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
      const auto found = std::find_if(
          monitors.begin(), monitors.end(),
          [active](const MonitorSnapshot &monitor) {
            return monitor.monitor == active;
          });
      if (found != monitors.end()) targetDisplay = found->logical.id;
    }
    placement = clamp(origin, placement.size, targetDisplay);
    if (!positionWindow()) {
      invalidateInputRegion();
      return;
    }
    if (visible && !sleeping) showWindow();
    if (emitChange) {
      emit(kCallbackDisplayChanged, placement.x, placement.y,
           placement.displayId);
    }
  }

  void beginDrag() {
    if (dropTarget != nullptr) dropTarget->cancelHover();
    if (!GetCursorPos(&dragCursorOrigin)) return;
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr || !GetWindowRect(window, &dragWindowOrigin)) return;
    (void)SetCapture(window);
    if (GetCapture() != window) return;
    dragging = true;
    dragMoved = false;
  }

  void continueDrag() {
    if (!dragging) return;
    POINT cursor{};
    if (!GetCursorPos(&cursor)) return;
    const int dx = cursor.x - dragCursorOrigin.x;
    const int dy = cursor.y - dragCursorOrigin.y;
    const MonitorSnapshot *current = FindMonitor(monitors, placement.displayId);
    const double scale = current == nullptr ? 1.0 : current->logical.scale;
    if (!dragMoved &&
        std::hypot(static_cast<double>(dx) / scale,
                   static_cast<double>(dy) / scale) < kDragThreshold) {
      return;
    }
    dragMoved = true;
    POINT center{dragWindowOrigin.left + dx +
                     (dragWindowOrigin.right - dragWindowOrigin.left) / 2,
                 dragWindowOrigin.top + dy +
                     (dragWindowOrigin.bottom - dragWindowOrigin.top) / 2};
    const HMONITOR target =
        MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
    const auto found = std::find_if(
        monitors.begin(), monitors.end(),
        [target](const MonitorSnapshot &monitor) {
          return monitor.monitor == target;
        });
    const uint64_t targetId =
        found == monitors.end() ? placement.displayId : found->logical.id;
    if (found == monitors.end() && current == nullptr) return;
    const DisplayInfo &targetDisplay =
        found == monitors.end() ? current->logical : found->logical;
    const LogicalPoint origin = PhysicalToLogical(
        {static_cast<double>(dragWindowOrigin.left + dx),
         static_cast<double>(dragWindowOrigin.top + dy)},
        targetDisplay);
    placement = clamp(origin, placement.size, targetId);
    if (!positionWindow()) invalidateInputRegion();
  }

  void endDrag() {
    if (!dragging) return;
    dragging = false;
    (void)ReleaseCapture();
    if (dragMoved) {
      emit(kCallbackMoved, placement.x, placement.y, placement.displayId);
    } else {
      emit(kCallbackClicked, 0.0, 0.0, placement.displayId);
    }
    dragMoved = false;
  }

  LRESULT handleMessage(HWND window, UINT message, WPARAM wParam,
                        LPARAM lParam) {
    switch (message) {
      case kMessageApply: {
        std::unique_ptr<PetConfig> config;
        {
          std::lock_guard<std::mutex> lock(commandMutex);
          config = std::move(pendingApply);
        }
        if (config) applyConfig(*config);
        return 0;
      }
      case kMessageFinishDrop: {
        std::vector<FinishDropCommand> commands;
        {
          std::lock_guard<std::mutex> lock(commandMutex);
          commands.swap(pendingDropFinishes);
        }
        if (dropTarget != nullptr) {
          for (const FinishDropCommand &command : commands) {
            (void)dropTarget->finish(command.generation, command.result);
          }
        }
        return 0;
      }
      case kMessageShow:
        showWindow();
        return 0;
      case kMessageHide:
        hideWindow();
        return 0;
      case kMessageReset:
        monitors = MonitorSnapshots(dpiProbe);
        if (!resetPosition()) invalidateInputRegion();
        return 0;
      case kMessageShutdown:
        (void)signalStop();
        return 0;
      case WM_NCHITTEST: {
        POINT point{static_cast<short>(LOWORD(lParam)),
                    static_cast<short>(HIWORD(lParam))};
        if (!ScreenToClient(window, &point)) return HTTRANSPARENT;
        const MonitorSnapshot *monitor =
            FindMonitor(monitors, placement.displayId);
        const double scale = monitor == nullptr ? 1.0 : monitor->logical.scale;
        const LogicalPoint local{static_cast<double>(point.x) / scale,
                                 static_cast<double>(point.y) / scale};
        return HitTestPet(local, placement.size) == PetHit::Drag
                   ? HTCLIENT
                   : HTTRANSPARENT;
      }
      case WM_LBUTTONDOWN:
        beginDrag();
        return 0;
      case WM_MOUSEMOVE:
        if ((wParam & MK_LBUTTON) != 0) continueDrag();
        return 0;
      case WM_LBUTTONUP:
        endDrag();
        return 0;
      case WM_CAPTURECHANGED:
        if (dragging && reinterpret_cast<HWND>(lParam) != window) endDrag();
        return 0;
      case WM_DPICHANGED: {
        const RECT *suggested = reinterpret_cast<const RECT *>(lParam);
        if (suggested != nullptr) {
          if (!SetWindowPos(window, nullptr, suggested->left, suggested->top,
                            suggested->right - suggested->left,
                            suggested->bottom - suggested->top,
                          SWP_NOACTIVATE | SWP_NOZORDER)) {
            invalidateInputRegion();
            return 0;
          }
        }
        recoverForDisplays(true);
        return 0;
      }
      case WM_SIZE:
        if (wParam != SIZE_MINIMIZED) {
          resizeRenderer(static_cast<uint32_t>(LOWORD(lParam)),
                         static_cast<uint32_t>(HIWORD(lParam)));
        }
        return 0;
      case WM_DISPLAYCHANGE:
        recoverForDisplays(true);
        return 0;
      case WM_POWERBROADCAST:
        if (wParam == PBT_APMSUSPEND && !sleeping) {
          sleeping = true;
          if (dropTarget != nullptr) dropTarget->cancelHover();
          dragging = false;
          dragMoved = false;
          renderState.setVisible(false);
          resetRenderClock();
          if (renderer != nullptr) renderer->setVisible(false);
          (void)ReleaseCapture();
          (void)ShowWindow(window, SW_HIDE);
          emit(kCallbackSleep, 0.0, 0.0, placement.displayId);
        } else if ((wParam == PBT_APMRESUMEAUTOMATIC ||
                    wParam == PBT_APMRESUMESUSPEND) &&
                   sleeping) {
          sleeping = false;
          recoverForDisplays(true);
          if (visible) showWindow();
          emit(kCallbackWake, 0.0, 0.0, placement.displayId);
        }
        return TRUE;
      case WM_ERASEBKGND:
        return 1;
      case WM_PAINT: {
        PAINTSTRUCT paint{};
        const HDC device = BeginPaint(window, &paint);
        if (device != nullptr) (void)EndPaint(window, &paint);
        return 0;
      }
      case WM_CLOSE:
        (void)signalStop();
        return 0;
      case WM_NCDESTROY: {
        revokeDropTarget(window);
        shutdownRenderer();
        windowDestroyed = true;
        hwnd.store(nullptr, std::memory_order_release);
        SetLastError(ERROR_SUCCESS);
        const LONG_PTR previous = SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        (void)previous;
        (void)GetLastError();
        PostQuitMessage(0);
        return DefWindowProcW(window, message, wParam, lParam);
      }
      default:
        return DefWindowProcW(window, message, wParam, lParam);
    }
  }
};

PetWindow::PetWindow(PetCallback callback, const char *hlslSource)
    : impl_(std::make_shared<Impl>(callback, hlslSource)) {}

PetWindow::~PetWindow() { (void)shutdown(); }

std::unique_ptr<PetWindow> PetWindow::create(PetCallback callback,
                                             const char *hlslSource) {
  if (callback == nullptr || hlslSource == nullptr) return nullptr;
  try {
    std::unique_ptr<PetWindow> window(new PetWindow(callback, hlslSource));
    if (!window->start()) return nullptr;
    return window;
  } catch (...) {
    return nullptr;
  }
}

bool PetWindow::start() {
  const auto deadline = std::chrono::steady_clock::now() +
                        std::chrono::milliseconds(
                            kShutdownTimeoutMilliseconds);
  impl_->stopEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (impl_->stopEvent == nullptr) return false;
  auto *context =
      new (std::nothrow) std::shared_ptr<Impl>(impl_);
  if (context == nullptr) return false;
  unsigned threadId = 0;
  const uintptr_t rawHandle =
      _beginthreadex(nullptr, 0, &Impl::ThreadEntry, context, 0, &threadId);
  if (rawHandle == 0) {
    delete context;
    return false;
  }
  const HANDLE handle = reinterpret_cast<HANDLE>(rawHandle);
  impl_->ownerHandle.store(handle, std::memory_order_release);
  std::unique_lock<std::mutex> lock(impl_->readyMutex);
  const bool waitSatisfied = impl_->readyCondition.wait_until(
      lock, deadline, [this] { return impl_->ready; });
  const bool ready = impl_->ready;
  const bool created = impl_->created;
  const OwnerReadinessAction readiness =
      ResolveOwnerReadiness(waitSatisfied, ready, created);
  lock.unlock();
  if (readiness != OwnerReadinessAction::Created) {
    (void)impl_->signalStop();
    const auto now = std::chrono::steady_clock::now();
    const auto remaining = now < deadline
                               ? std::chrono::duration_cast<
                                     std::chrono::milliseconds>(deadline - now)
                               : std::chrono::milliseconds(0);
    if (remaining.count() > 0) {
      (void)WaitForSingleObject(handle,
                                static_cast<DWORD>(remaining.count()));
    }
    HANDLE expected = handle;
    if (impl_->ownerHandle.compare_exchange_strong(expected, nullptr,
                                                   std::memory_order_acq_rel)) {
      (void)CloseHandle(handle);
    }
  }
  return readiness == OwnerReadinessAction::Created;
}

bool PetWindow::apply(PetConfig config) {
  if (!ValidConfig(config)) return false;
  return impl_->postApply(config);
}

void PetWindow::show() { impl_->post(kMessageShow); }

void PetWindow::hide() { impl_->post(kMessageHide); }

void PetWindow::reset() { impl_->post(kMessageReset); }

void PetWindow::finishDrop(uint64_t generation, uint32_t result) {
  (void)impl_->postFinishDrop(generation, result);
}

uint32_t PetWindow::rendererState() const {
  return impl_ == nullptr
             ? PET_RENDERER_UNAVAILABLE
             : impl_->nativeRendererState.load(std::memory_order_acquire);
}

uint32_t PetWindow::shutdown() {
  if (impl_ == nullptr) return PET_SHUTDOWN_COMPLETE;
  const std::shared_ptr<Impl> impl = impl_;
  std::lock_guard<std::mutex> shutdownLock(impl->shutdownMutex);
  bool stopSignaled = false;
  {
    std::lock_guard<std::mutex> lock(impl->commandMutex);
    const bool wasStopping = impl->stopping.exchange(true);
    stopSignaled = impl->stopEvent != nullptr && SetEvent(impl->stopEvent) != 0;
    if (!wasStopping) {
      HWND window = impl->hwnd.load(std::memory_order_acquire);
      bool posted =
          window == nullptr || PostMessageW(window, kMessageShutdown, 0, 0);
      if (!posted) {
        const DWORD threadId =
            impl->ownerThreadId.load(std::memory_order_acquire);
        (void)(threadId == 0 ||
               PostThreadMessageW(threadId, kMessageShutdown, 0, 0));
      }
    }
  }
  const HANDLE handle =
      impl->ownerHandle.exchange(nullptr, std::memory_order_acq_rel);
  if (handle == nullptr) {
    return stopSignaled ? PET_SHUTDOWN_COMPLETE : PET_SHUTDOWN_STOP_FAILED;
  }
  const DWORD ownerThreadId =
      impl->ownerThreadId.load(std::memory_order_acquire);
  DWORD waitResult = WAIT_TIMEOUT;
  if (ownerThreadId == 0 || ownerThreadId != GetCurrentThreadId()) {
    waitResult =
        WaitForSingleObject(handle, kShutdownTimeoutMilliseconds);
  }
  const bool closed = CloseHandle(handle) != 0;
  if (!closed || waitResult == WAIT_FAILED) return PET_SHUTDOWN_STOP_FAILED;
  if (waitResult != WAIT_OBJECT_0) return PET_SHUTDOWN_STOP_TIMED_OUT;
  return stopSignaled ? PET_SHUTDOWN_COMPLETE : PET_SHUTDOWN_STOP_FAILED;
}
