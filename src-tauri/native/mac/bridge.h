#ifndef BAMBU_POOLS_PET_BRIDGE_H
#define BAMBU_POOLS_PET_BRIDGE_H

#include <stdint.h>

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
} PetConfig;

void *pet_create(PetCallback callback, const char *metal_source);
void pet_destroy(void *handle);
bool pet_apply(void *handle, PetConfig config);
void pet_show(void *handle);
void pet_hide(void *handle);
void pet_reset(void *handle);
void pet_signal(void *handle, uint32_t signal);
uint32_t pet_abi_version(void);

#ifdef __cplusplus
}
#endif

#endif
