#ifndef CYLUNE_PET_INGEST_ANIMATION_H
#define CYLUNE_PET_INGEST_ANIMATION_H

#include <algorithm>
#include <cmath>

constexpr double kPetSwallowDurationSeconds = 0.74;
constexpr double kPetEjectDurationSeconds = 0.62;

inline double PetClampUnit(double value) {
  return std::min(1.0, std::max(0.0, value));
}

inline double PetSwallowProgress(double elapsedSeconds) {
  if (elapsedSeconds <= 0.0) return 0.0;
  if (elapsedSeconds >= kPetSwallowDurationSeconds) return 1.0;
  return PetClampUnit(elapsedSeconds / kPetSwallowDurationSeconds);
}

inline double PetEjectProgress(double elapsedSeconds) {
  if (elapsedSeconds <= kPetSwallowDurationSeconds) return 0.0;
  if (elapsedSeconds >=
      kPetSwallowDurationSeconds + kPetEjectDurationSeconds) {
    return 1.0;
  }
  return PetClampUnit((elapsedSeconds - kPetSwallowDurationSeconds) /
                      kPetEjectDurationSeconds);
}

inline double PetEase(double progress) {
  const double value = PetClampUnit(progress);
  return value * value * (3.0 - 2.0 * value);
}

inline double PetOrbitScale(double progress) {
  return std::pow(1.0 - PetEase(progress), 1.18);
}

inline double PetDropTargetSide(double visualSize) {
  return std::max(144.0, visualSize);
}

inline bool PetShouldDrawDropOverlay(bool fileHovering,
                                     bool ingestAnimationActive) {
  (void)fileHovering;
  return ingestAnimationActive;
}

inline bool PetPointInsideDropTarget(double x, double y, double side) {
  if (!std::isfinite(x) || !std::isfinite(y) || !std::isfinite(side) ||
      side <= 0.0) {
    return false;
  }
  const double radius = side * 0.48;
  const double dx = x - side * 0.5;
  const double dy = y - side * 0.5;
  return std::hypot(dx, dy) <= radius;
}

#endif
