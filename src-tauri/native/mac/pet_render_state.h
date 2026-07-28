#ifndef BAMBU_POOLS_PET_RENDER_STATE_H
#define BAMBU_POOLS_PET_RENDER_STATE_H

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>

enum class PetRenderActivity {
  kIdle,
  kDropHover,
  kSignal,
  kHidden,
};

inline constexpr uint32_t PetTargetFps(uint32_t configured_fps,
                                      PetRenderActivity activity) {
  if (activity == PetRenderActivity::kHidden) {
    return 0;
  }
  if (configured_fps == 30 || configured_fps == 60) {
    return configured_fps;
  }
  return activity == PetRenderActivity::kIdle ? 30 : 60;
}

struct PetAnimationSnapshot {
  float hover_progress;
  float swallow_progress;
  float success_progress;
  float error_progress;
  PetRenderActivity activity;
};

class PetRenderAnimationState {
 public:
  void set_hover(bool hovering, double now_seconds) {
    if (hovering_ == hovering) {
      return;
    }
    hovering_ = hovering;
    hover_changed_at_ = now_seconds;
  }

  void complete_drop(double now_seconds) {
    set_hover(false, now_seconds);
  }

  void signal(uint32_t signal, double now_seconds) {
    switch (signal) {
      case 1:
        swallow_started_at_ = now_seconds;
        break;
      case 2:
        error_started_at_ = now_seconds;
        break;
      case 3:
        success_started_at_ = now_seconds;
        break;
      default:
        break;
    }
  }

  PetAnimationSnapshot sample(double now_seconds, bool reduce_motion) const {
    const float hover = reduce_motion ? 0.0f : hover_progress(now_seconds);
    const double transition_duration = reduce_motion ? 0.15 : 0.52;
    const float swallow =
        progress(swallow_started_at_, now_seconds, transition_duration);
    const float success =
        progress(success_started_at_, now_seconds,
                 reduce_motion ? 0.15 : 0.48);
    const float error =
        progress(error_started_at_, now_seconds,
                 reduce_motion ? 0.15 : 0.42);
    const bool signal_active =
        swallow > 0.0f || success > 0.0f || error > 0.0f;
    return {
        hover,
        swallow,
        success,
        error,
        signal_active ? PetRenderActivity::kSignal
                      : (hover > 0.0f ? PetRenderActivity::kDropHover
                                      : PetRenderActivity::kIdle),
    };
  }

 private:
  static constexpr double kNotStarted =
      -std::numeric_limits<double>::infinity();

  static float smoothstep(float value) {
    const float bounded = std::clamp(value, 0.0f, 1.0f);
    return bounded * bounded * (3.0f - 2.0f * bounded);
  }

  static float progress(double started_at, double now_seconds,
                        double duration) {
    const double elapsed = now_seconds - started_at;
    if (!std::isfinite(elapsed) || elapsed < 0.0 || elapsed >= duration) {
      return 0.0f;
    }
    return static_cast<float>(elapsed / duration);
  }

  float hover_progress(double now_seconds) const {
    const double elapsed = now_seconds - hover_changed_at_;
    const float transition =
        smoothstep(static_cast<float>(elapsed / 0.15));
    return hovering_ ? transition : 1.0f - transition;
  }

  bool hovering_ = false;
  double hover_changed_at_ = kNotStarted;
  double swallow_started_at_ = kNotStarted;
  double success_started_at_ = kNotStarted;
  double error_started_at_ = kNotStarted;
};

#endif
