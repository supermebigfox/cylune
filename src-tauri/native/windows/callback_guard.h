#ifndef CYLUNE_WINDOWS_PET_CALLBACK_GUARD_H
#define CYLUNE_WINDOWS_PET_CALLBACK_GUARD_H

#include "bridge.h"

#include <cstdint>

inline void InvokePetCallbackNoThrow(PetCallback callback, uint32_t kind,
                                     const char *payload, double x, double y,
                                     uint64_t eventValue) noexcept {
  if (callback == nullptr) return;
  try {
    callback(kind, payload, x, y, eventValue);
  } catch (...) {
  }
}

#endif
