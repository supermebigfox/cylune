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

#include "window_state.h"

#include <windows.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <condition_variable>
#include <cstdint>
#include <limits>
#include <mutex>
#include <new>
#include <thread>
#include <utility>
#include <vector>

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
constexpr uint32_t kCallbackClicked = 1;
constexpr uint32_t kCallbackMoved = 2;
constexpr uint32_t kCallbackDisplayChanged = 6;
constexpr uint32_t kCallbackSleep = 9;
constexpr uint32_t kCallbackWake = 10;
constexpr double kDragThreshold = 4.0;
constexpr DWORD kShutdownTimeoutMilliseconds = 2000;

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

double MonitorScale(const RECT &rect) {
  HWND probe = CreateWindowExW(WS_EX_TOOLWINDOW, L"STATIC", L"", WS_POPUP,
                               rect.left, rect.top, 1, 1, nullptr, nullptr,
                               GetModuleHandleW(nullptr), nullptr);
  if (probe == nullptr) return 1.0;
  const UINT dpi = GetDpiForWindow(probe);
  DestroyWindow(probe);
  return dpi == 0 ? 1.0 : static_cast<double>(dpi) / 96.0;
}

struct MonitorSnapshot {
  HMONITOR monitor;
  RECT physical;
  DisplayInfo logical;
  bool primary;
};

BOOL CALLBACK CollectMonitor(HMONITOR monitor, HDC, LPRECT,
                             LPARAM contextValue) {
  auto *monitors =
      reinterpret_cast<std::vector<MonitorSnapshot> *>(contextValue);
  MONITORINFOEXW info{};
  info.cbSize = sizeof(info);
  if (!GetMonitorInfoW(monitor, &info)) return TRUE;
  const RECT rect = info.rcMonitor;
  const double scale = MonitorScale(rect);
  monitors->push_back(
      {monitor,
       rect,
       {DisplayId(info.szDevice), static_cast<double>(rect.left) / scale,
        static_cast<double>(rect.top) / scale,
        static_cast<double>(rect.right - rect.left) / scale,
        static_cast<double>(rect.bottom - rect.top) / scale, scale},
       (info.dwFlags & MONITORINFOF_PRIMARY) != 0});
  return TRUE;
}

std::vector<MonitorSnapshot> MonitorSnapshots() {
  std::vector<MonitorSnapshot> monitors;
  EnumDisplayMonitors(nullptr, nullptr, CollectMonitor,
                      reinterpret_cast<LPARAM>(&monitors));
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
  explicit Impl(PetCallback callbackValue) : callback(callbackValue) {}

  PetCallback callback;
  std::thread owner;
  std::mutex readyMutex;
  std::condition_variable readyCondition;
  bool ready = false;
  bool created = false;
  std::atomic<HWND> hwnd{nullptr};
  std::atomic<DWORD> ownerThreadId{0};
  std::atomic<bool> stopping{false};
  std::mutex commandMutex;
  std::unique_ptr<PetConfig> pendingApply;

  std::vector<MonitorSnapshot> monitors;
  Placement placement{0, 0.0, 0.0, 300.0};
  bool visible = false;
  bool sleeping = false;
  bool dragging = false;
  bool dragMoved = false;
  POINT dragCursorOrigin{};
  RECT dragWindowOrigin{};

  static LRESULT CALLBACK WindowProc(HWND window, UINT message, WPARAM wParam,
                                     LPARAM lParam) {
    Impl *self = reinterpret_cast<Impl *>(
        GetWindowLongPtrW(window, GWLP_USERDATA));
    if (message == WM_NCCREATE) {
      const auto *create = reinterpret_cast<CREATESTRUCTW *>(lParam);
      self = static_cast<Impl *>(create->lpCreateParams);
      SetWindowLongPtrW(window, GWLP_USERDATA,
                        reinterpret_cast<LONG_PTR>(self));
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

  void threadMain() {
    ownerThreadId.store(GetCurrentThreadId(), std::memory_order_release);
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    const DPI_AWARENESS_CONTEXT previousDpiContext =
        SetThreadDpiAwarenessContext(
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    if (previousDpiContext == nullptr) {
      signalReady(false);
      return;
    }
    const HRESULT apartment = CoInitializeEx(nullptr, COINIT_APARTMENTTHREADED);
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
      if (apartmentInitialized) CoUninitialize();
      return;
    }

    const DWORD style = WS_POPUP;
    const DWORD exStyle = WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
    HWND window = CreateWindowExW(
        exStyle, kWindowClassName, L"CYLUNE Desktop Pet", style, 0, 0, 300,
        300, nullptr, nullptr, GetModuleHandleW(nullptr), this);
    if (window == nullptr) {
      signalReady(false);
      if (apartmentInitialized) CoUninitialize();
      return;
    }
    hwnd.store(window, std::memory_order_release);
    SetWindowLongPtrW(window, GWL_EXSTYLE,
                      GetWindowLongPtrW(window, GWL_EXSTYLE) | WS_EX_LAYERED);
    SetLayeredWindowAttributes(window, 0, 1, LWA_ALPHA);
    SetWindowDisplayAffinity(window, WDA_EXCLUDEFROMCAPTURE);
    monitors = MonitorSnapshots();
    resetPosition();
    signalReady(true);

    MSG message{};
    while (GetMessageW(&message, nullptr, 0, 0) > 0) {
      TranslateMessage(&message);
      DispatchMessageW(&message);
    }
    if (IsWindow(window)) DestroyWindow(window);
    hwnd.store(nullptr, std::memory_order_release);
    ownerThreadId.store(0, std::memory_order_release);
    CoUninitialize();
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

  void emit(uint32_t kind, double x = 0.0, double y = 0.0,
            uint64_t displayId = 0) const {
    if (callback != nullptr) callback(kind, nullptr, x, y, displayId);
  }

  Placement clamp(LogicalPoint origin, double size,
                  uint64_t preferredDisplay = 0) const {
    if (preferredDisplay != 0) {
      if (const MonitorSnapshot *preferred =
              FindMonitor(monitors, preferredDisplay)) {
        return ClampPetOrigin(origin, size, {preferred->logical});
      }
    }
    return ClampPetOrigin(origin, size, DisplayInfos(monitors));
  }

  void positionWindow() {
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr) return;
    const MonitorSnapshot *monitor = FindMonitor(monitors, placement.displayId);
    const double scale = monitor == nullptr ? 1.0 : monitor->logical.scale;
    const LogicalPoint physical =
        LogicalToPhysical({placement.x, placement.y}, scale);
    const int side = std::max(1, Rounded(placement.size * scale));
    SetWindowPos(window, HWND_TOPMOST, Rounded(physical.x), Rounded(physical.y),
                 side, side, SWP_NOACTIVATE | SWP_NOOWNERZORDER);
  }

  void resetPosition() {
    const double size = placement.size;
    placement = clamp(
        {std::numeric_limits<double>::quiet_NaN(),
         std::numeric_limits<double>::quiet_NaN()},
        size);
    positionWindow();
  }

  void applyConfig(PetConfig config) {
    monitors = MonitorSnapshots();
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
    positionWindow();
    config.visible != 0 ? showWindow() : hideWindow();
  }

  void showWindow() {
    visible = true;
    if (sleeping) return;
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr) return;
    ShowWindow(window, SW_SHOWNOACTIVATE);
    SetWindowPos(window, HWND_TOPMOST, 0, 0, 0, 0,
                 SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE |
                     SWP_NOOWNERZORDER);
  }

  void hideWindow() {
    dragging = false;
    dragMoved = false;
    visible = false;
    if (HWND window = hwnd.load(std::memory_order_relaxed)) {
      ReleaseCapture();
      ShowWindow(window, SW_HIDE);
    }
  }

  void recoverForDisplays(bool emitChange) {
    RECT rect{};
    HWND window = hwnd.load(std::memory_order_relaxed);
    const bool haveRect = window != nullptr && GetWindowRect(window, &rect);
    const uint64_t priorDisplay = placement.displayId;
    monitors = MonitorSnapshots();
    LogicalPoint origin{placement.x, placement.y};
    if (haveRect) {
      HMONITOR active = MonitorFromRect(&rect, MONITOR_DEFAULTTONEAREST);
      const auto found = std::find_if(
          monitors.begin(), monitors.end(),
          [active](const MonitorSnapshot &monitor) {
            return monitor.monitor == active;
          });
      const double scale =
          found == monitors.end() ? 1.0 : found->logical.scale;
      origin = PhysicalToLogical(
          {static_cast<double>(rect.left), static_cast<double>(rect.top)},
          scale);
    }
    placement = clamp(origin, placement.size, priorDisplay);
    positionWindow();
    if (emitChange) {
      emit(kCallbackDisplayChanged, placement.x, placement.y,
           placement.displayId);
    }
  }

  void beginDrag() {
    if (!GetCursorPos(&dragCursorOrigin)) return;
    HWND window = hwnd.load(std::memory_order_relaxed);
    if (window == nullptr || !GetWindowRect(window, &dragWindowOrigin)) return;
    dragging = true;
    dragMoved = false;
    SetCapture(window);
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
    const double targetScale =
        found == monitors.end() ? scale : found->logical.scale;
    const uint64_t targetId =
        found == monitors.end() ? placement.displayId : found->logical.id;
    const LogicalPoint origin = PhysicalToLogical(
        {static_cast<double>(dragWindowOrigin.left + dx),
         static_cast<double>(dragWindowOrigin.top + dy)},
        targetScale);
    placement = clamp(origin, placement.size, targetId);
    positionWindow();
  }

  void endDrag() {
    if (!dragging) return;
    dragging = false;
    ReleaseCapture();
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
      case kMessageShow:
        showWindow();
        return 0;
      case kMessageHide:
        hideWindow();
        return 0;
      case kMessageReset:
        monitors = MonitorSnapshots();
        resetPosition();
        return 0;
      case kMessageShutdown:
        hideWindow();
        DestroyWindow(window);
        PostQuitMessage(0);
        return 0;
      case WM_NCHITTEST: {
        POINT point{static_cast<short>(LOWORD(lParam)),
                    static_cast<short>(HIWORD(lParam))};
        ScreenToClient(window, &point);
        const MonitorSnapshot *monitor =
            FindMonitor(monitors, placement.displayId);
        const double scale = monitor == nullptr ? 1.0 : monitor->logical.scale;
        const LogicalPoint local = PhysicalToLogical(
            {static_cast<double>(point.x), static_cast<double>(point.y)},
            scale);
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
          SetWindowPos(window, nullptr, suggested->left, suggested->top,
                       suggested->right - suggested->left,
                       suggested->bottom - suggested->top,
                       SWP_NOACTIVATE | SWP_NOZORDER);
        }
        recoverForDisplays(true);
        return 0;
      }
      case WM_DISPLAYCHANGE:
        recoverForDisplays(true);
        return 0;
      case WM_POWERBROADCAST:
        if (wParam == PBT_APMSUSPEND && !sleeping) {
          sleeping = true;
          dragging = false;
          dragMoved = false;
          ReleaseCapture();
          ShowWindow(window, SW_HIDE);
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
        BeginPaint(window, &paint);
        EndPaint(window, &paint);
        return 0;
      }
      case WM_CLOSE:
        hideWindow();
        DestroyWindow(window);
        PostQuitMessage(0);
        return 0;
      case WM_NCDESTROY:
        SetWindowLongPtrW(window, GWLP_USERDATA, 0);
        return DefWindowProcW(window, message, wParam, lParam);
      default:
        return DefWindowProcW(window, message, wParam, lParam);
    }
  }
};

PetWindow::PetWindow(PetCallback callback)
    : impl_(std::make_unique<Impl>(callback)) {}

PetWindow::~PetWindow() {
  if (shutdown() == PET_SHUTDOWN_STOP_TIMED_OUT) {
    // The owner thread still references Impl. Preserve it rather than risking
    // a use-after-free during an exceptional timed-out shutdown.
    impl_.release();
  }
}

std::unique_ptr<PetWindow> PetWindow::create(PetCallback callback) {
  if (callback == nullptr) return nullptr;
  try {
    std::unique_ptr<PetWindow> window(new PetWindow(callback));
    if (!window->start()) return nullptr;
    return window;
  } catch (...) {
    return nullptr;
  }
}

bool PetWindow::start() {
  try {
    impl_->owner = std::thread([impl = impl_.get()] { impl->threadMain(); });
  } catch (...) {
    return false;
  }
  std::unique_lock<std::mutex> lock(impl_->readyMutex);
  impl_->readyCondition.wait(lock, [this] { return impl_->ready; });
  const bool created = impl_->created;
  lock.unlock();
  if (!created && impl_->owner.joinable()) impl_->owner.join();
  return created;
}

bool PetWindow::apply(PetConfig config) {
  if (!ValidConfig(config)) return false;
  return impl_->postApply(config);
}

void PetWindow::show() { impl_->post(kMessageShow); }

void PetWindow::hide() { impl_->post(kMessageHide); }

void PetWindow::reset() { impl_->post(kMessageReset); }

uint32_t PetWindow::shutdown() {
  if (impl_ == nullptr) return PET_SHUTDOWN_COMPLETE;
  bool posted = true;
  {
    std::lock_guard<std::mutex> lock(impl_->commandMutex);
    const bool wasStopping = impl_->stopping.exchange(true);
    if (!wasStopping) {
      HWND window = impl_->hwnd.load(std::memory_order_acquire);
      posted =
          window == nullptr || PostMessageW(window, kMessageShutdown, 0, 0);
      if (!posted) {
        const DWORD threadId =
            impl_->ownerThreadId.load(std::memory_order_acquire);
        posted = threadId == 0 || PostThreadMessageW(threadId, WM_QUIT, 0, 0);
      }
    }
  }
  if (impl_->owner.joinable()) {
    const DWORD ownerThreadId =
        impl_->ownerThreadId.load(std::memory_order_acquire);
    if (ownerThreadId != 0 && ownerThreadId == GetCurrentThreadId()) {
      return PET_SHUTDOWN_STOP_TIMED_OUT;
    }
    const DWORD waitResult = WaitForSingleObject(
        impl_->owner.native_handle(), kShutdownTimeoutMilliseconds);
    if (waitResult != WAIT_OBJECT_0) return PET_SHUTDOWN_STOP_TIMED_OUT;
    impl_->owner.join();
  }
  return posted ? PET_SHUTDOWN_COMPLETE : PET_SHUTDOWN_STOP_FAILED;
}
