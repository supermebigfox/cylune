#ifndef BAMBU_POOLS_PET_DROP_STATE_H
#define BAMBU_POOLS_PET_DROP_STATE_H

#include "bridge.h"

#include <algorithm>
#include <cmath>
#include <cstdint>

enum class PetDropPhase : uint32_t {
  kIdle = 0,
  kExternalHoverValid = 1,
  kImportPending = 2,
  kSwallow = 3,
  kImportRejected = 4,
};

struct PetDropOrigin {
  float x;
  float y;
};

struct PetDropSnapshot {
  PetDropPhase phase;
  uint64_t generation;
  PetDropOrigin origin;
  uint32_t file_kind;
  float hover_progress;
  float faller_progress;
  float absorption_progress;
  float error_progress;
  float reduced_fade;
  uint32_t fragment_count;
  bool deliver_once;
};

struct PetDropRenderSnapshot {
  PetDropSnapshot drop;
  float progress;
  bool reduce_motion;
};

inline void PetApplyDropMotionUniforms(
    const PetDropRenderSnapshot &render,
    PetRenderUniforms &uniforms) {
  uniforms.drop_progress = render.progress;
  uniforms.reduce_motion = render.reduce_motion ? 1u : 0u;
}

class PetDropState {
 public:
  bool begin_wait(uint64_t generation, PetDropOrigin origin,
                  uint32_t file_kind, double now_seconds) {
    if ((phase_ != PetDropPhase::kIdle &&
         phase_ != PetDropPhase::kExternalHoverValid) ||
        generation == 0 || !valid_file_kind(file_kind) ||
        !std::isfinite(now_seconds)) {
      return false;
    }
    phase_ = PetDropPhase::kImportPending;
    generation_ = generation;
    origin_ = origin;
    file_kind_ = file_kind;
    started_at_ = now_seconds;
    delivered_ = false;
    return true;
  }

  bool finish(uint64_t generation, uint32_t result, double now_seconds) {
    if (phase_ != PetDropPhase::kImportPending || generation == 0 ||
        generation != generation_ || !std::isfinite(now_seconds) ||
        (result != PET_DROP_ACCEPTED && result != PET_DROP_REJECTED)) {
      return false;
    }
    phase_ = result == PET_DROP_ACCEPTED ? PetDropPhase::kSwallow
                                        : PetDropPhase::kImportRejected;
    started_at_ = now_seconds;
    delivered_ = false;
    motion_policy_latched_ = false;
    reduced_motion_ = false;
    return true;
  }

  void cancel() { reset(); }

  uint64_t generation() const { return generation_; }

  PetDropRenderSnapshot sample_render(double now_seconds,
                                      bool configured_reduce_motion) {
    const PetDropSnapshot drop =
        sample(now_seconds, configured_reduce_motion);
    const bool active_result =
        drop.phase == PetDropPhase::kSwallow ||
        drop.phase == PetDropPhase::kImportRejected;
    const bool effective_reduce_motion =
        active_result && motion_policy_latched_
            ? reduced_motion_
            : configured_reduce_motion;
    return {
        drop,
        effective_reduce_motion ? drop.reduced_fade
                                : drop.faller_progress,
        effective_reduce_motion,
    };
  }

  PetDropSnapshot sample(double now_seconds, bool reduce_motion) {
    PetDropSnapshot snapshot = current_snapshot();
    if (phase_ == PetDropPhase::kIdle) {
      return snapshot;
    }
    if (phase_ == PetDropPhase::kExternalHoverValid ||
        phase_ == PetDropPhase::kImportPending) {
      snapshot.hover_progress = 1.0f;
      return snapshot;
    }

    const double elapsed = now_seconds - started_at_;
    if (!std::isfinite(elapsed) || elapsed < 0.0) {
      return snapshot;
    }

    if (!motion_policy_latched_) {
      reduced_motion_ = reduce_motion;
      motion_policy_latched_ = true;
    }
    if (reduced_motion_) {
      if (elapsed >= kReducedDuration) {
        reset();
        return current_snapshot();
      }
      snapshot.reduced_fade =
          bounded_progress(elapsed, kReducedDuration);
      if (phase_ == PetDropPhase::kImportRejected) {
        snapshot.error_progress = snapshot.reduced_fade;
      } else if (!delivered_) {
        snapshot.deliver_once = true;
        delivered_ = true;
      }
      return snapshot;
    }

    if (phase_ == PetDropPhase::kImportRejected) {
      if (elapsed >= kRejectedDuration) {
        reset();
        return current_snapshot();
      }
      snapshot.error_progress =
          bounded_progress(elapsed, kRejectedDuration);
      return snapshot;
    }

    if (elapsed >= kSwallowLifetime) {
      reset();
      return current_snapshot();
    }

    snapshot.faller_progress =
        bounded_progress(elapsed, kFallerDuration);
    if (elapsed + kBoundaryTolerance >= kCrossingTime) {
      snapshot.absorption_progress =
          bounded_progress(elapsed - kCrossingTime, kAbsorptionDuration);
      if (!delivered_) {
        snapshot.deliver_once = true;
        delivered_ = true;
      }
    }
    const float u = snapshot.faller_progress;
    snapshot.fragment_count =
        u >= 0.45f && u < 0.88f ? 12u : 0u;
    return snapshot;
  }

 private:
  static constexpr double kFallerDuration = 4.6;
  static constexpr double kCrossingTime = kFallerDuration * 0.82;
  static constexpr double kAbsorptionDuration = 0.90;
  static constexpr double kSwallowLifetime =
      kCrossingTime + kAbsorptionDuration;
  static constexpr double kRejectedDuration = 0.42;
  static constexpr double kReducedDuration = 0.15;
  static constexpr double kBoundaryTolerance = 1e-9;

  static bool valid_file_kind(uint32_t file_kind) {
    return file_kind == PET_FILE_3MF || file_kind == PET_FILE_GCODE;
  }

  static float bounded_progress(double elapsed, double duration) {
    return static_cast<float>(
        std::clamp(elapsed / duration, 0.0, 1.0));
  }

  PetDropSnapshot current_snapshot() const {
    return {
        phase_, generation_, origin_, file_kind_,
        0.0f,   0.0f,        0.0f,    0.0f,
        0.0f,   0u,          false,
    };
  }

  void reset() {
    phase_ = PetDropPhase::kIdle;
    generation_ = 0;
    origin_ = {0.0f, 0.0f};
    file_kind_ = PET_FILE_NONE;
    started_at_ = 0.0;
    delivered_ = false;
    motion_policy_latched_ = false;
    reduced_motion_ = false;
  }

  PetDropPhase phase_ = PetDropPhase::kIdle;
  uint64_t generation_ = 0;
  PetDropOrigin origin_ = {0.0f, 0.0f};
  uint32_t file_kind_ = PET_FILE_NONE;
  double started_at_ = 0.0;
  bool delivered_ = false;
  bool motion_policy_latched_ = false;
  bool reduced_motion_ = false;
};

#endif
