#ifndef CYLUNE_BLACK_HOLE_CAPTURE_POLICY_H
#define CYLUNE_BLACK_HOLE_CAPTURE_POLICY_H

#include <stdbool.h>
#include <stdint.h>

typedef enum {
  BHCaptureFreshFrame = 0,
  BHCapturePermissionDenied = 1,
  BHCaptureUnavailable = 2,
  BHCaptureTransientFailure = 3,
} BHCaptureResult;

typedef struct {
  bool useScreenTexture;
  bool clearPreviousScreenTexture;
  bool useWallpaperFallback;
} BHCaptureDecision;

static inline BHCaptureDecision BHDecideCapture(BHCaptureResult result) {
  BHCaptureDecision decision;
  decision.useScreenTexture = result == BHCaptureFreshFrame;
  decision.clearPreviousScreenTexture = result != BHCaptureFreshFrame;
  decision.useWallpaperFallback = result != BHCaptureFreshFrame;
  return decision;
}

#endif
