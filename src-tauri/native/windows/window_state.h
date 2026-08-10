#ifndef CYLUNE_WINDOWS_PET_WINDOW_STATE_H
#define CYLUNE_WINDOWS_PET_WINDOW_STATE_H

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <limits>
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

struct PixelRegionBounds {
  int left;
  int top;
  int right;
  int bottom;
};

inline bool OwnerExitAfterDestroyAttempt(bool destroySucceeded,
                                         bool receivedNcDestroy) {
  return destroySucceeded || receivedNcDestroy;
}

namespace cylune_window_state {

constexpr double kSafeInset = 16.0;
constexpr double kMinimumSize = 300.0;
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
