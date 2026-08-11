#ifndef CYLUNE_WINDOWS_DESKTOP_CAPTURE_H
#define CYLUNE_WINDOWS_DESKTOP_CAPTURE_H

#ifndef NOMINMAX
#define NOMINMAX
#endif

#include <windows.h>

#include <d3d11.h>
#include <wrl/client.h>

#include <cstdint>
#include <memory>
#include <mutex>

enum class DesktopCaptureStatus : uint8_t {
  Frame,
  Timeout,
  RecoverableFailure,
  DeviceRemoved,
  Failed,
};

struct DesktopCaptureFrame {
  Microsoft::WRL::ComPtr<ID3D11ShaderResourceView> view;
  std::shared_ptr<std::mutex> contextMutex;
  uint64_t generation = 0;
  LUID adapterLuid{};
  HMONITOR monitor = nullptr;
  uint32_t rotation = 0;
};

// DesktopCapture owns only textures created on the renderer's D3D11 device.
// Callers must discard the returned pointer on every non-Frame result.
class DesktopCapture {
 public:
  static std::unique_ptr<DesktopCapture> create(ID3D11Device *device,
                                                 ID3D11DeviceContext *context);
  ~DesktopCapture();

  DesktopCapture(const DesktopCapture &) = delete;
  DesktopCapture &operator=(const DesktopCapture &) = delete;

  bool start(HMONITOR monitor) noexcept;
  DesktopCaptureStatus acquire(DWORD timeoutMilliseconds,
                               DesktopCaptureFrame *frame) noexcept;
  bool switchDisplay(HMONITOR monitor) noexcept;
  // No wait inside this method can exceed the supplied deadline.  Desktop
  // duplication is acquired on the UI owner thread, so stop is synchronous.
  bool stop(DWORD deadlineMilliseconds) noexcept;
  void invalidate() noexcept;
  uint64_t generation() const noexcept;
  bool ready() const noexcept;
  std::shared_ptr<std::mutex> contextMutex() noexcept;

 private:
  struct Impl;
  DesktopCapture();
  std::shared_ptr<Impl> impl_;
};

#endif
