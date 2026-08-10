#ifndef CYLUNE_WINDOWS_RENDER_STATE_H
#define CYLUNE_WINDOWS_RENDER_STATE_H

#include <algorithm>
#include <chrono>
#include <cstdint>

template <typename Clock, typename Duration>
std::chrono::time_point<Clock, Duration> NextRenderDeadline(
    std::chrono::time_point<Clock, Duration> frameCompleted,
    uint32_t framesPerSecond) {
  if (framesPerSecond == 0) {
    return std::chrono::time_point<Clock, Duration>::max();
  }
  Duration interval = std::chrono::duration_cast<Duration>(
      std::chrono::duration<double>(1.0 / framesPerSecond));
  if (interval <= Duration::zero()) interval = Duration(1);
  return frameCompleted + interval;
}

struct RenderConfig {
  uint32_t fps = 0;
  bool visible = false;
  double size = 300.0;
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
    configuredDiameter_ = std::clamp(config.size, 300.0, 900.0);
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
  double configuredDiameter_ = 300.0;
  double hoverProgress_ = 0.0;
  double animationElapsed_ = 0.0;
  RenderVisualState visualState_ = RenderVisualState::Idle;
  RenderFrameState frame_{};
};

#endif
