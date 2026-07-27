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
  double size;
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
} PetCaptureRegion;

enum {
  PET_CAPTURE_UNAVAILABLE = 0,
  PET_CAPTURE_NOT_DETERMINED = 1,
  PET_CAPTURE_DENIED = 2,
  PET_CAPTURE_RESTART_REQUIRED = 3,
  PET_CAPTURE_READY = 4,
  PET_CAPTURE_FAILED = 5,
};

void *pet_create(PetCallback callback, const char *metal_source);
void pet_destroy(void *handle);
bool pet_apply(void *handle, PetConfig config);
void pet_show(void *handle);
void pet_hide(void *handle);
void pet_reset(void *handle);
void pet_signal(void *handle, uint32_t signal);
uint32_t pet_capture_state(void *handle);
uint32_t pet_abi_version(void);

void *mac_capture_create(PetCallback callback);
void mac_capture_destroy(void *handle);
void mac_capture_configure(void *handle, PetCaptureRegion region,
                           bool real_mode, bool visible,
                           bool request_permission, uint32_t fps);
void mac_capture_stop(void *handle);
uint32_t mac_capture_state(void *handle);
IOSurfaceRef mac_capture_copy_latest_surface(void *handle);

#ifdef __cplusplus
}
#endif

#endif
