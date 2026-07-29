#include "black_hole_params.h"

#include <cassert>
#include <cmath>

int main() {
  const BHResolvedSettings gargantua =
      BHResolveSettings({0.25f, 0.75f, 420.0f, 30, 0}, 60);
  assert(gargantua.upstreamStyle == 1);
  assert(gargantua.framesPerSecond == 30);
  assert(gargantua.centerX == 0.25f);
  assert(gargantua.centerY == 0.75f);

  const BHResolvedSettings fusion =
      BHResolveSettings({0.5f, 0.5f, 640.0f, 60, 1}, 120);
  assert(fusion.upstreamStyle == 0);
  assert(fusion.framesPerSecond == 60);

  assert(BHResolveSettings({0.5f, 0.5f, 120.0f, 0, 0}, 24).size ==
         300.0f);
  assert(BHResolveSettings({0.5f, 0.5f, 1200.0f, 0, 0}, 144).size ==
         900.0f);
  assert(BHResolveSettings({0.5f, 0.5f, 500.0f, 0, 0}, 24)
             .framesPerSecond == 30);
  assert(BHResolveSettings({0.5f, 0.5f, 500.0f, 0, 0}, 75)
             .framesPerSecond == 75);
  assert(BHResolveSettings({0.5f, 0.5f, 500.0f, 0, 0}, 144)
             .framesPerSecond == 120);
  assert(BHShaderSizeForPixels(630.0f, 1000.0f) == 0.6f);
  assert(BHShaderSizeForPixels(630.0f, 0.0f) == 0.0f);

  const BHHoverEffect idle = BHResolveHoverEffect(0.0f);
  const BHHoverEffect nearbyFile = BHResolveHoverEffect(1.0f);
  assert(idle.rotationRate == 1.0f);
  assert(idle.pullGain == 1.0f);
  assert(nearbyFile.rotationRate == 2.4f);
  assert(nearbyFile.pullGain == 1.7f);
  assert(nearbyFile.rotationRate > idle.rotationRate);
  assert(nearbyFile.pullGain > idle.pullGain);
  assert(BHHoverVisualDiameter(300.0f, nearbyFile) == 300.0f);
  const double beforePause =
      BHAdvanceAnimationTime(4.0, 0.5, idle.rotationRate);
  const double afterResume =
      BHAdvanceAnimationTime(beforePause, 0.0, nearbyFile.rotationRate);
  assert(std::abs(beforePause - 4.1) < 1e-9);
  assert(std::abs(afterResume - beforePause) < 1e-9);
  assert(std::abs(BHAdvanceAnimationTime(4.0, 0.5,
                                         nearbyFile.rotationRate) -
                  4.24) < 1e-7);

  const BHHostSettings fixed = {0.33f, 0.61f, 500.0f, 0, 1};
  for (unsigned refresh = 30; refresh <= 120; refresh += 30) {
    const BHResolvedSettings resolved = BHResolveSettings(fixed, refresh);
    assert(resolved.centerX == fixed.centerX);
    assert(resolved.centerY == fixed.centerY);
  }
}
