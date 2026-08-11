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
  const OwnerDestroyDecision destroyRetry =
      NextOwnerDestroyDecision(1, false, false);
  assert(destroyRetry.action == OwnerDestroyAction::RetryAfterDelay);
  assert(destroyRetry.delayMilliseconds > 0);
  const OwnerDestroyDecision destroyRecovered =
      NextOwnerDestroyDecision(2, true, false);
  assert(destroyRecovered.action == OwnerDestroyAction::Complete);
  const OwnerDestroyDecision destroyPersistent =
      NextOwnerDestroyDecision(kOwnerDestroyMaximumAttempts, false, false);
  assert(destroyPersistent.action ==
         OwnerDestroyAction::DetachUserDataAndExit);
  assert(OwnerStopIsObservable(true, false, false));
  assert(!OwnerStopIsObservable(false, false, false));

  assert(ResolveOwnerReadiness(false, false, false) ==
         OwnerReadinessAction::TimedOutSignalStop);
  assert(ResolveOwnerReadiness(false, true, true) ==
         OwnerReadinessAction::TimedOutSignalStop);
  assert(ResolveOwnerReadiness(true, true, true) ==
         OwnerReadinessAction::Created);
  assert(ResolveOwnerReadiness(true, true, false) ==
         OwnerReadinessAction::Failed);
  assert(PetWindowMayShow(true, false, true));
  assert(!PetWindowMayShow(true, false, false));
  assert(!PetWindowMayShow(true, true, true));
  assert(ShouldRestorePresentationAfterResize(true, false));
  assert(!ShouldRestorePresentationAfterResize(false, false));
  assert(!ShouldRestorePresentationAfterResize(true, true));
  assert(ShouldResetPresentationRetryForPositionChange(true, true, false));
  assert(!ShouldResetPresentationRetryForPositionChange(false, true, false));
  assert(!ShouldResetPresentationRetryForPositionChange(true, false, false));
  assert(!ShouldResetPresentationRetryForPositionChange(true, true, true));

  assert(PetWindowNeedsResizeConceal(true, true, 220, 220, 330, 330));
  assert(!PetWindowNeedsResizeConceal(true, true, 220, 220, 220, 220));
  assert(!PetWindowNeedsResizeConceal(false, true, 220, 220, 330, 330));
  assert(!PetWindowNeedsResizeConceal(true, false, 220, 220, 330, 330));

  int resizeCalls = 0;
  int regionCalls = 0;
  assert(!TryPositionResizeAndRegion(
      true, []() { return true; }, [&resizeCalls]() {
        ++resizeCalls;
        return false;
      },
      [&regionCalls]() {
        ++regionCalls;
        return true;
      }));
  assert(resizeCalls == 1);
  assert(regionCalls == 0);
  assert(TryPositionResizeAndRegion(
      false, []() { return true; }, [&resizeCalls]() {
        ++resizeCalls;
        return false;
      },
      [&regionCalls]() {
        ++regionCalls;
        return true;
      }));
  assert(resizeCalls == 1);
  assert(regionCalls == 1);

  assert(OwnerExitAfterDestroyAttempt(true, false));
  assert(OwnerExitAfterDestroyAttempt(false, true));
  assert(!OwnerExitAfterDestroyAttempt(false, false));

  const PixelRegionBounds inputRegion = PetInputRegionBounds(1200);
  assert(inputRegion.left == 24);
  assert(inputRegion.top == 24);
  assert(inputRegion.right == 1176);
  assert(inputRegion.bottom == 1176);
  assert(inputRegion.right - inputRegion.left == 1152);

  const DisplayInfo left{1, -1920, 0, 1920, 1080, 1.0};
  const DisplayInfo right{2, 0, 0, 3840, 2160, 2.0};
  const std::vector<DisplayInfo> displays{left, right};

  const Placement placed = ClampPetOrigin({3700, 2080}, 600, displays);
  assert(placed.displayId == 2);
  assert(placed.x <= 3840 - 600 - 16);
  assert(placed.y <= 2160 - 600 - 16);
  assert(close_to(placed.size, 600));

  const Placement echoedPosition{2, 100.0, 200.0, 220.0};
  assert(!PlacementPositionChanged(echoedPosition, echoedPosition));
  assert(PlacementPositionChanged(
      echoedPosition, Placement{2, 101.0, 200.0, 220.0}));
  assert(PlacementPositionChanged(
      echoedPosition, Placement{3, 100.0, 200.0, 220.0}));

  assert(HitTestPet({300, 300}, 600) == PetHit::Drag);
  assert(HitTestPet({5, 5}, 600) == PetHit::Transparent);
  assert(HitTestPet({588, 300}, 600) == PetHit::Drag);
  assert(HitTestPet({589, 300}, 600) == PetHit::Transparent);

  const Placement negative = ClampPetOrigin({-3000, 400}, 300, displays);
  assert(negative.displayId == 1);
  assert(close_to(negative.x, -1920 + 16));
  assert(close_to(negative.y, 400));

  const DisplayInfo conversion{3, 0, 0, 1000, 1000, 2.0, 0, 0};
  const LogicalPoint physical = LogicalToPhysical({100, 75}, conversion);
  assert(close_to(physical.x, 200));
  assert(close_to(physical.y, 150));
  const LogicalPoint logical = PhysicalToLogical(physical, conversion);
  assert(close_to(logical.x, 100));
  assert(close_to(logical.y, 75));

  const DisplayInfo primaryMixed{10, 0, 0, 1920, 1080, 1.0, 0, 0};
  const DisplayInfo rightMixed{20, 1920, 0, 1920, 1080, 2.0, 1920, 0};
  const DisplayInfo negativeMixed{30, -3840, 0, 1920, 1080, 2.0,
                                  -3840, 0};
  const std::vector<DisplayInfo> mixedDisplays{primaryMixed, rightMixed};

  const LogicalPoint rightLogical =
      PhysicalToLogical({3000, 400}, rightMixed);
  assert(close_to(rightLogical.x, 2460));
  assert(close_to(rightLogical.y, 200));
  assert(ClampPetOrigin(rightLogical, 300, mixedDisplays).displayId == 20);
  const LogicalPoint rightRoundTrip =
      LogicalToPhysical(rightLogical, rightMixed);
  assert(close_to(rightRoundTrip.x, 3000));
  assert(close_to(rightRoundTrip.y, 400));

  const LogicalPoint primaryFromRight =
      PhysicalToLogical({1800, 400}, primaryMixed);
  assert(close_to(primaryFromRight.x, 1800));
  assert(close_to(LogicalToPhysical(primaryFromRight, primaryMixed).x, 1800));
  assert(ClampPetOrigin(primaryFromRight, 300, mixedDisplays).displayId == 10);

  const LogicalPoint negativeLogical =
      PhysicalToLogical({-2000, 400}, negativeMixed);
  assert(close_to(negativeLogical.x, -2920));
  assert(close_to(negativeLogical.y, 200));
  assert(close_to(LogicalToPhysical(negativeLogical, negativeMixed).x,
                  -2000));
  assert(ClampPetOrigin(negativeLogical, 300,
                        {negativeMixed, primaryMixed, rightMixed})
             .displayId == 30);

  const Placement preferred =
      ClampPetOrigin({1800, 200}, 300, mixedDisplays, 20);
  assert(preferred.displayId == 20);
  assert(close_to(preferred.x, 1936));
  const Placement removed =
      ClampPetOrigin(negativeLogical, 300, mixedDisplays, 30);
  assert(removed.displayId == 10);
  assert(close_to(removed.x, 16));

  assert(close_to(ClampPetOrigin({100, 100}, 119, displays).size, 120));
  assert(close_to(ClampPetOrigin({100, 100}, 120, displays).size, 120));
  assert(close_to(ClampPetOrigin({100, 100}, 220, displays).size, 220));
  assert(close_to(ClampPetOrigin({100, 100}, 299, displays).size, 299));
  assert(close_to(ClampPetOrigin({100, 100}, 300, displays).size, 300));
  const Placement maximum = ClampPetOrigin({100, 100}, 901, displays);
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
  assert(close_to(finite.x, -1920 + (1920 - 120) * 0.5));
  assert(close_to(finite.y, (1080 - 120) * 0.5));
  assert(close_to(finite.size, 120));
  assert(HitTestPet({nan, 10}, 300) == PetHit::Transparent);
  assert(HitTestPet({150, 150}, nan) == PetHit::Transparent);

  const double huge = std::numeric_limits<double>::max();
  for (const LogicalPoint extreme :
       {LogicalPoint{huge, huge}, LogicalPoint{-huge, -huge},
        LogicalPoint{1e307, -1e307}}) {
    const Placement extremePlaced = ClampPetOrigin(extreme, 300, displays);
    assert(extremePlaced.displayId == 1);
    assert(std::isfinite(extremePlaced.x));
    assert(std::isfinite(extremePlaced.y));
    assert(std::isfinite(extremePlaced.size));
    assert(extremePlaced.x >= left.x + 16);
    assert(extremePlaced.x <= left.x + left.width - 300 - 16);
    assert(extremePlaced.y >= left.y + 16);
    assert(extremePlaced.y <= left.y + left.height - 300 - 16);
  }
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
  assert(close_to(empty.size, 120));
}
