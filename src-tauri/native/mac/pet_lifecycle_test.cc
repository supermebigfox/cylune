#include "pet_lifecycle.h"

#include <assert.h>
#include <initializer_list>
#include <math.h>

static void pet_interaction_rect_corners_stay_inside_circle() {
  for (const double diameter : {120.0, 220.0, 360.0}) {
    const double side = PetInteractionSide(diameter);
    const double cornerDistance = hypot(side / 2.0, side / 2.0);
    assert(side > 0.0);
    assert(cornerDistance < diameter / 2.0);
  }
}

static void pet_visual_and_interaction_windows_share_lifecycle() {
  PetWindowLifecycle lifecycle;
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.interaction_visible());
  assert(!lifecycle.destroyed());

  assert(lifecycle.show());
  assert(lifecycle.visual_visible());
  assert(lifecycle.interaction_visible());
  assert(!lifecycle.show());

  assert(lifecycle.hide());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.interaction_visible());
  assert(!lifecycle.hide());

  assert(lifecycle.show());
  assert(lifecycle.destroy());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.interaction_visible());
  assert(lifecycle.destroyed());
  assert(!lifecycle.show());
  assert(!lifecycle.destroy());
}

int main() {
  pet_interaction_rect_corners_stay_inside_circle();
  pet_visual_and_interaction_windows_share_lifecycle();
}
