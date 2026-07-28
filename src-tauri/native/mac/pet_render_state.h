#ifndef BAMBU_POOLS_PET_RENDER_STATE_H
#define BAMBU_POOLS_PET_RENDER_STATE_H

#include "bridge.h"

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <limits>

enum class PetRenderActivity {
  kIdle,
  kDropHover,
  kSignal,
  kHidden,
};

enum class PetRendererStep {
  kRendered,
  kRetry,
  kBecameUnavailable,
  kUnavailable,
};

struct PetRendererBackend {
  void *context;
  void *(*create)(void *context, const char *source, void *layer);
  uint32_t (*draw)(void *context, void *handle, IOSurfaceRef surface,
                   PetRenderUniforms uniforms);
  void (*destroy)(void *context, void *handle);
};

class PetRendererDriver {
 public:
  PetRendererDriver() = default;
  PetRendererDriver(const PetRendererDriver &) = delete;
  PetRendererDriver &operator=(const PetRendererDriver &) = delete;

  ~PetRendererDriver() { shutdown(); }

  bool initialize(PetRendererBackend backend, const char *source,
                  void *layer) {
    shutdown();
    backend_ = backend;
    has_backend_ = backend.create != nullptr && backend.draw != nullptr &&
                   backend.destroy != nullptr;
    handle_ =
        has_backend_ ? backend_.create(backend_.context, source, layer)
                     : nullptr;
    available_ = handle_ != nullptr;
    report_pending_ = !available_;
    reported_ = false;
    consecutive_failures_ = 0;
    return available_;
  }

  PetRendererStep bind_host() {
    host_bound_ = true;
    if (available_) {
      return PetRendererStep::kRendered;
    }
    return report_unavailable_once();
  }

  PetRendererStep draw(IOSurfaceRef surface, PetRenderUniforms uniforms) {
    if (!available_ || handle_ == nullptr) {
      return report_unavailable_once();
    }
    const uint32_t result =
        backend_.draw(backend_.context, handle_, surface, uniforms);
    if (result == PET_RENDER_DRAW_OK) {
      consecutive_failures_ = 0;
      return PetRendererStep::kRendered;
    }
    if (result == PET_RENDER_DRAW_TRANSIENT) {
      ++consecutive_failures_;
      if (consecutive_failures_ < kTransientFailureLimit) {
        return PetRendererStep::kRetry;
      }
    }
    return degrade();
  }

  bool available() const { return available_; }

  void shutdown() {
    if (handle_ != nullptr && has_backend_) {
      backend_.destroy(backend_.context, handle_);
    }
    handle_ = nullptr;
    available_ = false;
    report_pending_ = false;
    consecutive_failures_ = 0;
  }

 private:
  static constexpr uint32_t kTransientFailureLimit = 3;

  PetRendererStep degrade() {
    if (handle_ != nullptr && has_backend_) {
      backend_.destroy(backend_.context, handle_);
    }
    handle_ = nullptr;
    available_ = false;
    report_pending_ = true;
    return report_unavailable_once();
  }

  PetRendererStep report_unavailable_once() {
    if (host_bound_ && report_pending_ && !reported_) {
      reported_ = true;
      report_pending_ = false;
      return PetRendererStep::kBecameUnavailable;
    }
    return PetRendererStep::kUnavailable;
  }

  PetRendererBackend backend_ = {};
  void *handle_ = nullptr;
  bool has_backend_ = false;
  bool available_ = false;
  bool host_bound_ = false;
  bool report_pending_ = false;
  bool reported_ = false;
  uint32_t consecutive_failures_ = 0;
};

class PetFrameDispatchGate {
 public:
  void set_enabled(bool enabled) {
    enabled_.store(enabled, std::memory_order_release);
    if (!enabled) {
      enqueued_.store(false, std::memory_order_release);
    }
  }

  bool try_enqueue() {
    return enabled_.load(std::memory_order_acquire) &&
           !enqueued_.exchange(true, std::memory_order_acq_rel);
  }

  void complete() {
    enqueued_.store(false, std::memory_order_release);
  }

  bool enabled() const {
    return enabled_.load(std::memory_order_acquire);
  }

 private:
  std::atomic<bool> enqueued_{false};
  std::atomic<bool> enabled_{false};
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

inline constexpr double PetSignalTransitionDuration(
    bool reduce_motion, double standard_duration) {
  return reduce_motion ? 0.15 : standard_duration;
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
    const double transition_duration =
        PetSignalTransitionDuration(reduce_motion, 0.52);
    const float swallow =
        progress(swallow_started_at_, now_seconds, transition_duration);
    const float success =
        progress(success_started_at_, now_seconds,
                 PetSignalTransitionDuration(reduce_motion, 0.48));
    const float error =
        progress(error_started_at_, now_seconds,
                 PetSignalTransitionDuration(reduce_motion, 0.42));
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
