#include "pet_lifecycle.h"

#include <assert.h>
#include <initializer_list>
#include <math.h>

static void event_horizon_circle_fits_inside_core_hit_target() {
  for (const double effectDiameter : {120.0, 220.0, 360.0}) {
    const PetEventHorizonGeometry geometry =
        PetEventHorizonGeometryForEffectDiameter(effectDiameter);
    const double eventHorizonRadius = geometry.event_horizon_diameter / 2.0;
    const double hitTargetHalfSide = geometry.core_hit_target_side / 2.0;
    assert(geometry.event_horizon_diameter > 0.0);
    assert(geometry.event_horizon_diameter ==
           geometry.core_hit_target_side);
    assert(eventHorizonRadius <= hitTargetHalfSide);
  }
}

static void core_hit_target_corners_stay_inside_decorative_effect_circle() {
  for (const double effectDiameter : {120.0, 220.0, 360.0}) {
    const PetEventHorizonGeometry geometry =
        PetEventHorizonGeometryForEffectDiameter(effectDiameter);
    const double hitTargetHalfSide = geometry.core_hit_target_side / 2.0;
    const double cornerDistance = hypot(hitTargetHalfSide, hitTargetHalfSide);
    assert(geometry.decorative_effect_diameter == effectDiameter);
    assert(cornerDistance < geometry.decorative_effect_diameter / 2.0);
  }
}

static void pet_visual_and_core_hit_target_windows_share_lifecycle() {
  PetWindowLifecycle lifecycle;
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(!lifecycle.destroyed());

  assert(lifecycle.show());
  assert(lifecycle.visual_visible());
  assert(lifecycle.core_hit_target_visible());
  assert(!lifecycle.show());

  assert(lifecycle.hide());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(!lifecycle.hide());

  assert(lifecycle.show());
  assert(lifecycle.destroy());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(lifecycle.destroyed());
  assert(!lifecycle.show());
  assert(!lifecycle.destroy());
}

int main() {
  event_horizon_circle_fits_inside_core_hit_target();
  core_hit_target_corners_stay_inside_decorative_effect_circle();
  pet_visual_and_core_hit_target_windows_share_lifecycle();
}
