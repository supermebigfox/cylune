#include "window_state.h"

#include <cassert>
#include <cmath>
#include <limits>
#include <vector>

namespace {

bool close_to(double lhs, double rhs) {
  return std::abs(lhs - rhs) < 1e-6;
}

} // namespace

int main() {
  const DisplayInfo left{1, -1920, 0, 1920, 1080, 1.0};
  const DisplayInfo right{2, 0, 0, 3840, 2160, 2.0};
  const std::vector<DisplayInfo> displays{left, right};

  const Placement placed = ClampPetOrigin({3700, 2080}, 600, displays);
  assert(placed.displayId == 2);
  assert(placed.x <= 3840 - 600 - 16);
  assert(placed.y <= 2160 - 600 - 16);
  assert(close_to(placed.size, 600));

  assert(HitTestPet({300, 300}, 600) == PetHit::Drag);
  assert(HitTestPet({5, 5}, 600) == PetHit::Transparent);
  assert(HitTestPet({588, 300}, 600) == PetHit::Drag);
  assert(HitTestPet({589, 300}, 600) == PetHit::Transparent);

  const Placement negative = ClampPetOrigin({-3000, 400}, 300, displays);
  assert(negative.displayId == 1);
  assert(close_to(negative.x, -1920 + 16));
  assert(close_to(negative.y, 400));

  const LogicalPoint physical = LogicalToPhysical({100, 75}, 2.0);
  assert(close_to(physical.x, 200));
  assert(close_to(physical.y, 150));
  const LogicalPoint logical = PhysicalToLogical(physical, 2.0);
  assert(close_to(logical.x, 100));
  assert(close_to(logical.y, 75));

  const Placement minimum = ClampPetOrigin({100, 100}, 120, displays);
  assert(close_to(minimum.size, 300));
  const Placement maximum = ClampPetOrigin({100, 100}, 1200, displays);
  assert(close_to(maximum.size, 900));

  const Placement recovered = ClampPetOrigin(
      {-960, 900}, 400, std::vector<DisplayInfo>{{2, 0, 0, 3840, 2160, 2.0}});
  assert(recovered.displayId == 2);
  assert(close_to(recovered.x, 16));
  assert(std::isfinite(recovered.x));
  assert(std::isfinite(recovered.y));

  const double nan = std::numeric_limits<double>::quiet_NaN();
  const Placement finite = ClampPetOrigin({nan, nan}, nan, displays);
  assert(finite.displayId == 1);
  assert(close_to(finite.x, -1920 + (1920 - 300) * 0.5));
  assert(close_to(finite.y, (1080 - 300) * 0.5));
  assert(close_to(finite.size, 300));
  assert(HitTestPet({nan, 10}, 300) == PetHit::Transparent);
  assert(HitTestPet({150, 150}, nan) == PetHit::Transparent);

  const double huge = std::numeric_limits<double>::max();
  const Placement overflowSafe = ClampPetOrigin(
      {nan, nan}, 300,
      {{99, huge, huge, huge, huge, 1.0}, {2, 0, 0, 3840, 2160, 2.0}});
  assert(overflowSafe.displayId == 2);
  assert(std::isfinite(overflowSafe.x));
  assert(std::isfinite(overflowSafe.y));

  const Placement empty = ClampPetOrigin({nan, nan}, nan, {});
  assert(empty.displayId == 0);
  assert(close_to(empty.x, 0));
  assert(close_to(empty.y, 0));
  assert(close_to(empty.size, 300));
}
