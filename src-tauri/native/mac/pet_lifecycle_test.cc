#include "pet_drop_state.h"
#include "pet_ingest_animation.h"
#include "pet_lifecycle.h"

#include <cassert>
#include <cmath>

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

  PetDropSession unsupported;
  const uint64_t unsupported_generation =
      unsupported.enter("/tmp/reference.png", PET_FILE_OTHER);
  assert(unsupported_generation != 0);
  assert(unsupported.submit(unsupported_generation, "/tmp/reference.png"));
  assert(unsupported.finish(unsupported_generation, PET_DROP_REJECTED));

  assert(PetDropTargetSide(120.0) == 144.0);
  assert(PetDropTargetSide(300.0) == 300.0);
  assert(PetPointInsideDropTarget(150.0, 150.0, 300.0));
  assert(PetPointInsideDropTarget(292.0, 150.0, 300.0));
  assert(!PetPointInsideDropTarget(299.0, 299.0, 300.0));

  assert(PetSwallowProgress(-1.0) == 0.0);
  assert(PetSwallowProgress(0.0) == 0.0);
  assert(std::abs(PetSwallowProgress(kPetSwallowDurationSeconds * 0.5) -
                  0.5) < 1e-9);
  assert(PetSwallowProgress(kPetSwallowDurationSeconds) == 1.0);
  assert(PetEjectProgress(kPetSwallowDurationSeconds) == 0.0);
  assert(std::abs(PetEjectProgress(kPetSwallowDurationSeconds +
                                  kPetEjectDurationSeconds * 0.5) -
                  0.5) < 1e-9);
  assert(PetEjectProgress(kPetSwallowDurationSeconds +
                          kPetEjectDurationSeconds) == 1.0);
  assert(PetOrbitScale(0.0) == 1.0);
  assert(PetOrbitScale(1.0) == 0.0);
}
