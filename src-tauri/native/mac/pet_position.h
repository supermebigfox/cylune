#ifndef CYLUNE_PET_POSITION_H
#define CYLUNE_PET_POSITION_H

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <limits>

struct PetScreenFrame {
  double x;
  double y;
  double width;
  double height;
  double scale;
  uint32_t display_id;
};

struct PetScreenPoint {
  double x;
  double y;
};

inline bool PetDisplayContainsPoint(PetScreenFrame display,
                                    PetScreenPoint point) {
  return std::isfinite(point.x) && std::isfinite(point.y) &&
         point.x >= display.x && point.x <= display.x + display.width &&
         point.y >= display.y && point.y <= display.y + display.height;
}

inline size_t PetDisplayIndexForPoint(PetScreenPoint point,
                                      const PetScreenFrame *displays,
                                      size_t count, size_t current_index) {
  if (displays == nullptr || count == 0) return 0;
  if (current_index < count &&
      PetDisplayContainsPoint(displays[current_index], point)) {
    return current_index;
  }
  for (size_t index = 0; index < count; ++index) {
    if (PetDisplayContainsPoint(displays[index], point)) return index;
  }
  return current_index < count ? current_index : 0;
}

inline size_t PetActivePaneCount(PetScreenPoint point,
                                 const PetScreenFrame *displays,
                                 size_t count) {
  if (displays == nullptr) return 0;
  for (size_t index = 0; index < count; ++index) {
    if (PetDisplayContainsPoint(displays[index], point)) return 1;
  }
  return count == 0 ? 0 : 1;
}

inline PetScreenPoint PetClampPointToDisplays(PetScreenPoint point,
                                              const PetScreenFrame *displays,
                                              size_t count) {
  if (displays == nullptr || count == 0) return point;
  for (size_t index = 0; index < count; ++index) {
    if (PetDisplayContainsPoint(displays[index], point)) return point;
  }
  PetScreenPoint nearest = point;
  double nearestDistance = std::numeric_limits<double>::infinity();
  for (size_t index = 0; index < count; ++index) {
    const PetScreenFrame display = displays[index];
    const PetScreenPoint candidate = {
        std::clamp(point.x, display.x, display.x + display.width),
        std::clamp(point.y, display.y, display.y + display.height),
    };
    const double dx = candidate.x - point.x;
    const double dy = candidate.y - point.y;
    const double distance = dx * dx + dy * dy;
    if (distance < nearestDistance) {
      nearest = candidate;
      nearestDistance = distance;
    }
  }
  return nearest;
}

inline PetScreenPoint PetRecoverCenter(PetScreenPoint center,
                                       const PetScreenFrame *displays,
                                       size_t count) {
  if (displays == nullptr || count == 0) return center;
  for (size_t index = 0; index < count; ++index) {
    if (PetDisplayContainsPoint(displays[index], center)) return center;
  }
  return {displays[0].x + displays[0].width * 0.5,
          displays[0].y + displays[0].height * 0.5};
}

inline PetScreenPoint PetPrimaryDisplayCenter(const PetScreenFrame *displays,
                                              size_t count) {
  if (displays == nullptr || count == 0) return {0.0, 0.0};
  return {displays[0].x + displays[0].width * 0.5,
          displays[0].y + displays[0].height * 0.5};
}

class PetFixedPosition {
 public:
  explicit PetFixedPosition(PetScreenPoint point) : point_(point) {}
  void observeElapsedTime(double) {}
  void moveTo(PetScreenPoint point) { point_ = point; }
  PetScreenPoint point() const { return point_; }

 private:
  PetScreenPoint point_;
};

#endif
