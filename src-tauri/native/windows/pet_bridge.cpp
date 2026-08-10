#include "bridge.h"
#include "window.h"

#include <memory>
#include <new>

namespace {

constexpr uint32_t kAbiVersion = 1;

PetWindow *from_handle(void *handle) { return static_cast<PetWindow *>(handle); }

} // namespace

extern "C" {

void *pet_create(PetCallback callback, const char *hlsl_source) {
  if (callback == nullptr || hlsl_source == nullptr) {
    return nullptr;
  }
  return PetWindow::create(callback).release();
}

uint32_t pet_destroy(void *handle) {
  std::unique_ptr<PetWindow> pet(from_handle(handle));
  if (pet == nullptr) return PET_SHUTDOWN_COMPLETE;
  return pet->shutdown();
}

bool pet_apply(void *handle, PetConfig config) {
  auto *pet = from_handle(handle);
  return pet != nullptr && pet->apply(config);
}

void pet_show(void *handle) {
  if (auto *pet = from_handle(handle)) pet->show();
}

void pet_hide(void *handle) {
  if (auto *pet = from_handle(handle)) pet->hide();
}

void pet_reset(void *handle) {
  if (auto *pet = from_handle(handle)) pet->reset();
}

void pet_signal(void *handle, uint32_t signal) {
  (void)handle;
  (void)signal;
}

void pet_finish_drop(void *handle, uint64_t generation, uint32_t result) {
  (void)handle;
  (void)generation;
  (void)result;
}

uint32_t pet_capture_state(void *handle) {
  (void)handle;
  return PET_CAPTURE_UNAVAILABLE;
}

uint32_t pet_renderer_state(void *handle) {
  (void)handle;
  return PET_RENDERER_UNAVAILABLE;
}

uint32_t pet_abi_version(void) { return kAbiVersion; }

} // extern "C"
