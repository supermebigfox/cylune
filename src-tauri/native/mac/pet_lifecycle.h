#ifndef BAMBU_POOLS_PET_LIFECYCLE_H
#define BAMBU_POOLS_PET_LIFECYCLE_H

struct PetEventHorizonGeometry {
  double decorative_effect_diameter;
  double event_horizon_diameter;
  double core_hit_target_side;
};

inline PetEventHorizonGeometry PetEventHorizonGeometryForEffectDiameter(
    double effect_diameter) {
  constexpr double kSquareRootOfTwo = 1.4142135623730950488;
  constexpr double kCoreHitTargetInset = 1.0;
  const double core_diameter =
      effect_diameter / kSquareRootOfTwo - (2.0 * kCoreHitTargetInset);
  const double clamped_core_diameter =
      core_diameter > 0.0 ? core_diameter : 0.0;
  return {effect_diameter > 0.0 ? effect_diameter : 0.0,
          clamped_core_diameter, clamped_core_diameter};
}

class PetWindowLifecycle {
 public:
  bool show() {
    if (destroyed_ || (visual_visible_ && core_hit_target_visible_)) {
      return false;
    }
    visual_visible_ = true;
    core_hit_target_visible_ = true;
    return true;
  }

  bool hide() {
    if (!visual_visible_ && !core_hit_target_visible_) {
      return false;
    }
    visual_visible_ = false;
    core_hit_target_visible_ = false;
    return true;
  }

  bool destroy() {
    if (destroyed_) {
      return false;
    }
    visual_visible_ = false;
    core_hit_target_visible_ = false;
    destroyed_ = true;
    return true;
  }

  bool visual_visible() const { return visual_visible_; }
  bool core_hit_target_visible() const { return core_hit_target_visible_; }
  bool destroyed() const { return destroyed_; }

 private:
  bool visual_visible_ = false;
  bool core_hit_target_visible_ = false;
  bool destroyed_ = false;
};

#endif
