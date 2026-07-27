#ifndef BAMBU_POOLS_PET_VISUAL_STATE_H
#define BAMBU_POOLS_PET_VISUAL_STATE_H

#include <math.h>
#include <stdint.h>

enum class PetVisualSignalEffect {
  kNone = 0,
  kImportSwallow = 1,
  kFailureRedRipple = 2,
  kSettlementGreenRing = 3,
};

struct PetPendingDotPlacement {
  uint32_t ring_index;
  uint32_t index_in_ring;
  uint32_t dots_in_ring;
  double normalized_radius;
  double angle_radians;
};

inline uint32_t PetPendingDotCount(uint32_t pending_count) {
  return pending_count;
}

inline uint64_t PetPendingRingCapacity(uint32_t ring_index) {
  return 8ULL + (4ULL * ring_index);
}

inline uint32_t PetPendingRingCount(uint32_t pending_count) {
  uint64_t remaining = pending_count;
  uint32_t ring_index = 0;
  while (remaining > 0) {
    const uint64_t capacity = PetPendingRingCapacity(ring_index);
    if (remaining <= capacity) {
      return ring_index + 1;
    }
    remaining -= capacity;
    ++ring_index;
  }
  return 0;
}

inline PetPendingDotPlacement PetPendingDotPlacementForIndex(
    uint32_t index, uint32_t pending_count) {
  if (index >= pending_count) {
    return {0, 0, 0, 0.0, 0.0};
  }

  uint64_t ring_start = 0;
  uint32_t ring_index = 0;
  while (index >= ring_start + PetPendingRingCapacity(ring_index)) {
    ring_start += PetPendingRingCapacity(ring_index);
    ++ring_index;
  }

  const uint64_t capacity = PetPendingRingCapacity(ring_index);
  const uint64_t remaining = (uint64_t)pending_count - ring_start;
  const uint32_t dots_in_ring =
      (uint32_t)(remaining < capacity ? remaining : capacity);
  const uint32_t index_in_ring = (uint32_t)((uint64_t)index - ring_start);
  const uint32_t ring_count = PetPendingRingCount(pending_count);
  const double normalized_radius =
      ring_count <= 1
          ? 0.78
          : 0.62 + (0.28 * (double)ring_index / (double)(ring_count - 1));
  constexpr double kTau = 6.2831853071795864769;
  constexpr double kTop = -1.5707963267948966192;
  const double stagger = (ring_index % 2 == 0) ? 0.0 : 0.5;
  const double angle =
      kTop + (kTau * ((double)index_in_ring + stagger) / dots_in_ring);
  return {ring_index, index_in_ring, dots_in_ring, normalized_radius, angle};
}

inline PetVisualSignalEffect PetVisualSignalForCode(uint32_t signal) {
  switch (signal) {
    case 1:
      return PetVisualSignalEffect::kImportSwallow;
    case 2:
      return PetVisualSignalEffect::kFailureRedRipple;
    case 3:
      return PetVisualSignalEffect::kSettlementGreenRing;
    default:
      return PetVisualSignalEffect::kNone;
  }
}

class PetVisualState {
 public:
  void apply_pending_count(uint32_t pending_count) {
    pending_count_ = pending_count;
  }

  void apply_signal(uint32_t signal) {
    signal_effect_ = PetVisualSignalForCode(signal);
  }

  uint32_t pending_count() const { return pending_count_; }
  uint32_t pending_dot_count() const {
    return PetPendingDotCount(pending_count_);
  }
  PetVisualSignalEffect signal_effect() const { return signal_effect_; }

 private:
  uint32_t pending_count_ = 0;
  PetVisualSignalEffect signal_effect_ = PetVisualSignalEffect::kNone;
};

#endif
