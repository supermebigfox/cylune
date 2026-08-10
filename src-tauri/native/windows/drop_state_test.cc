#include "drop_state.h"

#include <cassert>
#include <cstdint>

int main() {
  assert(ResolveDropEffect(0, true) == 0);  // NONE
  assert(ResolveDropEffect(2, true) == 0);  // MOVE only
  assert(ResolveDropEffect(4, true) == 0);  // LINK only
  assert(ResolveDropEffect(1, true) == 1);  // COPY
  assert(ResolveDropEffect(3, true) == 1);  // COPY | MOVE chooses COPY
  assert(ResolveDropEffect(1, false) == 0); // target rejected

  DropSession state;
  const uint64_t generation =
      state.enter(L"C:\\prints\\mask.3mf", FileKind::ThreeMf);
  assert(generation != 0);
  assert(state.hovering());
  assert(!state.waitingForAck());
  assert(state.submit(generation, L"C:\\prints\\mask.3mf"));
  assert(!state.hovering());
  assert(state.waitingForAck());

  // A second drag cannot replace a file whose business acknowledgement is
  // still outstanding.
  assert(state.enter(L"C:\\prints\\second.3mf", FileKind::ThreeMf) == 0);
  assert(!state.finish(generation + 1, PET_DROP_ACCEPTED));
  assert(state.waitingForAck());
  assert(state.generation() == generation);
  assert(state.finish(generation, PET_DROP_ACCEPTED));
  assert(!state.waitingForAck());
  assert(state.generation() == 0);

  const uint64_t shutdownGeneration =
      state.enter(L"C:\\prints\\shutdown.3mf", FileKind::ThreeMf);
  assert(state.submit(shutdownGeneration, L"C:\\prints\\shutdown.3mf"));
  state.deactivate();
  assert(!state.hovering());
  assert(!state.waitingForAck());
  assert(state.generation() == 0);

  // Native ingestion accepts every ordinary file. Rust owns support/type
  // policy and may acknowledge an otherwise ordinary file as rejected.
  const uint64_t otherGeneration =
      state.enter(L"C:\\prints\\notes.txt", FileKind::Other);
  assert(otherGeneration != 0);
  assert(state.submit(otherGeneration, L"C:\\prints\\notes.txt"));
  assert(state.finish(otherGeneration, PET_DROP_REJECTED));
  assert(!state.waitingForAck());

  // DragLeave cancels only hover state. It neither submits the file nor
  // manufactures a completion animation/result.
  const uint64_t leftGeneration =
      state.enter(L"C:\\prints\\left.gcode", FileKind::GCode);
  assert(leftGeneration != 0);
  assert(state.leave());
  assert(!state.hovering());
  assert(!state.waitingForAck());
  assert(state.generation() == 0);
  assert(!state.finish(leftGeneration, PET_DROP_REJECTED));

  // Mere pet-window movement has no DropSession input and therefore cannot
  // create a generation; only enter() above does so.
  DropSession movedWindow;
  assert(!movedWindow.hovering());
  assert(!movedWindow.waitingForAck());
  assert(movedWindow.generation() == 0);

  // The OLE pointer contract uses an exact radius of side * 0.48.
  assert(PointerInsideDropTarget(300.0, 300.0, 600.0));
  assert(PointerInsideDropTarget(588.0, 300.0, 600.0));
  assert(!PointerInsideDropTarget(588.01, 300.0, 600.0));
  assert(!PointerInsideDropTarget(300.0, 300.0, 0.0));

  const DropVisualActivity hover = ResolveDropVisualActivity(
      0, 600.0, PetDropVisualState::Hover);
  assert(hover.targetFps == 60);
  assert(hover.visualSize == 600.0);
  const DropVisualActivity idle = ResolveDropVisualActivity(
      0, 600.0, PetDropVisualState::Idle);
  assert(idle.targetFps == 30);
  assert(idle.visualSize == 600.0);
  const DropVisualActivity rejected = ResolveDropVisualActivity(
      30, 900.0, PetDropVisualState::SwallowAndEject);
  assert(rejected.targetFps == 60);
  assert(rejected.visualSize == 900.0);
}
