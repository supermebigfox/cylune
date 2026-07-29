#include "pet_drop_state.h"
#include "pet_lifecycle.h"

#include <cassert>

int main() {
  PetWindowLifecycle lifecycle;
  lifecycle.show();
  assert(lifecycle.visible());
  lifecycle.sleep();
  assert(lifecycle.sleeping());
  lifecycle.wake();
  lifecycle.hide();
  assert(!lifecycle.visible());
  lifecycle.destroy();
  lifecycle.show();
  assert(lifecycle.destroyed());
  assert(!lifecycle.visible());

  PetApplyGenerationGate gate;
  const uint64_t first = gate.issue();
  const uint64_t second = gate.issue();
  assert(gate.accept(second));
  assert(!gate.accept(first));

  PetDropSession drop;
  const uint64_t generation = drop.enter("/tmp/model.gcode.3mf", PET_FILE_3MF);
  assert(generation != 0);
  assert(drop.submit(generation, "/tmp/model.gcode.3mf"));
  assert(drop.waitingForAck());
  assert(drop.enter("/tmp/other.gcode", PET_FILE_GCODE) == 0);
  assert(!drop.finish(generation + 1, PET_DROP_ACCEPTED));
  assert(drop.finish(generation, PET_DROP_ACCEPTED));
  assert(!drop.waitingForAck());
  assert(drop.enter(nullptr, PET_FILE_3MF) == 0);
}
