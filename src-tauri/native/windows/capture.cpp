#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#ifndef NOMINMAX
#define NOMINMAX
#endif

#include "capture.h"
#include "capture_state.h"

#include <d3d11.h>
#include <dxgi1_2.h>
#include <wrl/client.h>

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <mutex>
#include <new>
#include <thread>

using Microsoft::WRL::ComPtr;

namespace {

uint32_t RotationValue(DXGI_MODE_ROTATION rotation) noexcept {
  switch (rotation) {
    case DXGI_MODE_ROTATION_ROTATE90: return 90;
    case DXGI_MODE_ROTATION_ROTATE180: return 180;
    case DXGI_MODE_ROTATION_ROTATE270: return 270;
    default: return 0;
  }
}

class FrameLease {
 public:
  explicit FrameLease(IDXGIOutputDuplication *duplication) noexcept
      : duplication_(duplication) {}
  ~FrameLease() { (void)release(); }
  HRESULT release() noexcept {
    if (!active_ || duplication_ == nullptr) return S_OK;
    active_ = false;
    return duplication_->ReleaseFrame();
  }

 private:
  IDXGIOutputDuplication *duplication_ = nullptr;
  bool active_ = true;
};

}  // namespace

struct DesktopCapture::Impl {
  ComPtr<ID3D11Device> device;
  ComPtr<ID3D11DeviceContext> context;
  ComPtr<IDXGIOutputDuplication> duplication;
  ComPtr<ID3D11Texture2D> copiedTexture;
  ComPtr<ID3D11ShaderResourceView> copiedView;
  HMONITOR monitor = nullptr;
  std::thread worker;
  HANDLE stopEvent = nullptr;
  HANDLE exitedEvent = nullptr;
  std::mutex stateMutex;
  std::shared_ptr<std::mutex> contextMutex = std::make_shared<std::mutex>();
  std::condition_variable readyCondition;
  bool startupComplete = false;
  bool startupSucceeded = false;
  bool hasFrame = false;
  bool recoverableFailure = false;
  bool deviceRemoved = false;
  bool failed = false;
  bool stopPending = false;
  uint64_t generation = 0;
  uint64_t workerGeneration = 0;
  uint32_t rotation = 0;
  LUID adapterLuid{};
  CaptureMachine machine;

  ~Impl() {
    if (stopEvent != nullptr) (void)CloseHandle(stopEvent);
    if (exitedEvent != nullptr) (void)CloseHandle(exitedEvent);
  }

  void clearFrameLocked() noexcept { hasFrame = false; }

  bool createDuplication() noexcept {
    ComPtr<IDXGIDevice> dxgiDevice;
    ComPtr<IDXGIAdapter> adapter;
    if (monitor == nullptr || device == nullptr ||
        FAILED(device.As(&dxgiDevice)) ||
        FAILED(dxgiDevice->GetAdapter(adapter.ReleaseAndGetAddressOf()))) {
      return false;
    }
    DXGI_ADAPTER_DESC adapterDescription{};
    if (FAILED(adapter->GetDesc(&adapterDescription))) return false;
    adapterLuid = adapterDescription.AdapterLuid;
    for (UINT index = 0;; ++index) {
      ComPtr<IDXGIOutput> output;
      const HRESULT enumerated =
          adapter->EnumOutputs(index, output.ReleaseAndGetAddressOf());
      if (enumerated == DXGI_ERROR_NOT_FOUND) break;
      if (FAILED(enumerated)) return false;
      DXGI_OUTPUT_DESC outputDescription{};
      if (FAILED(output->GetDesc(&outputDescription)) ||
          outputDescription.Monitor != monitor) continue;
      ComPtr<IDXGIOutput1> output1;
      if (FAILED(output.As(&output1)) ||
          FAILED(output1->DuplicateOutput(device.Get(),
                                          duplication.ReleaseAndGetAddressOf()))) {
        return false;
      }
      DXGI_OUTDUPL_DESC description{};
      duplication->GetDesc(&description);
      rotation = RotationValue(description.Rotation);
      D3D11_TEXTURE2D_DESC texture{};
      texture.Width = description.ModeDesc.Width;
      texture.Height = description.ModeDesc.Height;
      texture.MipLevels = 1;
      texture.ArraySize = 1;
      texture.Format = description.ModeDesc.Format;
      texture.SampleDesc.Count = 1;
      texture.Usage = D3D11_USAGE_DEFAULT;
      texture.BindFlags = D3D11_BIND_SHADER_RESOURCE;
      if (texture.Width == 0 || texture.Height == 0 ||
          FAILED(device->CreateTexture2D(
              &texture, nullptr, copiedTexture.ReleaseAndGetAddressOf())) ||
          FAILED(device->CreateShaderResourceView(
              copiedTexture.Get(), nullptr, copiedView.ReleaseAndGetAddressOf()))) {
        duplication.Reset();
        copiedTexture.Reset();
        copiedView.Reset();
        return false;
      }
      return true;
    }
    return false;
  }

  void workerMain(uint64_t generationToken) noexcept {
    const HRESULT apartment = CoInitializeEx(nullptr, COINIT_MULTITHREADED);
    const bool apartmentInitialized = SUCCEEDED(apartment);
    const bool created = createDuplication();
    {
      std::lock_guard<std::mutex> lock(stateMutex);
      startupSucceeded = created;
      startupComplete = true;
      if (!created) {
        failed = true;
        const CaptureDecision decision = machine.reduce(CaptureEvent::Failed);
        generation = decision.generation;
      }
    }
    readyCondition.notify_all();
    if (!created) {
      if (apartmentInitialized) CoUninitialize();
      (void)SetEvent(exitedEvent);
      return;
    }

    // Desktop Duplication is owned by this worker.  It blocks for at most
    // 16ms; rendering and CopyResource use the same immediate context only
    // while contextMutex is held, so the context is never used concurrently.
    while (WaitForSingleObject(stopEvent, 0) != WAIT_OBJECT_0) {
      DXGI_OUTDUPL_FRAME_INFO info{};
      ComPtr<IDXGIResource> resource;
      const HRESULT acquired = duplication->AcquireNextFrame(
          16, &info, resource.ReleaseAndGetAddressOf());
      if (acquired == DXGI_ERROR_WAIT_TIMEOUT) {
        std::lock_guard<std::mutex> lock(stateMutex);
        if (generation != generationToken) break;
        (void)machine.reduce(CaptureEvent::Timeout);
        clearFrameLocked();
        continue;
      }
      if (acquired == DXGI_ERROR_ACCESS_LOST ||
          acquired == DXGI_ERROR_DEVICE_REMOVED ||
          acquired == DXGI_ERROR_DEVICE_RESET) {
        std::lock_guard<std::mutex> lock(stateMutex);
        if (generation != generationToken) break;
        const CaptureDecision decision = machine.reduce(
            acquired == DXGI_ERROR_DEVICE_REMOVED
                ? CaptureEvent::DeviceRemoved
                : (acquired == DXGI_ERROR_DEVICE_RESET
                       ? CaptureEvent::DeviceReset
                       : CaptureEvent::AccessLost));
        generation = decision.generation;
        clearFrameLocked();
        deviceRemoved = acquired == DXGI_ERROR_DEVICE_REMOVED ||
                        acquired == DXGI_ERROR_DEVICE_RESET;
        recoverableFailure = !deviceRemoved;
        break;
      }
      if (FAILED(acquired)) {
        std::lock_guard<std::mutex> lock(stateMutex);
        if (generation != generationToken) break;
        clearFrameLocked();
        failed = true;
        const CaptureDecision decision = machine.reduce(CaptureEvent::Failed);
        generation = decision.generation;
        break;
      }

      // AcquireNextFrame succeeded: FrameLease releases exactly once, even
      // across a conversion or CopyResource failure.
      FrameLease lease(duplication.Get());
      bool copied = false;
      ComPtr<ID3D11Texture2D> source;
      if (SUCCEEDED(resource.As(&source))) {
        std::lock_guard<std::mutex> contextLock(*contextMutex);
        if (context != nullptr && copiedTexture != nullptr) {
          context->CopyResource(copiedTexture.Get(), source.Get());
          copied = true;
        }
      }
      const HRESULT released = lease.release();
      std::lock_guard<std::mutex> stateLock(stateMutex);
      if (generation != generationToken) break;
      if (FAILED(released)) {
        const CaptureDecision decision = machine.reduce(CaptureEvent::AccessLost);
        generation = decision.generation;
        clearFrameLocked();
        recoverableFailure = true;
        break;
      }
      hasFrame = copied;
      if (copied) {
        const CaptureDecision decision = machine.reduceFrameReady(generation);
        hasFrame = decision.action == CaptureAction::PublishFrame;
      } else {
        failed = true;
        const CaptureDecision decision = machine.reduce(CaptureEvent::Failed);
        generation = decision.generation;
      }
    }
    if (apartmentInitialized) CoUninitialize();
    (void)SetEvent(exitedEvent);
  }

  void releaseAll() noexcept {
    std::lock_guard<std::mutex> contextLock(*contextMutex);
    copiedView.Reset();
    copiedTexture.Reset();
    duplication.Reset();
    monitor = nullptr;
    rotation = 0;
  }
};

DesktopCapture::DesktopCapture() : impl_(std::make_shared<Impl>()) {}
DesktopCapture::~DesktopCapture() {
  if (impl_ != nullptr && !stop(32) && impl_->worker.joinable()) {
    impl_->worker.detach();
    impl_.reset();
  }
}

std::unique_ptr<DesktopCapture> DesktopCapture::create(
    ID3D11Device *device, ID3D11DeviceContext *context) {
  if (device == nullptr || context == nullptr) return nullptr;
  std::unique_ptr<DesktopCapture> capture;
  try {
    capture.reset(new DesktopCapture());
  } catch (...) {
    return nullptr;
  }
  if (capture == nullptr || capture->impl_ == nullptr) return nullptr;
  capture->impl_->device = device;
  capture->impl_->context = context;
  return capture;
}

bool DesktopCapture::start(HMONITOR monitor) noexcept {
  if (impl_ == nullptr || monitor == nullptr || impl_->worker.joinable()) return false;
  impl_->stopEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  impl_->exitedEvent = CreateEventW(nullptr, TRUE, FALSE, nullptr);
  if (impl_->stopEvent == nullptr || impl_->exitedEvent == nullptr) {
    if (impl_->stopEvent != nullptr) (void)CloseHandle(impl_->stopEvent);
    if (impl_->exitedEvent != nullptr) (void)CloseHandle(impl_->exitedEvent);
    impl_->stopEvent = nullptr;
    impl_->exitedEvent = nullptr;
    return false;
  }
  {
    std::lock_guard<std::mutex> lock(impl_->stateMutex);
    const CaptureDecision decision = impl_->machine.reduce(CaptureEvent::Start);
    impl_->generation = decision.generation;
    impl_->workerGeneration = impl_->generation;
    impl_->monitor = monitor;
    impl_->startupComplete = false;
    impl_->startupSucceeded = false;
    impl_->hasFrame = false;
    impl_->recoverableFailure = false;
    impl_->deviceRemoved = false;
    impl_->failed = false;
    impl_->stopPending = false;
  }
  try {
    const uint64_t token = impl_->workerGeneration;
    std::shared_ptr<Impl> keepAlive = impl_;
    impl_->worker = std::thread([keepAlive, token]() {
      keepAlive->workerMain(token);
    });
  } catch (...) {
    (void)CloseHandle(impl_->stopEvent);
    (void)CloseHandle(impl_->exitedEvent);
    impl_->stopEvent = nullptr;
    impl_->exitedEvent = nullptr;
    return false;
  }
  std::unique_lock<std::mutex> lock(impl_->stateMutex);
  const bool started = impl_->readyCondition.wait_for(
      lock, std::chrono::milliseconds(250),
      [this]() { return impl_->startupComplete; });
  if (!started) {
    // Keep the timed-out worker strongly owned and joinable. The owner can
    // observe the failure, retry stop with a deadline, and only then destroy
    // its renderer/device resources.
    lock.unlock();
    (void)SetEvent(impl_->stopEvent);
    lock.lock();
    const CaptureDecision decision =
        impl_->machine.reduce(CaptureEvent::StopDeadline);
    impl_->generation = decision.generation;
    impl_->clearFrameLocked();
    impl_->failed = true;
    impl_->stopPending = true;
    return false;
  }
  return impl_->startupSucceeded;
}

DesktopCaptureStatus DesktopCapture::acquire(
    DWORD timeoutMilliseconds, DesktopCaptureFrame *frame) noexcept {
  (void)timeoutMilliseconds;
  if (frame != nullptr) *frame = {};
  if (impl_ == nullptr) return DesktopCaptureStatus::Failed;
  std::lock_guard<std::mutex> lock(impl_->stateMutex);
  if (impl_->recoverableFailure) return DesktopCaptureStatus::RecoverableFailure;
  if (impl_->deviceRemoved) return DesktopCaptureStatus::DeviceRemoved;
  if (impl_->failed || impl_->copiedView == nullptr) return DesktopCaptureStatus::Failed;
  if (!impl_->hasFrame) return DesktopCaptureStatus::Timeout;
  if (frame != nullptr) {
    frame->view = impl_->copiedView;
    frame->contextMutex = impl_->contextMutex;
    frame->generation = impl_->generation;
    frame->adapterLuid = impl_->adapterLuid;
    frame->monitor = impl_->monitor;
    frame->rotation = impl_->rotation;
  }
  return DesktopCaptureStatus::Frame;
}

bool DesktopCapture::switchDisplay(HMONITOR monitor) noexcept {
  if (impl_ == nullptr || monitor == nullptr) return false;
  {
    std::lock_guard<std::mutex> lock(impl_->stateMutex);
    if (CaptureWorkerMayReuseForDisplay(
            impl_->monitor == monitor, impl_->worker.joinable(),
            impl_->stopPending,
            impl_->machine.phase() == CapturePhase::Running &&
                impl_->startupSucceeded && !impl_->failed &&
                !impl_->recoverableFailure && !impl_->deviceRemoved)) {
      return true;
    }
    impl_->stopPending = true;
  }
  // This is a migration, not terminal destruction: keep CaptureMachine in
  // Running and invalidate its generation before the replacement worker.
  if (impl_->stopEvent != nullptr) (void)SetEvent(impl_->stopEvent);
  if (impl_->worker.joinable()) {
    if (WaitForSingleObject(impl_->exitedEvent, 32) != WAIT_OBJECT_0) return false;
    impl_->worker.join();
  }
  impl_->releaseAll();
  if (impl_->stopEvent != nullptr) {
    (void)CloseHandle(impl_->stopEvent);
    impl_->stopEvent = nullptr;
  }
  if (impl_->exitedEvent != nullptr) {
    (void)CloseHandle(impl_->exitedEvent);
    impl_->exitedEvent = nullptr;
  }
  {
    std::lock_guard<std::mutex> lock(impl_->stateMutex);
    const CaptureDecision decision = impl_->machine.reduce(CaptureEvent::SwitchDisplay);
    impl_->generation = decision.generation;
  }
  return start(monitor);
}

bool DesktopCapture::stop(DWORD deadlineMilliseconds) noexcept {
  if (impl_ == nullptr) return true;
  {
    std::lock_guard<std::mutex> lock(impl_->stateMutex);
    impl_->stopPending = true;
  }
  if (impl_->stopEvent != nullptr) (void)SetEvent(impl_->stopEvent);
  if (impl_->worker.joinable()) {
    const DWORD waited = WaitForSingleObject(impl_->exitedEvent,
                                              deadlineMilliseconds);
    if (waited != WAIT_OBJECT_0) {
      std::lock_guard<std::mutex> lock(impl_->stateMutex);
      const CaptureDecision decision =
          impl_->machine.reduce(CaptureEvent::StopDeadline);
      impl_->generation = decision.generation;
      impl_->clearFrameLocked();
      return false;
    }
    impl_->worker.join();
  }
  {
    std::lock_guard<std::mutex> lock(impl_->stateMutex);
    const CaptureDecision decision = impl_->machine.reduce(CaptureEvent::Stop);
    impl_->generation = decision.generation;
    impl_->clearFrameLocked();
  }
  impl_->releaseAll();
  if (impl_->stopEvent != nullptr) {
    (void)CloseHandle(impl_->stopEvent);
    impl_->stopEvent = nullptr;
  }
  if (impl_->exitedEvent != nullptr) {
    (void)CloseHandle(impl_->exitedEvent);
    impl_->exitedEvent = nullptr;
  }
  return true;
}

void DesktopCapture::invalidate() noexcept {
  if (impl_ == nullptr) return;
  std::lock_guard<std::mutex> lock(impl_->stateMutex);
  const CaptureDecision decision = impl_->machine.reduce(CaptureEvent::AccessLost);
  impl_->generation = decision.generation;
  impl_->clearFrameLocked();
}

uint64_t DesktopCapture::generation() const noexcept {
  if (impl_ == nullptr) return 0;
  std::lock_guard<std::mutex> lock(impl_->stateMutex);
  return impl_->generation;
}

bool DesktopCapture::ready() const noexcept {
  if (impl_ == nullptr) return false;
  std::lock_guard<std::mutex> lock(impl_->stateMutex);
  return impl_->machine.phase() == CapturePhase::Running &&
         impl_->startupSucceeded && !impl_->failed &&
         !impl_->recoverableFailure && !impl_->deviceRemoved;
}

std::shared_ptr<std::mutex> DesktopCapture::contextMutex() noexcept {
  return impl_ == nullptr ? nullptr : impl_->contextMutex;
}
