#ifndef CYLUNE_WINDOWS_PET_BRIDGE_H
#define CYLUNE_WINDOWS_PET_BRIDGE_H

#include <stdint.h>

#ifndef __cplusplus
#include <stdbool.h>
#endif

#ifdef __cplusplus
extern "C" {
#endif

typedef void (*PetCallback)(uint32_t kind, const char *payload, double x,
                            double y, uint64_t event_value);

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
  uint8_t visual_style;
  uint8_t _reserved;
} PetConfig;

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
  PET_SHUTDOWN_COMPLETE = 0,
  PET_SHUTDOWN_STOP_FAILED = 1,
  PET_SHUTDOWN_STOP_TIMED_OUT = 2,
};

enum {
  PET_DROP_ACCEPTED = 1,
  PET_DROP_REJECTED = 2,
};

void *pet_create(PetCallback callback, const char *hlsl_source);
uint32_t pet_destroy(void *handle);
bool pet_apply(void *handle, PetConfig config);
void pet_show(void *handle);
void pet_hide(void *handle);
void pet_reset(void *handle);
void pet_signal(void *handle, uint32_t signal);
void pet_finish_drop(void *handle, uint64_t generation, uint32_t result);
uint32_t pet_capture_state(void *handle);
uint32_t pet_renderer_state(void *handle);
uint32_t pet_abi_version(void);

#ifdef __cplusplus
}
#endif

#endif
