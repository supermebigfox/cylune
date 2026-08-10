#include "bridge.h"

#include <new>

namespace {

constexpr uint32_t kAbiVersion = 1;

struct PetHandle {
  PetCallback callback;
  PetConfig config;
  bool visible;
};

PetHandle *from_handle(void *handle) { return static_cast<PetHandle *>(handle); }

} // namespace

extern "C" {

void *pet_create(PetCallback callback, const char *hlsl_source) {
  if (callback == nullptr || hlsl_source == nullptr) {
    return nullptr;
  }
  return new (std::nothrow) PetHandle{callback, {}, false};
}

uint32_t pet_destroy(void *handle) {
  delete from_handle(handle);
  return PET_SHUTDOWN_COMPLETE;
}

bool pet_apply(void *handle, PetConfig config) {
  auto *pet = from_handle(handle);
  if (pet == nullptr || config.abi_version != kAbiVersion) {
    return false;
  }
  pet->config = config;
  pet->visible = config.visible != 0;
  return true;
}

void pet_show(void *handle) {
  if (auto *pet = from_handle(handle)) {
    pet->visible = true;
  }
}

void pet_hide(void *handle) {
  if (auto *pet = from_handle(handle)) {
    pet->visible = false;
  }
}

void pet_reset(void *handle) {
  if (auto *pet = from_handle(handle)) {
    pet->config = {};
    pet->visible = false;
  }
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
