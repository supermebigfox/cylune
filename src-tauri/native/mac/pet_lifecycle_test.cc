#include "pet_lifecycle.h"

#include <assert.h>

static void pet_monitor_lifecycle_show_hide_destroy() {
  PetMonitorLifecycle lifecycle;
  assert(!lifecycle.monitor_active());
  assert(!lifecycle.destroyed());

  assert(lifecycle.show());
  assert(lifecycle.monitor_active());
  assert(!lifecycle.show());

  assert(lifecycle.hide());
  assert(!lifecycle.monitor_active());
  assert(!lifecycle.hide());

  assert(lifecycle.show());
  assert(lifecycle.monitor_active());
  assert(lifecycle.destroy());
  assert(!lifecycle.monitor_active());
  assert(lifecycle.destroyed());
  assert(!lifecycle.show());
  assert(!lifecycle.destroy());
}

int main() { pet_monitor_lifecycle_show_hide_destroy(); }
