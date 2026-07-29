#include "capture_policy.h"

#include <cassert>

int main() {
  const BHCaptureDecision fresh =
      BHDecideCapture(BHCaptureFreshFrame);
  assert(fresh.useScreenTexture);
  assert(!fresh.clearPreviousScreenTexture);
  assert(!fresh.useWallpaperFallback);

  const BHCaptureResult failures[] = {
      BHCapturePermissionDenied,
      BHCaptureUnavailable,
      BHCaptureTransientFailure,
  };
  for (BHCaptureResult failure : failures) {
    const BHCaptureDecision decision = BHDecideCapture(failure);
    assert(!decision.useScreenTexture);
    assert(decision.clearPreviousScreenTexture);
    assert(decision.useWallpaperFallback);
  }
}
