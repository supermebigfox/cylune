#ifndef BAMBU_POOLS_PET_LIFECYCLE_H
#define BAMBU_POOLS_PET_LIFECYCLE_H

#include "bridge.h"

#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <condition_variable>
#include <cstddef>
#include <cstdint>
#include <mutex>

struct PetPanelFrame {
  double x;
  double y;
  double width;
  double height;
};

struct PetScreenFrame {
  double x;
  double y;
  double width;
  double height;
  double scale;
  uint32_t display_id;
};

inline double PetPanelIntersectionArea(PetPanelFrame panel,
                                       PetScreenFrame display) {
  const double left = std::max(panel.x, display.x);
  const double bottom = std::max(panel.y, display.y);
  const double right =
      std::min(panel.x + panel.width, display.x + display.width);
  const double top =
      std::min(panel.y + panel.height, display.y + display.height);
  return std::max(0.0, right - left) * std::max(0.0, top - bottom);
}

inline size_t PetGreatestIntersectionDisplayIndex(
    PetPanelFrame panel, const PetScreenFrame *displays, size_t count) {
  if (displays == nullptr || count == 0) {
    return 0;
  }
  size_t selected = 0;
  double greatest_area = PetPanelIntersectionArea(panel, displays[0]);
  for (size_t index = 1; index < count; ++index) {
    const double area = PetPanelIntersectionArea(panel, displays[index]);
    if (area > greatest_area) {
      selected = index;
      greatest_area = area;
    }
  }
  return selected;
}

inline PetPanelFrame PetClampPanelToDisplay(PetPanelFrame panel,
                                            PetScreenFrame display,
                                            double safe_inset) {
  const double minimum_x = display.x + safe_inset;
  const double maximum_x =
      display.x + display.width - panel.width - safe_inset;
  const double minimum_y = display.y + safe_inset;
  const double maximum_y =
      display.y + display.height - panel.height - safe_inset;
  panel.x = minimum_x <= maximum_x
                ? std::clamp(panel.x, minimum_x, maximum_x)
                : display.x + (display.width - panel.width) / 2.0;
  panel.y = minimum_y <= maximum_y
                ? std::clamp(panel.y, minimum_y, maximum_y)
                : display.y + (display.height - panel.height) / 2.0;
  return panel;
}

class PetDragPersistenceGate {
 public:
  void begin() {
    active_ = true;
    dragged_ = false;
    persisted_ = false;
  }

  void mark_dragged() {
    if (active_) {
      dragged_ = true;
    }
  }

  bool dragged() const { return dragged_; }

  bool should_persist(bool mouse_up) {
    if (!active_ || !dragged_ || !mouse_up || persisted_) {
      return false;
    }
    persisted_ = true;
    active_ = false;
    return true;
  }

 private:
  bool active_ = false;
  bool dragged_ = false;
  bool persisted_ = false;
};

struct PetCaptureConfigurationKey {
  uint32_t mode;
  bool visible;
  PetCaptureRegion region;
};

inline bool PetCaptureRegionsEqual(PetCaptureRegion lhs,
                                   PetCaptureRegion rhs) {
  return lhs.display_id == rhs.display_id &&
         lhs.source_x == rhs.source_x && lhs.source_y == rhs.source_y &&
         lhs.source_width == rhs.source_width &&
         lhs.source_height == rhs.source_height &&
         lhs.pixel_width == rhs.pixel_width &&
         lhs.pixel_height == rhs.pixel_height;
}

inline bool PetCaptureConfigurationKeysEqual(
    PetCaptureConfigurationKey lhs, PetCaptureConfigurationKey rhs) {
  return lhs.mode == rhs.mode && lhs.visible == rhs.visible &&
         PetCaptureRegionsEqual(lhs.region, rhs.region);
}

class PetCaptureConfigurationGate {
 public:
  bool should_configure(PetCaptureConfigurationKey key, bool force) {
    if (!force && has_key_ &&
        PetCaptureConfigurationKeysEqual(key_, key)) {
      return false;
    }
    key_ = key;
    has_key_ = true;
    return true;
  }

  void invalidate() { has_key_ = false; }

 private:
  bool has_key_ = false;
  PetCaptureConfigurationKey key_ = {};
};

class PetFaultLatch {
 public:
  bool report_once() {
    if (reported_) {
      return false;
    }
    reported_ = true;
    return true;
  }

  void reset() { reported_ = false; }

 private:
  bool reported_ = false;
};

struct PetCapturePolicy {
  bool captures_audio;
  bool captures_microphone;
  bool shows_cursor;
  bool excludes_own_process;
  uint32_t queue_depth;
  uint32_t maximum_retained_frames;
};

inline constexpr PetCapturePolicy PetSafeCapturePolicy() {
  return {false, false, false, true, 1, 1};
}

struct PetRendererDecision {
  uint32_t state;
  bool real_effect_available;
  bool stop_capture;
};

inline constexpr PetRendererDecision
PetRendererDecisionForMetalAvailability(bool metal_available) {
  return metal_available
             ? PetRendererDecision{PET_RENDERER_READY, true, false}
             : PetRendererDecision{PET_RENDERER_UNAVAILABLE, false, true};
}

enum class PetShutdownState : uint32_t {
  kComplete = PET_SHUTDOWN_COMPLETE,
  kStopFailed = PET_SHUTDOWN_STOP_FAILED,
  kStopTimedOut = PET_SHUTDOWN_STOP_TIMED_OUT,
};

class PetStopCompletion {
 public:
  void complete(bool success) {
    std::lock_guard<std::mutex> lock(mutex_);
    if (state_ != State::kPending) {
      return;
    }
    state_ = success ? State::kComplete : State::kStopFailed;
    condition_.notify_all();
  }

  PetShutdownState wait_for(std::chrono::milliseconds timeout) {
    std::unique_lock<std::mutex> lock(mutex_);
    if (!condition_.wait_for(lock, timeout,
                             [this] { return state_ != State::kPending; })) {
      state_ = State::kStopTimedOut;
    }
    return public_state();
  }

  PetShutdownState state() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return public_state();
  }

  bool done() const {
    std::lock_guard<std::mutex> lock(mutex_);
    return state_ != State::kPending;
  }

 private:
  enum class State {
    kPending,
    kComplete,
    kStopFailed,
    kStopTimedOut,
  };

  PetShutdownState public_state() const {
    switch (state_) {
      case State::kComplete:
        return PetShutdownState::kComplete;
      case State::kStopFailed:
        return PetShutdownState::kStopFailed;
      case State::kStopTimedOut:
        return PetShutdownState::kStopTimedOut;
      case State::kPending:
        return PetShutdownState::kStopTimedOut;
    }
  }

  mutable std::mutex mutex_;
  std::condition_variable condition_;
  State state_ = State::kPending;
};

inline constexpr std::chrono::milliseconds PetCaptureShutdownTimeout() {
  return std::chrono::seconds(2);
}

class PetFrameRetention {
 public:
  void start() {
    accepting_.store(true, std::memory_order_release);
  }

  void stop() {
    accepting_.store(false, std::memory_order_release);
  }

  bool accepting() const {
    return accepting_.load(std::memory_order_acquire);
  }

 private:
  std::atomic<bool> accepting_{false};
};

enum class PetPermissionAction {
  kNone,
  kRequestSystemPermission,
  kEnumerateCapture,
};

struct PetPermissionDecision {
  uint32_t state;
  PetPermissionAction action;
};

struct PetApplyCapturePlan {
  bool refresh_capture;
  bool request_permission;
};

inline constexpr PetApplyCapturePlan PetApplyCapturePlanForVisibility(
    bool visible, bool explicit_real_mode_action) {
  return {visible || explicit_real_mode_action, explicit_real_mode_action};
}

inline constexpr bool PetShouldStartCapture(PetPermissionDecision decision,
                                            bool visible) {
  return visible &&
         decision.action == PetPermissionAction::kEnumerateCapture;
}

class PetPermissionLifecycle {
 public:
  PetPermissionDecision preflight(bool granted,
                                  bool explicit_real_mode_action) {
    if (granted) {
      if (request_attempted_ &&
          (state_ == PET_CAPTURE_NOT_DETERMINED ||
           state_ == PET_CAPTURE_DENIED ||
           state_ == PET_CAPTURE_RESTART_REQUIRED)) {
        state_ = PET_CAPTURE_RESTART_REQUIRED;
        return {state_, PetPermissionAction::kNone};
      }
      state_ = PET_CAPTURE_READY;
      return {state_, PetPermissionAction::kEnumerateCapture};
    }

    if (!explicit_real_mode_action) {
      if (state_ == PET_CAPTURE_READY) {
        state_ = PET_CAPTURE_DENIED;
      } else if (state_ != PET_CAPTURE_DENIED &&
                 state_ != PET_CAPTURE_RESTART_REQUIRED) {
        state_ = PET_CAPTURE_NOT_DETERMINED;
      }
      return {state_, PetPermissionAction::kNone};
    }
    if (request_attempted_) {
      if (state_ != PET_CAPTURE_RESTART_REQUIRED) {
        state_ = PET_CAPTURE_DENIED;
      }
      return {state_, PetPermissionAction::kNone};
    }

    request_attempted_ = true;
    state_ = PET_CAPTURE_NOT_DETERMINED;
    return {state_, PetPermissionAction::kRequestSystemPermission};
  }

  PetPermissionDecision request_result(bool granted) {
    state_ =
        granted ? PET_CAPTURE_RESTART_REQUIRED : PET_CAPTURE_DENIED;
    return {state_, PetPermissionAction::kNone};
  }

 private:
  uint32_t state_ = PET_CAPTURE_UNAVAILABLE;
  bool request_attempted_ = false;
};

inline PetCaptureRegion PetCaptureRegionForPanel(PetPanelFrame panel,
                                                 PetScreenFrame display) {
  constexpr double kLensExpansionFactor = 1.24;
  const double requested_side = std::floor(panel.width * kLensExpansionFactor);
  const double margin = (requested_side - panel.width) / 2.0;
  const double requested_x = panel.x - margin;
  const double requested_y = panel.y - margin;
  const double display_right = display.x + display.width;
  const double display_top = display.y + display.height;
  const double capture_x =
      std::clamp(requested_x, display.x, display_right);
  const double capture_y =
      std::clamp(requested_y, display.y, display_top);
  const double capture_right =
      std::clamp(requested_x + requested_side, display.x, display_right);
  const double capture_top =
      std::clamp(requested_y + requested_side, display.y, display_top);
  const double width = std::max(0.0, capture_right - capture_x);
  const double height = std::max(0.0, capture_top - capture_y);

  return {
      display.display_id,
      capture_x - display.x,
      display_top - capture_top,
      width,
      height,
      static_cast<uint32_t>(std::llround(width * display.scale)),
      static_cast<uint32_t>(std::llround(height * display.scale)),
  };
}

struct PetEventHorizonGeometry {
  double decorative_effect_diameter;
  double event_horizon_diameter;
  double core_hit_target_side;
};

inline PetEventHorizonGeometry PetEventHorizonGeometryForEffectDiameter(
    double effect_diameter) {
  constexpr double kSquareRootOfTwo = 1.4142135623730950488;
  constexpr double kCoreHitTargetInset = 1.0;
  const double core_diameter =
      effect_diameter / kSquareRootOfTwo - (2.0 * kCoreHitTargetInset);
  const double clamped_core_diameter =
      core_diameter > 0.0 ? core_diameter : 0.0;
  return {effect_diameter > 0.0 ? effect_diameter : 0.0,
          clamped_core_diameter, clamped_core_diameter};
}

class PetWindowLifecycle {
 public:
  bool show() {
    if (destroyed_ || (visual_visible_ && core_hit_target_visible_)) {
      return false;
    }
    visual_visible_ = true;
    core_hit_target_visible_ = true;
    return true;
  }

  bool hide() {
    if (!visual_visible_ && !core_hit_target_visible_) {
      return false;
    }
    visual_visible_ = false;
    core_hit_target_visible_ = false;
    return true;
  }

  bool destroy() {
    if (destroyed_) {
      return false;
    }
    visual_visible_ = false;
    core_hit_target_visible_ = false;
    destroyed_ = true;
    return true;
  }

  bool visual_visible() const { return visual_visible_; }
  bool core_hit_target_visible() const { return core_hit_target_visible_; }
  bool destroyed() const { return destroyed_; }

 private:
  bool visual_visible_ = false;
  bool core_hit_target_visible_ = false;
  bool destroyed_ = false;
};

#endif
