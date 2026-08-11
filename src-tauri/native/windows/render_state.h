#ifndef CYLUNE_WINDOWS_RENDER_STATE_H
#define CYLUNE_WINDOWS_RENDER_STATE_H

#include <algorithm>
#include <chrono>
#include <cstdint>
#include <limits>
#include <utility>

template <typename Clock, typename Duration>
std::chrono::time_point<Clock, Duration> NextRenderDeadline(
    std::chrono::time_point<Clock, Duration> frameStarted,
    uint32_t framesPerSecond) {
  if (framesPerSecond == 0) {
    return std::chrono::time_point<Clock, Duration>::max();
  }
  Duration interval = std::chrono::duration_cast<Duration>(
      std::chrono::duration<double>(1.0 / framesPerSecond));
  if (interval <= Duration::zero()) interval = Duration(1);
  return frameStarted + interval;
}

template <typename TimePoint>
struct HiddenRenderClockState {
  TimePoint lastFrame;
  TimePoint nextFrame;
};

template <typename TimePoint>
HiddenRenderClockState<TimePoint> HiddenRenderClock(TimePoint now) {
  return {now, TimePoint::max()};
}

template <typename TimePoint>
uint32_t FrameWaitMilliseconds(TimePoint nextFrame, TimePoint now) {
  if (nextFrame == TimePoint::max()) return std::numeric_limits<uint32_t>::max();
  if (nextFrame <= now) return 0;
  const auto milliseconds =
      std::chrono::duration_cast<std::chrono::milliseconds>(nextFrame - now)
          .count();
  if (milliseconds <= 0) return 1;
  const auto maximum = std::numeric_limits<uint32_t>::max();
  return milliseconds > static_cast<decltype(milliseconds)>(maximum)
             ? maximum
             : static_cast<uint32_t>(milliseconds);
}

enum class PresentDisposition : uint32_t {
  Presented,
  Retry,
  DeviceFailure,
};

inline PresentDisposition ClassifyPresentResult(int32_t result,
                                                int32_t busyResult) {
  if (result == busyResult) return PresentDisposition::Retry;
  return result >= 0 ? PresentDisposition::Presented
                     : PresentDisposition::DeviceFailure;
}

class SurfacePrimeState {
 public:
  void conceal() { visible_ = false; }

  void invalidatePrime() {
    primed_ = false;
    visible_ = false;
  }

  void markPrimed() { primed_ = true; }

  void applyPrimePresent(PresentDisposition disposition) {
    if (disposition == PresentDisposition::Presented) {
      markPrimed();
    } else {
      invalidatePrime();
    }
  }

  bool reveal() {
    visible_ = primed_;
    return visible_;
  }

  bool primed() const { return primed_; }
  bool canRender() const { return primed_ && visible_; }

 private:
  bool primed_ = false;
  bool visible_ = false;
};

enum class RendererAvailability : uint32_t { Unavailable = 0, Ready = 1 };

class RendererStatusState {
 public:
  RendererAvailability value() const { return value_; }

  bool transition(RendererAvailability value) {
    if (value == value_) return false;
    value_ = value;
    return true;
  }

 private:
  RendererAvailability value_ = RendererAvailability::Unavailable;
};

class RendererRetryState {
 public:
  static constexpr uint32_t kMaximumAttempts = 4;

  void request(uint64_t nowMilliseconds, bool resetBudget) {
    if (resetBudget) attempts_ = 0;
    if (attempts_ >= kMaximumAttempts) return;
    pending_ = true;
    deadlineMilliseconds_ = nowMilliseconds;
  }

  void failed(uint64_t nowMilliseconds) {
    if (attempts_ < kMaximumAttempts) ++attempts_;
    if (attempts_ >= kMaximumAttempts) {
      pending_ = false;
      return;
    }
    const uint32_t shift = attempts_ == 0 ? 0 : attempts_ - 1;
    deadlineMilliseconds_ = nowMilliseconds + (100ULL << shift);
    pending_ = true;
  }

  void succeeded() {
    attempts_ = 0;
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  void cancel() {
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  bool due(uint64_t nowMilliseconds) const {
    return pending_ && nowMilliseconds >= deadlineMilliseconds_;
  }

  bool pending() const { return pending_; }
  uint32_t attempts() const { return attempts_; }
  uint64_t deadlineMilliseconds() const { return deadlineMilliseconds_; }

 private:
  uint32_t attempts_ = 0;
  bool pending_ = false;
  uint64_t deadlineMilliseconds_ = 0;
};

class PresentationRetryState {
 public:
  static constexpr uint32_t kMaximumAttempts = 4;

  bool request(uint64_t nowMilliseconds, bool resetBudget) {
    if (resetBudget) {
      attempts_ = 0;
      pending_ = true;
      deadlineMilliseconds_ = nowMilliseconds;
      return true;
    }
    if (attempts_ >= kMaximumAttempts) return false;
    if (!pending_) {
      pending_ = true;
      deadlineMilliseconds_ = nowMilliseconds;
    }
    return true;
  }

  void failed(uint64_t nowMilliseconds) {
    if (attempts_ < kMaximumAttempts) ++attempts_;
    if (attempts_ >= kMaximumAttempts) {
      pending_ = false;
      return;
    }
    const uint32_t shift = attempts_ == 0 ? 0 : attempts_ - 1;
    deadlineMilliseconds_ = nowMilliseconds + (100ULL << shift);
    pending_ = true;
  }

  void succeeded() {
    attempts_ = 0;
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  void cancel() {
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  bool due(uint64_t nowMilliseconds) const {
    return pending_ && nowMilliseconds >= deadlineMilliseconds_;
  }

  uint32_t waitMilliseconds(uint64_t nowMilliseconds) const {
    if (!pending_) return std::numeric_limits<uint32_t>::max();
    if (deadlineMilliseconds_ <= nowMilliseconds) return 0;
    const uint64_t remaining = deadlineMilliseconds_ - nowMilliseconds;
    return remaining > std::numeric_limits<uint32_t>::max()
               ? std::numeric_limits<uint32_t>::max()
               : static_cast<uint32_t>(remaining);
  }

  bool pending() const { return pending_; }
  uint32_t attempts() const { return attempts_; }
  bool exhausted() const { return attempts_ >= kMaximumAttempts; }

 private:
  uint32_t attempts_ = 0;
  bool pending_ = false;
  uint64_t deadlineMilliseconds_ = 0;
};

struct RendererSettingsInput {
  uint8_t mode = 0;
  uint8_t effectiveMode = 0;
  bool hasPosition = false;
  double x = 0.0;
  double y = 0.0;
  double size = 220.0;
  uint64_t displayId = 0;
  uint32_t fps = 0;
  bool visible = false;
  bool reduceMotion = false;
  uint32_t pendingCount = 0;
  bool requestPermission = false;
  uint8_t visualStyle = 0;
};

class RendererSettingsFingerprintState {
 public:
  bool shouldResetRetry(RendererSettingsInput input) {
    const bool changed =
        !initialized_ || input.mode != value_.mode ||
        input.size != value_.size || input.fps != value_.fps ||
        input.visible != value_.visible ||
        input.reduceMotion != value_.reduceMotion ||
        input.visualStyle != value_.visualStyle;
    value_ = input;
    initialized_ = true;
    return changed;
  }

 private:
  bool initialized_ = false;
  RendererSettingsInput value_{};
};

class RenderPresentationState {
 public:
  void requestVisible(bool visible) {
    requestedVisible_ = visible;
    if (!visible) actuallyVisible_ = false;
  }

  void conceal() { actuallyVisible_ = false; }
  void reveal() { actuallyVisible_ = requestedVisible_; }
  bool requestedVisible() const { return requestedVisible_; }
  bool actuallyVisible() const { return actuallyVisible_; }

 private:
  bool requestedVisible_ = false;
  bool actuallyVisible_ = false;
};

template <typename PrimeOperation, typename ShowOperation>
bool TryPrimeAndShow(RenderPresentationState &presentation,
                     bool prerequisitesReady, PrimeOperation &&prime,
                     ShowOperation &&show) {
  if (!presentation.requestedVisible() || !prerequisitesReady ||
      !std::forward<PrimeOperation>(prime)() ||
      !std::forward<ShowOperation>(show)()) {
    presentation.conceal();
    return false;
  }
  presentation.reveal();
  return presentation.actuallyVisible();
}

template <typename PrimeOperation, typename ShowOperation>
bool TryPrimeAndShowWithRetry(RenderPresentationState &presentation,
                              PresentationRetryState &retry,
                              uint64_t nowMilliseconds,
                              bool prerequisitesReady, bool resetBudget,
                              PrimeOperation &&prime, ShowOperation &&show) {
  if (!presentation.requestedVisible() || !prerequisitesReady) {
    presentation.conceal();
    retry.cancel();
    return false;
  }
  if (!retry.request(nowMilliseconds, resetBudget) ||
      !retry.due(nowMilliseconds)) {
    presentation.conceal();
    return false;
  }
  if (!std::forward<PrimeOperation>(prime)() ||
      !std::forward<ShowOperation>(show)()) {
    presentation.conceal();
    retry.failed(nowMilliseconds);
    return false;
  }
  retry.succeeded();
  presentation.reveal();
  return presentation.actuallyVisible();
}

template <typename ConcealOperation, typename ResizeOperation>
bool TryResizeWhileConcealed(RenderPresentationState &presentation,
                             ConcealOperation &&conceal,
                             ResizeOperation &&resize) {
  presentation.conceal();
  std::forward<ConcealOperation>(conceal)();
  return std::forward<ResizeOperation>(resize)();
}

struct RenderConfig {
  uint32_t fps = 0;
  bool visible = false;
  double size = 220.0;
  uint32_t pendingCount = 0;
  uint8_t visualStyle = 0;
};

enum class RenderVisualState : uint32_t {
  Idle,
  Hover,
  WaitingForAck,
  SwallowAndSuccessJet,
  SwallowAndEject,
};

struct RenderFrameState {
  double animationTime = 0.0;
  double rotationRate = 1.0;
  double pullGain = 1.0;
  double ingestProgress = 0.0;
  double ejectProgress = 0.0;
  double successJetProgress = 0.0;
  uint32_t pendingCount = 0;
  uint32_t shaderStyle = 1;
};

class RenderState {
 public:
  void apply(RenderConfig config) {
    configuredFps_ = config.fps == 30 || config.fps == 60 ? config.fps : 0;
    visible_ = config.visible;
    configuredDiameter_ = std::clamp(config.size, 120.0, 900.0);
    frame_.pendingCount = config.pendingCount;
    frame_.shaderStyle = config.visualStyle == 0 ? 1u : 0u;
  }

  void setVisible(bool visible) { visible_ = visible; }

  void setHover(bool hovering) {
    setHoverProgress(hovering ? 1.0 : 0.0);
  }

  void setHoverProgress(double progress) {
    const double value = std::clamp(progress, 0.0, 1.0);
    hoverProgress_ = value;
    frame_.rotationRate = 1.0 + 1.4 * value;
    frame_.pullGain = 1.0 + 0.7 * value;
  }

  void setVisualState(RenderVisualState state) {
    if (state == visualState_) return;
    const bool continueIngest =
        visualState_ == RenderVisualState::WaitingForAck &&
        (state == RenderVisualState::SwallowAndSuccessJet ||
         state == RenderVisualState::SwallowAndEject);
    visualState_ = state;
    if (state == RenderVisualState::Hover) {
      setHover(true);
      resetAnimationProgress();
      return;
    }
    setHover(false);
    if (continueIngest) {
      animationElapsed_ = std::min(animationElapsed_, kSwallowDuration);
    } else {
      animationElapsed_ = 0.0;
    }
    if (state == RenderVisualState::Idle) resetAnimationProgress();
    updateAnimationProgress();
  }

  void advance(double elapsedSeconds) {
    const double nonnegativeElapsed = std::max(elapsedSeconds, 0.0);
    const double animationStep = std::min(nonnegativeElapsed, 0.1);
    frame_.animationTime += animationStep * frame_.rotationRate;
    animationElapsed_ += nonnegativeElapsed;
    updateAnimationProgress();
  }

  uint32_t targetFps(uint32_t) const {
    if (!visible_) return 0;
    if (configuredFps_ != 0) return configuredFps_;
    return hoverProgress_ > 0.0 || visualState_ != RenderVisualState::Idle
               ? 60u
               : 30u;
  }

  double configuredDiameter() const { return configuredDiameter_; }
  double visualDiameter() const { return configuredDiameter_; }
  RenderVisualState visualState() const { return visualState_; }
  const RenderFrameState &frame() const { return frame_; }

 private:
  static constexpr double kSwallowDuration = 0.74;
  static constexpr double kEjectDuration = 0.62;
  static constexpr double kSuccessJetDuration = 0.50;

  static double Unit(double value) { return std::clamp(value, 0.0, 1.0); }

  void resetAnimationProgress() {
    animationElapsed_ = 0.0;
    frame_.ingestProgress = 0.0;
    frame_.ejectProgress = 0.0;
    frame_.successJetProgress = 0.0;
  }

  void updateAnimationProgress() {
    if (visualState_ == RenderVisualState::WaitingForAck) {
      frame_.ingestProgress = Unit(animationElapsed_ / kSwallowDuration);
      frame_.ejectProgress = 0.0;
      frame_.successJetProgress = 0.0;
      return;
    }
    if (visualState_ == RenderVisualState::SwallowAndSuccessJet) {
      if (animationElapsed_ >= kSwallowDuration + kSuccessJetDuration) {
        visualState_ = RenderVisualState::Idle;
        resetAnimationProgress();
        return;
      }
      frame_.ingestProgress = Unit(animationElapsed_ / kSwallowDuration);
      frame_.ejectProgress = 0.0;
      frame_.successJetProgress =
          Unit((animationElapsed_ - kSwallowDuration) / kSuccessJetDuration);
      return;
    }
    if (visualState_ == RenderVisualState::SwallowAndEject) {
      if (animationElapsed_ >= kSwallowDuration + kEjectDuration) {
        visualState_ = RenderVisualState::Idle;
        resetAnimationProgress();
        return;
      }
      frame_.ingestProgress = Unit(animationElapsed_ / kSwallowDuration);
      frame_.ejectProgress =
          Unit((animationElapsed_ - kSwallowDuration) / kEjectDuration);
      frame_.successJetProgress = 0.0;
      return;
    }
    frame_.ingestProgress = 0.0;
    frame_.ejectProgress = 0.0;
    frame_.successJetProgress = 0.0;
  }

  uint32_t configuredFps_ = 0;
  bool visible_ = false;
  double configuredDiameter_ = 220.0;
  double hoverProgress_ = 0.0;
  double animationElapsed_ = 0.0;
  RenderVisualState visualState_ = RenderVisualState::Idle;
  RenderFrameState frame_{};
};

#endif
