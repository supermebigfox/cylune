#ifndef BAMBU_POOLS_PET_LIFECYCLE_H
#define BAMBU_POOLS_PET_LIFECYCLE_H

inline double PetInteractionSide(double diameter) {
  constexpr double kSquareRootOfTwo = 1.4142135623730950488;
  constexpr double kInteractionInset = 1.0;
  const double side =
      diameter / kSquareRootOfTwo - (2.0 * kInteractionInset);
  return side > 0.0 ? side : 0.0;
}

class PetWindowLifecycle {
 public:
  bool show() {
    if (destroyed_ || (visual_visible_ && interaction_visible_)) {
      return false;
    }
    visual_visible_ = true;
    interaction_visible_ = true;
    return true;
  }

  bool hide() {
    if (!visual_visible_ && !interaction_visible_) {
      return false;
    }
    visual_visible_ = false;
    interaction_visible_ = false;
    return true;
  }

  bool destroy() {
    if (destroyed_) {
      return false;
    }
    visual_visible_ = false;
    interaction_visible_ = false;
    destroyed_ = true;
    return true;
  }

  bool visual_visible() const { return visual_visible_; }
  bool interaction_visible() const { return interaction_visible_; }
  bool destroyed() const { return destroyed_; }

 private:
  bool visual_visible_ = false;
  bool interaction_visible_ = false;
  bool destroyed_ = false;
};

#endif
