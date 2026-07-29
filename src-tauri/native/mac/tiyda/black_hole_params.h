#ifndef CYLUNE_BLACK_HOLE_PARAMS_H
#define CYLUNE_BLACK_HOLE_PARAMS_H

#include <stdint.h>

typedef struct {
  float centerX;
  float centerY;
  float size;
  uint32_t fpsMode;
  uint32_t cyluneStyle;
} BHHostSettings;

typedef struct {
  float centerX;
  float centerY;
  float size;
  uint32_t framesPerSecond;
  uint32_t upstreamStyle;
} BHResolvedSettings;

static inline BHResolvedSettings BHResolveSettings(
    BHHostSettings input, uint32_t displayRefreshRate) {
  const uint32_t automaticFps = displayRefreshRate < 30
                                    ? 30
                                    : (displayRefreshRate > 120
                                           ? 120
                                           : displayRefreshRate);
  const uint32_t framesPerSecond =
      input.fpsMode == 30 || input.fpsMode == 60 ? input.fpsMode : automaticFps;
  const float resolvedSize =
      input.size < 300.0f ? 300.0f : (input.size > 900.0f ? 900.0f : input.size);
  BHResolvedSettings resolved;
  resolved.centerX = input.centerX;
  resolved.centerY = input.centerY;
  resolved.size = resolvedSize;
  resolved.framesPerSecond = framesPerSecond;
  // CYLUNE persists Gargantua as 0 and Fusion as 1. The upstream renderer
  // uses 1 for Gargantua and 0 for its Default/Fusion presentation.
  resolved.upstreamStyle = input.cyluneStyle == 0 ? 1u : 0u;
  return resolved;
}

static inline float BHShaderSizeForPixels(float visualDiameterPixels,
                                         float drawableHeightPixels) {
  if (drawableHeightPixels <= 0.0f) {
    return 0.0f;
  }
  // The upstream shader fades to transparent at 4.2 * rh, with
  // rh = 0.125 * size. Its full visible diameter is therefore
  // 2 * 4.2 * 0.125 = 1.05 times the drawable height.
  return visualDiameterPixels / (1.05f * drawableHeightPixels);
}

#endif
