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
};

struct Placement {
  uint64_t displayId;
  double x;
  double y;
  double size;
};

namespace cylune_window_state {

constexpr double kSafeInset = 16.0;
constexpr double kMinimumSize = 300.0;
constexpr double kMaximumSize = 900.0;

inline bool ValidDisplay(const DisplayInfo &display) {
  return std::isfinite(display.x) && std::isfinite(display.y) &&
         std::isfinite(display.width) && std::isfinite(display.height) &&
         std::isfinite(display.scale) && display.width > 0.0 &&
         display.height > 0.0 && display.scale > 0.0 &&
         std::isfinite(display.x + display.width) &&
         std::isfinite(display.y + display.height);
}

inline double DistanceSquaredToDisplay(LogicalPoint point,
                                       const DisplayInfo &display) {
  const double closestX =
      std::clamp(point.x, display.x, display.x + display.width);
  const double closestY =
      std::clamp(point.y, display.y, display.y + display.height);
  const double dx = closestX - point.x;
  const double dy = closestY - point.y;
  return dx * dx + dy * dy;
}

inline double ClampAxis(double value, double origin, double extent,
                        double size) {
  const double minimum = origin + kSafeInset;
  const double maximum = origin + extent - size - kSafeInset;
  if (maximum < minimum) return origin + (extent - size) * 0.5;
  return std::clamp(value, minimum, maximum);
}

} // namespace cylune_window_state

inline LogicalPoint LogicalToPhysical(LogicalPoint point, double scale) {
  const double safeScale =
      std::isfinite(scale) && scale > 0.0 ? scale : 1.0;
  return {std::isfinite(point.x) ? point.x * safeScale : 0.0,
          std::isfinite(point.y) ? point.y * safeScale : 0.0};
}

inline LogicalPoint PhysicalToLogical(LogicalPoint point, double scale) {
  const double safeScale =
      std::isfinite(scale) && scale > 0.0 ? scale : 1.0;
  return {std::isfinite(point.x) ? point.x / safeScale : 0.0,
          std::isfinite(point.y) ? point.y / safeScale : 0.0};
}

inline Placement ClampPetOrigin(LogicalPoint origin, double size,
                                const std::vector<DisplayInfo> &displays) {
  using namespace cylune_window_state;
  const double safeSize = std::isfinite(size)
                              ? std::clamp(size, kMinimumSize, kMaximumSize)
                              : kMinimumSize;

  const DisplayInfo *selected = nullptr;
  if (std::isfinite(origin.x) && std::isfinite(origin.y)) {
    double nearestDistance = std::numeric_limits<double>::infinity();
    for (const DisplayInfo &display : displays) {
      if (!ValidDisplay(display)) continue;
      const double distance = DistanceSquaredToDisplay(origin, display);
      if (distance < nearestDistance) {
        nearestDistance = distance;
        selected = &display;
      }
    }
  } else {
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
