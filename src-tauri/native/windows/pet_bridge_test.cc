#include "bridge.h"

#include <cstddef>

static_assert(sizeof(PetConfig) == 64);
static_assert(offsetof(PetConfig, display_id) == 40);
static_assert(offsetof(PetConfig, visual_style) == 62);

int main() { return pet_abi_version() == 1 ? 0 : 1; }
