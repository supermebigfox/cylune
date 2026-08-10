#ifndef CYLUNE_WINDOWS_PET_DROP_STATE_H
#define CYLUNE_WINDOWS_PET_DROP_STATE_H

#include "bridge.h"

#include <cmath>
#include <cstdint>
#include <string>
#include <utility>

enum class FileKind : uint32_t { None = 0, ThreeMf = 1, GCode = 2, Other = 3 };

inline uint32_t ResolveDropEffect(uint32_t allowed, bool targetAccepts) {
  constexpr uint32_t kCopy = 1;
  return targetAccepts && (allowed & kCopy) != 0 ? kCopy : 0;
}

enum class PetDropVisualState : uint32_t {
  Idle,
  Hover,
  WaitingForAck,
  SwallowAndSuccessJet,
  SwallowAndEject,
};

struct DropVisualActivity {
  uint32_t targetFps;
  double visualSize;
};

inline DropVisualActivity ResolveDropVisualActivity(
    uint32_t configuredFps, double configuredSize, PetDropVisualState state) {
  const uint32_t idleFps = configuredFps == 0 ? 30 : configuredFps;
  return {state == PetDropVisualState::Idle ? idleFps : 60,
          configuredSize};
}

inline bool PointerInsideDropTarget(double x, double y, double side) {
  if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(side) ||
      side <= 0.0) {
    return false;
  }
  const double center = side * 0.5;
  return std::hypot(x - center, y - center) <= side * 0.48;
}

class DropSession {
 public:
  uint64_t enter(const std::wstring &path, FileKind kind) {
    if (waitingForAck_ || path.empty() || kind == FileKind::None) return 0;
    std::wstring acceptedPath(path);
    (void)leave();
    ++nextGeneration_;
    if (nextGeneration_ == 0) ++nextGeneration_;
    generation_ = nextGeneration_;
    path_ = std::move(acceptedPath);
    kind_ = kind;
    hovering_ = true;
    return generation_;
  }

  bool leave() {
    if (waitingForAck_ || !hovering_) return false;
    clear();
    return true;
  }

  bool submit(uint64_t generation, const std::wstring &path) {
    if (!hovering_ || waitingForAck_ || generation == 0 ||
        generation != generation_ || path != path_) {
      return false;
    }
    hovering_ = false;
    waitingForAck_ = true;
    return true;
  }

  bool finish(uint64_t generation, uint32_t result) {
    if (!waitingForAck_ || generation == 0 || generation != generation_ ||
        (result != PET_DROP_ACCEPTED && result != PET_DROP_REJECTED)) {
      return false;
    }
    clear();
    return true;
  }

  void deactivate() { clear(); }

  uint64_t generation() const { return generation_; }
  FileKind fileKind() const { return kind_; }
  bool hovering() const { return hovering_; }
  bool waitingForAck() const { return waitingForAck_; }

 private:
  void clear() {
    generation_ = 0;
    path_.clear();
    kind_ = FileKind::None;
    hovering_ = false;
    waitingForAck_ = false;
  }

  uint64_t nextGeneration_ = 0;
  uint64_t generation_ = 0;
  std::wstring path_;
  FileKind kind_ = FileKind::None;
  bool hovering_ = false;
  bool waitingForAck_ = false;
};

#endif
