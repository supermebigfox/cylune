#include "pet_position.h"

#include <cassert>
#include <cmath>

static bool close_to(double lhs, double rhs) {
  return std::abs(lhs - rhs) < 1e-6;
}

int main() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1440.0, 900.0, 2.0, 10},
      {-1920.0, 0.0, 1920.0, 1080.0, 1.0, 20},
  };

  assert(PetDisplayIndexForPoint({720.0, 450.0}, displays, 2, 1) == 0);
  assert(PetDisplayIndexForPoint({-960.0, 540.0}, displays, 2, 0) == 1);
  assert(PetActivePaneCount({-960.0, 540.0}, displays, 2) == 1);

  const PetScreenPoint crossing =
      PetClampPointToDisplays({-100.0, 450.0}, displays, 2);
  assert(close_to(crossing.x, -100.0));
  assert(close_to(crossing.y, 450.0));

  const PetScreenPoint outside =
      PetClampPointToDisplays({3000.0, 3000.0}, displays, 2);
  assert(close_to(outside.x, 1440.0));
  assert(close_to(outside.y, 900.0));

  const PetScreenPoint reset = PetPrimaryDisplayCenter(displays, 2);
  assert(close_to(reset.x, 720.0));
  assert(close_to(reset.y, 450.0));

  PetFixedPosition fixed({300.0, 400.0});
  fixed.observeElapsedTime(1.0);
  fixed.observeElapsedTime(1000.0);
  assert(close_to(fixed.point().x, 300.0));
  assert(close_to(fixed.point().y, 400.0));
  fixed.moveTo({-500.0, 600.0});
  assert(close_to(fixed.point().x, -500.0));
  assert(close_to(fixed.point().y, 600.0));
}
