#ifndef CYLUNE_WINDOWS_ANIMATION_H
#define CYLUNE_WINDOWS_ANIMATION_H

#include <algorithm>
#include <cmath>

constexpr double kSwallowDurationSeconds = 0.74;
constexpr double kEjectDurationSeconds = 0.62;
constexpr double kSuccessJetDurationSeconds = 0.50;

inline double ClampUnit(double value) {
  return std::min(1.0, std::max(0.0, value));
}

inline double SwallowProgress(double elapsedSeconds) {
  if (elapsedSeconds <= 0.0) return 0.0;
  if (elapsedSeconds >= kSwallowDurationSeconds) return 1.0;
  return ClampUnit(elapsedSeconds / kSwallowDurationSeconds);
}

inline double EjectProgress(double elapsedSeconds) {
  if (elapsedSeconds <= kSwallowDurationSeconds) return 0.0;
  if (elapsedSeconds >= kSwallowDurationSeconds + kEjectDurationSeconds) {
    return 1.0;
  }
  return ClampUnit((elapsedSeconds - kSwallowDurationSeconds) /
                   kEjectDurationSeconds);
}

inline double SuccessJetProgress(double elapsedSeconds) {
  if (elapsedSeconds <= kSwallowDurationSeconds) return 0.0;
  if (elapsedSeconds >=
      kSwallowDurationSeconds + kSuccessJetDurationSeconds) {
    return 1.0;
  }
  return ClampUnit((elapsedSeconds - kSwallowDurationSeconds) /
                   kSuccessJetDurationSeconds);
}

inline double SmoothstepEase(double progress) {
  const double value = ClampUnit(progress);
  return value * value * (3.0 - 2.0 * value);
}

inline double OrbitScale(double progress) {
  return std::pow(1.0 - SmoothstepEase(progress), 1.18);
}

struct HoverUniforms {
  float rotationRate = 1.0f;
  float pullGain = 1.0f;
  float visualDiameterScale = 1.0f;
};

inline HoverUniforms HoverEffect(double progress) {
  const float value = static_cast<float>(ClampUnit(progress));
  return {1.0f + 1.4f * value, 1.0f + 0.7f * value, 1.0f};
}

inline double HoverVisualDiameter(double visualDiameter,
                                  HoverUniforms effect) {
  (void)effect;
  return visualDiameter;
}

enum class AnimationState {
  Idle,
  Hover,
  Swallow,
  Eject,
  SuccessJet,
};

struct AnimationUniforms {
  double ingestProgress = 0.0;
  double ejectProgress = 0.0;
  double successJetProgress = 0.0;
  double orbitScale = 1.0;
  HoverUniforms hover{};
};

inline AnimationUniforms ResolveAnimation(AnimationState state,
                                          double elapsedSeconds) {
  AnimationUniforms uniforms{};
  switch (state) {
    case AnimationState::Hover:
      uniforms.hover = HoverEffect(elapsedSeconds);
      break;
    case AnimationState::Swallow:
      uniforms.ingestProgress = SwallowProgress(elapsedSeconds);
      uniforms.orbitScale = OrbitScale(uniforms.ingestProgress);
      break;
    case AnimationState::Eject:
      uniforms.ingestProgress = SwallowProgress(elapsedSeconds);
      uniforms.ejectProgress = EjectProgress(elapsedSeconds);
      break;
    case AnimationState::SuccessJet:
      uniforms.ingestProgress = SwallowProgress(elapsedSeconds);
      uniforms.successJetProgress = SuccessJetProgress(elapsedSeconds);
      break;
    case AnimationState::Idle:
      break;
  }
  return uniforms;
}

#endif
