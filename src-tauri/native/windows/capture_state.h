#ifndef CYLUNE_WINDOWS_CAPTURE_STATE_H
#define CYLUNE_WINDOWS_CAPTURE_STATE_H

#include <cstdint>
#include <limits>

enum class CaptureEvent : uint8_t {
  Start, Timeout, AccessLost, DeviceRemoved, DeviceReset, Rotation0, Rotation90,
  Rotation180, Rotation270, SwitchDisplay, Sleep, Wake, RecoveryReady, Failed, Stop,
  StopDeadline,
};

struct CaptureFrameIdentity {
  uint64_t generation;
  uint64_t monitor;
  uint32_t adapterLow;
  int32_t adapterHigh;
};

inline bool CaptureFrameMatchesOwner(
    const CaptureFrameIdentity &owner,
    const CaptureFrameIdentity &frame) noexcept {
  return owner.generation == frame.generation &&
         owner.monitor == frame.monitor &&
         owner.adapterLow == frame.adapterLow &&
         owner.adapterHigh == frame.adapterHigh;
}

inline uint32_t CaptureAvailabilityAfterPause(
    uint32_t currentAvailability) noexcept {
  return currentAvailability;
}

inline uint32_t CaptureAvailabilityAfterRendererRebuild(
    uint32_t currentAvailability, bool finalShutdown) noexcept {
  return finalShutdown ? 0U : currentAvailability;
}

inline bool CaptureWorkerMayReuseForDisplay(
    bool sameMonitor, bool joinable, bool stopPending,
    bool ready) noexcept {
  return sameMonitor && joinable && !stopPending && ready;
}

enum class CaptureAction : uint8_t {
  None, CreateDuplication, PublishFrame, ClearFrame, RecreateDuplication,
  EnumerateDisplays, ReleaseAll, ProceduralFallback, StopTimedOut,
};

enum class CaptureRotation : uint8_t {
  Identity, Rotate90, Rotate180, Rotate270,
};

enum class CapturePhase : uint8_t { Dormant, Running, Recovering, Sleeping, Failed, Stopped };

struct CaptureUv { float x; float y; };
inline CaptureUv RotateCaptureUv(CaptureUv uv, CaptureRotation rotation) noexcept {
  switch (rotation) {
    case CaptureRotation::Rotate90: return {uv.y, 1.0f - uv.x};
    case CaptureRotation::Rotate180: return {1.0f - uv.x, 1.0f - uv.y};
    case CaptureRotation::Rotate270: return {1.0f - uv.y, uv.x};
    default: return uv;
  }
}

struct CaptureDecision {
  CaptureAction action = CaptureAction::None;
  bool desktopAvailable = false;
  bool rendererRemainsReady = true;
  bool stopBounded = false;
  uint64_t generation = 0;
};

class CaptureRetryState {
 public:
  static constexpr uint32_t kMaximumAttempts = 4;

  bool request(uint64_t nowMilliseconds, bool resetBudget) noexcept {
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

  void failed(uint64_t nowMilliseconds) noexcept {
    if (attempts_ < kMaximumAttempts) ++attempts_;
    if (attempts_ >= kMaximumAttempts) {
      pending_ = false;
      return;
    }
    deadlineMilliseconds_ =
        nowMilliseconds + (100ULL << (attempts_ - 1));
    pending_ = true;
  }

  void succeeded() noexcept {
    attempts_ = 0;
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  void cancel() noexcept {
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  bool due(uint64_t nowMilliseconds) const noexcept {
    return pending_ && nowMilliseconds >= deadlineMilliseconds_;
  }

  uint32_t waitMilliseconds(uint64_t nowMilliseconds) const noexcept {
    if (!pending_) return std::numeric_limits<uint32_t>::max();
    if (deadlineMilliseconds_ <= nowMilliseconds) return 0;
    const uint64_t remaining = deadlineMilliseconds_ - nowMilliseconds;
    return remaining > std::numeric_limits<uint32_t>::max()
               ? std::numeric_limits<uint32_t>::max()
               : static_cast<uint32_t>(remaining);
  }

  bool pending() const noexcept { return pending_; }
  bool exhausted() const noexcept {
    return attempts_ >= kMaximumAttempts && !pending_;
  }
  uint32_t attempts() const noexcept { return attempts_; }

 private:
  uint32_t attempts_ = 0;
  bool pending_ = false;
  uint64_t deadlineMilliseconds_ = 0;
};

class CaptureStopRetryState {
 public:
  static constexpr uint32_t kMaximumAttempts = 4;

  bool request(uint64_t nowMilliseconds, bool resetBudget) noexcept {
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

  void failed(uint64_t nowMilliseconds) noexcept {
    if (attempts_ < kMaximumAttempts) ++attempts_;
    if (attempts_ >= kMaximumAttempts) {
      pending_ = false;
      return;
    }
    deadlineMilliseconds_ =
        nowMilliseconds + (25ULL << (attempts_ - 1));
    pending_ = true;
  }

  void succeeded() noexcept {
    attempts_ = 0;
    pending_ = false;
    deadlineMilliseconds_ = 0;
  }

  bool due(uint64_t nowMilliseconds) const noexcept {
    return pending_ && nowMilliseconds >= deadlineMilliseconds_;
  }

  uint32_t waitMilliseconds(uint64_t nowMilliseconds) const noexcept {
    if (!pending_) return std::numeric_limits<uint32_t>::max();
    if (deadlineMilliseconds_ <= nowMilliseconds) return 0;
    const uint64_t remaining = deadlineMilliseconds_ - nowMilliseconds;
    return remaining > std::numeric_limits<uint32_t>::max()
               ? std::numeric_limits<uint32_t>::max()
               : static_cast<uint32_t>(remaining);
  }

  bool pending() const noexcept { return pending_; }
  bool exhausted() const noexcept {
    return attempts_ >= kMaximumAttempts && !pending_;
  }

 private:
  uint32_t attempts_ = 0;
  bool pending_ = false;
  uint64_t deadlineMilliseconds_ = 0;
};

// No DXGI/Win32 types: all generation and terminal-state policy is testable.
class CaptureMachine {
 public:
  CaptureDecision reduce(CaptureEvent event) noexcept {
    switch (event) {
      case CaptureEvent::Start:
        if (phase_ != CapturePhase::Dormant) return decision();
        phase_ = CapturePhase::Running;
        return invalidate(CaptureAction::CreateDuplication);
      case CaptureEvent::Timeout:
        return decision();
      case CaptureEvent::AccessLost:
        return recover(CaptureAction::RecreateDuplication, true);
      case CaptureEvent::DeviceRemoved:
      case CaptureEvent::DeviceReset:
        return recover(CaptureAction::RecreateDuplication, false);
      case CaptureEvent::Rotation0:
        return setRotation(CaptureRotation::Identity);
      case CaptureEvent::Rotation90:
        return setRotation(CaptureRotation::Rotate90);
      case CaptureEvent::Rotation180:
        return setRotation(CaptureRotation::Rotate180);
      case CaptureEvent::Rotation270:
        return setRotation(CaptureRotation::Rotate270);
      case CaptureEvent::SwitchDisplay:
        if (phase_ != CapturePhase::Running) return decision();
        return invalidate(CaptureAction::EnumerateDisplays);
      case CaptureEvent::Sleep:
        if (phase_ != CapturePhase::Running) return decision();
        phase_ = CapturePhase::Sleeping;
        return invalidate(CaptureAction::ReleaseAll);
      case CaptureEvent::Wake:
        if (phase_ != CapturePhase::Sleeping) return decision();
        phase_ = CapturePhase::Running;
        return invalidate(CaptureAction::EnumerateDisplays);
      case CaptureEvent::RecoveryReady:
        if (phase_ != CapturePhase::Recovering) return decision();
        phase_ = CapturePhase::Running;
        return decision(CaptureAction::CreateDuplication);
      case CaptureEvent::Failed:
        if (phase_ == CapturePhase::Failed || phase_ == CapturePhase::Stopped) return decision();
        phase_ = CapturePhase::Failed;
        return invalidate(CaptureAction::ProceduralFallback);
      case CaptureEvent::Stop:
        if (phase_ == CapturePhase::Stopped) return decision();
        phase_ = CapturePhase::Stopped;
        return invalidate(CaptureAction::ReleaseAll);
      case CaptureEvent::StopDeadline: {
        if (phase_ != CapturePhase::Stopped) {
          phase_ = CapturePhase::Stopped;
          ++generation_;
          hasCurrentFrame_ = false;
        }
        CaptureDecision result = decision(CaptureAction::StopTimedOut);
        result.desktopAvailable = false;
        result.stopBounded = true;
        return result;
      }
    }
    return decision();
  }

  // A worker's old generation may never republish after sleep, migration, or
  // recovery. The production capture worker calls this with its frame tag.
  CaptureDecision reduceFrameReady(uint64_t frameGeneration) noexcept {
    if (phase_ != CapturePhase::Running || frameGeneration != generation_) {
      return decision(CaptureAction::ClearFrame);
    }
    hasCurrentFrame_ = true;
    return decision(CaptureAction::PublishFrame);
  }

  uint64_t generation() const noexcept { return generation_; }
  bool hasCurrentFrame() const noexcept { return hasCurrentFrame_; }
  CaptureRotation rotation() const noexcept { return rotation_; }
  CapturePhase phase() const noexcept { return phase_; }

 private:
  CaptureDecision decision(CaptureAction action = CaptureAction::None) const noexcept {
    return {action, phase_ == CapturePhase::Running && hasCurrentFrame_,
            true, false, generation_};
  }
  CaptureDecision invalidate(CaptureAction action) noexcept {
    ++generation_;
    hasCurrentFrame_ = false;
    return decision(action);
  }
  CaptureDecision recover(CaptureAction action, bool rendererReady) noexcept {
    if (phase_ != CapturePhase::Running) return decision();
    phase_ = CapturePhase::Recovering;
    CaptureDecision result = invalidate(action);
    result.rendererRemainsReady = rendererReady;
    return result;
  }
  CaptureDecision setRotation(CaptureRotation rotation) noexcept {
    if (phase_ != CapturePhase::Running || rotation_ == rotation) return decision();
    rotation_ = rotation;
    return invalidate(CaptureAction::ClearFrame);
  }

  uint64_t generation_ = 0;
  bool hasCurrentFrame_ = false;
  CaptureRotation rotation_ = CaptureRotation::Identity;
  CapturePhase phase_ = CapturePhase::Dormant;
};

#endif
