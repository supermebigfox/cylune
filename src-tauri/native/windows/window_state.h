#ifndef CYLUNE_WINDOWS_PET_WINDOW_STATE_H
#define CYLUNE_WINDOWS_PET_WINDOW_STATE_H

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
#include <utility>
#include <vector>

enum class PetHit { Transparent, Drag };

struct LogicalPoint {
  double x;
  double y;
};

struct DisplayInfo {
  uint64_t id;
  double x;
  double y;
  double width;
  double height;
  double scale;
  double physicalX;
  double physicalY;

  DisplayInfo(uint64_t displayId, double logicalX, double logicalY,
              double logicalWidth, double logicalHeight,
              double displayScale)
      : id(displayId),
        x(logicalX),
        y(logicalY),
        width(logicalWidth),
        height(logicalHeight),
        scale(displayScale),
        physicalX(logicalX),
        physicalY(logicalY) {}

  DisplayInfo(uint64_t displayId, double logicalX, double logicalY,
              double logicalWidth, double logicalHeight, double displayScale,
              double physicalOriginX, double physicalOriginY)
      : id(displayId),
        x(logicalX),
        y(logicalY),
        width(logicalWidth),
        height(logicalHeight),
        scale(displayScale),
        physicalX(physicalOriginX),
        physicalY(physicalOriginY) {}
};

struct Placement {
  uint64_t displayId;
  double x;
  double y;
  double size;
};

inline bool PlacementPositionChanged(const Placement &before,
                                     const Placement &after) {
  return before.displayId != after.displayId || before.x != after.x ||
         before.y != after.y;
}

struct PixelRegionBounds {
  int left;
  int top;
  int right;
  int bottom;
};

inline PixelRegionBounds VisualEffectBounds(PixelRegionBounds monitor,
                                            int centerX, int centerY,
                                            int visualDiameter) {
  const int monitorWidth = std::max(1, monitor.right - monitor.left);
  const int monitorHeight = std::max(1, monitor.bottom - monitor.top);
  const int requestedSide = std::max(
      1, static_cast<int>(std::ceil(std::max(1, visualDiameter) * 1.5)));
  const int width = std::min(requestedSide, monitorWidth);
  const int height = std::min(requestedSide, monitorHeight);
  const int maximumLeft = monitor.right - width;
  const int maximumTop = monitor.bottom - height;
  const int left = std::clamp(centerX - width / 2, monitor.left, maximumLeft);
  const int top = std::clamp(centerY - height / 2, monitor.top, maximumTop);
  return {left, top, left + width, top + height};
}

enum class OwnerDestroyAction {
  Complete,
  RetryAfterDelay,
  DetachUserDataAndExit,
};

struct OwnerDestroyDecision {
  OwnerDestroyAction action;
  uint32_t delayMilliseconds;
};

enum class OwnerResourceStopAction {
  RetryAfterDelay,
  DestroyVisualsThenInput,
};

struct OwnerResourceStopDecision {
  OwnerResourceStopAction action;
  uint64_t deadlineMilliseconds;
};

inline OwnerResourceStopDecision NextOwnerResourceStopDecision(
    bool resourcesStopped, uint64_t nowMilliseconds) {
  return {resourcesStopped
              ? OwnerResourceStopAction::DestroyVisualsThenInput
              : OwnerResourceStopAction::RetryAfterDelay,
          resourcesStopped ? nowMilliseconds : nowMilliseconds + 25U};
}

inline uint32_t OwnerResourceStopWaitMilliseconds(
    const OwnerResourceStopDecision &decision, uint64_t nowMilliseconds) {
  if (decision.deadlineMilliseconds <= nowMilliseconds) return 0;
  const uint64_t remaining = decision.deadlineMilliseconds - nowMilliseconds;
  return remaining > UINT32_MAX ? UINT32_MAX
                                : static_cast<uint32_t>(remaining);
}

constexpr uint32_t kOwnerDestroyMaximumAttempts = 5;

inline OwnerDestroyDecision NextOwnerDestroyDecision(
    uint32_t attemptsCompleted, bool destroySucceeded,
    bool receivedNcDestroy) {
  if (destroySucceeded || receivedNcDestroy) {
    return {OwnerDestroyAction::Complete, 0};
  }
  if (attemptsCompleted >= kOwnerDestroyMaximumAttempts) {
    return {OwnerDestroyAction::DetachUserDataAndExit, 0};
  }
  const uint32_t shift =
      std::min(attemptsCompleted == 0 ? 0U : attemptsCompleted - 1, 3U);
  return {OwnerDestroyAction::RetryAfterDelay, 25U << shift};
}

inline bool OwnerStopIsObservable(bool stopEventSignaled,
                                  bool windowMessagePosted,
                                  bool threadMessagePosted) {
  return stopEventSignaled || windowMessagePosted || threadMessagePosted;
}

enum class OwnerReadinessAction { Created, Failed, TimedOutSignalStop };

inline OwnerReadinessAction ResolveOwnerReadiness(bool waitSatisfied,
                                                  bool ready,
                                                  bool created) {
  if (!waitSatisfied) return OwnerReadinessAction::TimedOutSignalStop;
  return ready && created ? OwnerReadinessAction::Created
                          : OwnerReadinessAction::Failed;
}

inline bool PetWindowMayShow(bool requestedVisible, bool sleeping,
                             bool inputRegionValid) {
  return requestedVisible && !sleeping && inputRegionValid;
}

inline bool ShouldRestorePresentationAfterResize(bool requestedVisible,
                                                 bool sleeping) {
  return requestedVisible && !sleeping;
}

inline bool ShouldResetPresentationRetryForPositionChange(
    bool positionChanged, bool requestedVisible, bool sleeping) {
  return positionChanged && requestedVisible && !sleeping;
}

inline bool ShouldShowRequestedWindowAfterApply(bool requestedVisible,
                                                bool actuallyVisible) {
  return requestedVisible && !actuallyVisible;
}

inline bool PetWindowNeedsResizeConceal(
    bool actuallyVisible, bool rendererAvailable, uint32_t currentWidth,
    uint32_t currentHeight, uint32_t nextWidth, uint32_t nextHeight) {
  return actuallyVisible && rendererAvailable &&
         (currentWidth != nextWidth || currentHeight != nextHeight);
}

template <typename PositionOperation, typename ResizeOperation,
          typename RegionOperation>
bool TryPositionResizeAndRegion(bool resizeRequired,
                                PositionOperation &&position,
                                ResizeOperation &&resize,
                                RegionOperation &&applyRegion) {
  if (!std::forward<PositionOperation>(position)()) return false;
  if (resizeRequired && !std::forward<ResizeOperation>(resize)()) return false;
  return std::forward<RegionOperation>(applyRegion)();
}

inline bool OwnerExitAfterDestroyAttempt(bool destroySucceeded,
                                         bool receivedNcDestroy) {
  return destroySucceeded || receivedNcDestroy;
}

namespace cylune_window_state {

constexpr double kSafeInset = 16.0;
constexpr double kMinimumSize = 120.0;
constexpr double kMaximumSize = 900.0;

inline bool ValidDisplay(const DisplayInfo &display) {
  return std::isfinite(display.x) && std::isfinite(display.y) &&
         std::isfinite(display.width) && std::isfinite(display.height) &&
         std::isfinite(display.scale) && std::isfinite(display.physicalX) &&
         std::isfinite(display.physicalY) && display.width > 0.0 &&
         display.height > 0.0 && display.scale > 0.0 &&
         std::isfinite(display.x + display.width) &&
         std::isfinite(display.y + display.height) &&
         std::isfinite(display.width * display.scale) &&
         std::isfinite(display.height * display.scale) &&
         std::isfinite(display.physicalX + display.width * display.scale) &&
         std::isfinite(display.physicalY + display.height * display.scale);
}

inline double DistanceToDisplay(LogicalPoint point,
                                const DisplayInfo &display) {
  const double closestX =
      std::clamp(point.x, display.x, display.x + display.width);
  const double closestY =
      std::clamp(point.y, display.y, display.y + display.height);
  const double dx = closestX - point.x;
  const double dy = closestY - point.y;
  return std::hypot(dx, dy);
}

inline double ClampAxis(double value, double origin, double extent,
                        double size) {
  const double minimum = origin + kSafeInset;
  const double maximum = origin + extent - size - kSafeInset;
  if (maximum < minimum) return origin + (extent - size) * 0.5;
  return std::clamp(value, minimum, maximum);
}

} // namespace cylune_window_state

inline PixelRegionBounds PetInputRegionBounds(int physicalSide) {
  if (physicalSide <= 0) return {0, 0, 0, 0};
  const double inset = static_cast<double>(physicalSide) * 0.02;
  const int minimum = static_cast<int>(std::floor(inset));
  const int maximum = static_cast<int>(
      std::ceil(static_cast<double>(physicalSide) - inset));
  return {minimum, minimum, maximum, maximum};
}

inline LogicalPoint LogicalToPhysical(LogicalPoint point,
                                      const DisplayInfo &display) {
  if (!cylune_window_state::ValidDisplay(display) ||
      !std::isfinite(point.x) || !std::isfinite(point.y)) {
    return {display.physicalX, display.physicalY};
  }
  return {display.physicalX + (point.x - display.x) * display.scale,
          display.physicalY + (point.y - display.y) * display.scale};
}

inline LogicalPoint PhysicalToLogical(LogicalPoint point,
                                      const DisplayInfo &display) {
  if (!cylune_window_state::ValidDisplay(display) ||
      !std::isfinite(point.x) || !std::isfinite(point.y)) {
    return {display.x, display.y};
  }
  return {display.x + (point.x - display.physicalX) / display.scale,
          display.y + (point.y - display.physicalY) / display.scale};
}

inline Placement ClampPetOrigin(LogicalPoint origin, double size,
                                const std::vector<DisplayInfo> &displays,
                                uint64_t preferredDisplayId = 0) {
  using namespace cylune_window_state;
  const double safeSize = std::isfinite(size)
                              ? std::clamp(size, kMinimumSize, kMaximumSize)
                              : kMinimumSize;

  const DisplayInfo *selected = nullptr;
  if (preferredDisplayId != 0) {
    for (const DisplayInfo &display : displays) {
      if (display.id == preferredDisplayId && ValidDisplay(display)) {
        selected = &display;
        break;
      }
    }
  }
  if (selected == nullptr && std::isfinite(origin.x) &&
      std::isfinite(origin.y)) {
    double nearestDistance = std::numeric_limits<double>::infinity();
    for (const DisplayInfo &display : displays) {
      if (!ValidDisplay(display)) continue;
      const double distance = DistanceToDisplay(origin, display);
      if (selected == nullptr || distance < nearestDistance) {
        nearestDistance = distance;
        selected = &display;
      }
    }
  } else if (selected == nullptr) {
    for (const DisplayInfo &display : displays) {
      if (ValidDisplay(display)) {
        selected = &display;
        break;
      }
    }
  }

  if (selected == nullptr) return {0, 0.0, 0.0, safeSize};

  const double requestedX = std::isfinite(origin.x)
                                ? origin.x
                                : selected->x + (selected->width - safeSize) * 0.5;
  const double requestedY = std::isfinite(origin.y)
                                ? origin.y
                                : selected->y + (selected->height - safeSize) * 0.5;
  return {selected->id,
          ClampAxis(requestedX, selected->x, selected->width, safeSize),
          ClampAxis(requestedY, selected->y, selected->height, safeSize),
          safeSize};
}

inline PetHit HitTestPet(LogicalPoint point, double side) {
  if (!std::isfinite(point.x) || !std::isfinite(point.y) ||
      !std::isfinite(side) || side <= 0.0) {
    return PetHit::Transparent;
  }
  const double center = side * 0.5;
  const double radius = side * 0.48;
  return std::hypot(point.x - center, point.y - center) <= radius
             ? PetHit::Drag
             : PetHit::Transparent;
}

#endif
