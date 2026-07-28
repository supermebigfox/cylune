#ifndef BAMBU_POOLS_PET_BRIDGE_H
#define BAMBU_POOLS_PET_BRIDGE_H

#include <stdint.h>

#ifdef __APPLE__
#include <IOSurface/IOSurfaceRef.h>
#else
typedef void *IOSurfaceRef;
#endif

#ifndef __cplusplus
#include <stdbool.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*PetCallback)(uint32_t kind, const char *payload,
                            double x, double y, uint64_t display_id);

typedef struct {
  uint32_t abi_version;
  uint32_t mode;
  uint32_t effective_mode;
  uint8_t has_position;
  double size;
  double x;
  double y;
  uint64_t display_id;
  uint32_t fps;
  uint8_t visible;
  uint32_t pending_count;
  uint8_t reduce_motion;
  uint8_t request_permission;
} PetConfig;

typedef struct {
  uint32_t display_id;
  double source_x;
  double source_y;
  double source_width;
  double source_height;
  uint32_t pixel_width;
  uint32_t pixel_height;
  float panel_origin_uv[2];
  float panel_extent_uv[2];
} PetCaptureRegion;

typedef struct {
  float viewport_px[2];
  float time_seconds;
  float lens_strength;
  float hover_progress;
  float swallow_progress;
  float success_progress;
  float error_progress;
  uint32_t pending_count;
  uint32_t mode;
  uint32_t reduce_motion;
  uint32_t _padding;
  float capture_origin_uv[2];
  float capture_extent_uv[2];
} PetRenderUniforms;

typedef struct {
  uint32_t base_draw_calls;
  uint32_t pending_draw_calls;
  uint32_t pending_instances;
  uint32_t fragment_pending_iterations;
} PetRenderStats;

enum {
  PET_CAPTURE_UNAVAILABLE = 0,
  PET_CAPTURE_NOT_DETERMINED = 1,
  PET_CAPTURE_DENIED = 2,
  PET_CAPTURE_RESTART_REQUIRED = 3,
  PET_CAPTURE_READY = 4,
  PET_CAPTURE_FAILED = 5,
};

enum {
  PET_RENDERER_UNAVAILABLE = 0,
  PET_RENDERER_READY = 1,
};

enum {
  PET_RENDER_DRAW_OK = 0,
  PET_RENDER_DRAW_TRANSIENT = 1,
  PET_RENDER_DRAW_FATAL = 2,
};

enum {
  PET_SHUTDOWN_COMPLETE = 0,
  PET_SHUTDOWN_STOP_FAILED = 1,
  PET_SHUTDOWN_STOP_TIMED_OUT = 2,
};

void *pet_create(PetCallback callback, const char *metal_source);
uint32_t pet_destroy(void *handle);
bool pet_apply(void *handle, PetConfig config);
void pet_show(void *handle);
void pet_hide(void *handle);
void pet_reset(void *handle);
void pet_signal(void *handle, uint32_t signal);
uint32_t pet_capture_state(void *handle);
uint32_t pet_renderer_state(void *handle);
uint32_t pet_abi_version(void);

void *mac_capture_create(PetCallback callback);
uint32_t mac_capture_destroy(void *handle);
void mac_capture_configure(void *handle, PetCaptureRegion region,
                           bool real_mode, bool visible,
                           bool request_permission, uint32_t fps);
void mac_capture_stop(void *handle);
uint32_t mac_capture_state(void *handle);
IOSurfaceRef mac_capture_copy_latest_surface(void *handle);

void *mac_renderer_create(const char *metal_source, void *metal_layer);
void mac_renderer_destroy(void *handle);
uint32_t mac_renderer_draw(void *handle, IOSurfaceRef surface,
                           PetRenderUniforms uniforms);
uint64_t pet_test_render_rgba(const uint8_t *input, uint32_t width,
                              uint32_t height, PetRenderUniforms uniforms,
                              uint8_t *output, uint64_t output_capacity,
                              PetRenderStats *stats,
                              const char *metal_source);

#ifdef __cplusplus
}
#endif

#endif
