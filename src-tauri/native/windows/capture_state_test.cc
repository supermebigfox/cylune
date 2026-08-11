#include "capture_state.h"

#include <cassert>

void frame_generation_and_timeout() {
  CaptureMachine machine;
  assert(machine.reduce(CaptureEvent::Start).action == CaptureAction::CreateDuplication);
  const uint64_t first = machine.generation();
  assert(machine.reduceFrameReady(first).action == CaptureAction::PublishFrame);
  assert(machine.reduce(CaptureEvent::Timeout).action == CaptureAction::ClearFrame);
  assert(!machine.hasCurrentFrame());
  assert(machine.reduce(CaptureEvent::AccessLost).action == CaptureAction::RecreateDuplication);
  assert(machine.phase() == CapturePhase::Recovering);
  assert(machine.reduce(CaptureEvent::AccessLost).action == CaptureAction::None);
  assert(machine.reduce(CaptureEvent::RecoveryReady).action == CaptureAction::CreateDuplication);
  assert(machine.reduceFrameReady(first).action == CaptureAction::ClearFrame);
}

void lifecycle_is_terminal_and_idempotent() {
  CaptureMachine machine;
  (void)machine.reduce(CaptureEvent::Start);
  const uint64_t running = machine.generation();
  assert(machine.reduce(CaptureEvent::Start).action == CaptureAction::None);
  assert(machine.generation() == running);
  assert(machine.reduce(CaptureEvent::Sleep).action == CaptureAction::ReleaseAll);
  assert(machine.reduce(CaptureEvent::Sleep).action == CaptureAction::None);
  assert(machine.reduce(CaptureEvent::Wake).action == CaptureAction::EnumerateDisplays);
  assert(machine.reduce(CaptureEvent::Stop).action == CaptureAction::ReleaseAll);
  assert(machine.phase() == CapturePhase::Stopped);
  assert(machine.reduce(CaptureEvent::Wake).action == CaptureAction::None);
  assert(machine.reduce(CaptureEvent::SwitchDisplay).action == CaptureAction::None);
  assert(machine.reduce(CaptureEvent::Start).action == CaptureAction::None);
  const CaptureDecision deadline = machine.reduce(CaptureEvent::StopDeadline);
  assert(deadline.action == CaptureAction::StopTimedOut && deadline.stopBounded);
  assert(!deadline.desktopAvailable);
}

void deadline_is_terminal_even_before_stop() {
  CaptureMachine machine;
  (void)machine.reduce(CaptureEvent::Start);
  const uint64_t before = machine.generation();
  const CaptureDecision timeout = machine.reduce(CaptureEvent::StopDeadline);
  assert(timeout.stopBounded && !timeout.desktopAvailable);
  assert(timeout.generation == machine.generation());
  assert(timeout.generation > before);
  assert(machine.phase() == CapturePhase::Stopped);
  assert(machine.reduce(CaptureEvent::Wake).action == CaptureAction::None);
}

void failure_invalidates_the_published_generation() {
  CaptureMachine machine;
  (void)machine.reduce(CaptureEvent::Start);
  const uint64_t published = machine.generation();
  assert(machine.reduceFrameReady(published).action ==
         CaptureAction::PublishFrame);
  const CaptureDecision failure = machine.reduce(CaptureEvent::Failed);
  assert(failure.generation == machine.generation());
  assert(failure.generation > published);
  assert(!machine.hasCurrentFrame());
  assert(machine.reduceFrameReady(published).action == CaptureAction::ClearFrame);
}

void capture_retry_is_independent_bounded_and_backed_off() {
  CaptureRetryState retry;
  assert(retry.waitMilliseconds(0) == UINT32_MAX);
  assert(retry.request(100, true));
  assert(retry.due(100));
  retry.failed(100);
  assert(!retry.due(199));
  assert(retry.due(200));
  retry.failed(200);
  assert(!retry.due(399));
  assert(retry.due(400));
  retry.failed(400);
  assert(!retry.due(799));
  assert(retry.due(800));
  retry.failed(800);
  assert(retry.exhausted());
  assert(!retry.pending());
  assert(!retry.request(900, false));
  assert(retry.request(1000, true));
  retry.succeeded();
  assert(!retry.pending());
  assert(retry.attempts() == 0);
  assert(retry.request(1100, false));
  retry.cancel();
  assert(!retry.pending());
}

void capture_stop_retry_is_bounded_and_observable() {
  CaptureStopRetryState retry;
  assert(retry.request(50, true));
  assert(retry.due(50));
  retry.failed(50);
  assert(retry.waitMilliseconds(50) == 25);
  assert(retry.due(75));
  retry.failed(75);
  assert(retry.waitMilliseconds(75) == 50);
  assert(retry.due(125));
  retry.failed(125);
  assert(retry.waitMilliseconds(125) == 100);
  assert(retry.due(225));
  retry.failed(225);
  assert(retry.exhausted());
  assert(!retry.pending());
  assert(!retry.request(300, false));
  assert(retry.request(400, true));
  retry.succeeded();
  assert(!retry.pending());
}

void captured_frame_identity_must_match_the_active_owner() {
  const CaptureFrameIdentity owner{7, 0x1111, 0x2222, -3};
  assert(CaptureFrameMatchesOwner(owner, {7, 0x1111, 0x2222, -3}));
  assert(!CaptureFrameMatchesOwner(owner, {6, 0x1111, 0x2222, -3}));
  assert(!CaptureFrameMatchesOwner(owner, {7, 0x9999, 0x2222, -3}));
  assert(!CaptureFrameMatchesOwner(owner, {7, 0x1111, 0x3333, -3}));
  assert(!CaptureFrameMatchesOwner(owner, {7, 0x1111, 0x2222, 4}));
}

void pause_preserves_external_capture_capability() {
  assert(CaptureAvailabilityAfterPause(0) == 0);
  assert(CaptureAvailabilityAfterPause(4) == 4);
  assert(CaptureAvailabilityAfterPause(5) == 5);
  assert(CaptureAvailabilityAfterRendererRebuild(4, false) == 4);
  assert(CaptureAvailabilityAfterRendererRebuild(5, false) == 5);
  assert(CaptureAvailabilityAfterRendererRebuild(4, true) == 0);
}

void same_monitor_switch_reuses_only_a_live_worker() {
  assert(CaptureWorkerMayReuseForDisplay(true, true, false, true));
  assert(!CaptureWorkerMayReuseForDisplay(false, true, false, true));
  assert(!CaptureWorkerMayReuseForDisplay(true, false, false, true));
  assert(!CaptureWorkerMayReuseForDisplay(true, true, true, true));
  assert(!CaptureWorkerMayReuseForDisplay(true, true, false, false));
}

void rotation_and_failure_policy() {
  CaptureMachine machine;
  (void)machine.reduce(CaptureEvent::Start);
  const uint64_t before = machine.generation();
  (void)machine.reduceFrameReady(before);
  assert(machine.reduce(CaptureEvent::Rotation90).action == CaptureAction::ClearFrame);
  assert(machine.rotation() == CaptureRotation::Rotate90 && !machine.hasCurrentFrame());
  const uint64_t rotated = machine.generation();
  assert(machine.reduce(CaptureEvent::Rotation90).action == CaptureAction::None);
  assert(machine.generation() == rotated);
  (void)machine.reduce(CaptureEvent::Rotation180);
  (void)machine.reduce(CaptureEvent::Rotation270);
  (void)machine.reduce(CaptureEvent::Rotation0);
  const CaptureDecision device = machine.reduce(CaptureEvent::DeviceRemoved);
  assert(device.action == CaptureAction::RecreateDuplication);
  assert(!device.rendererRemainsReady);
  const CaptureDecision failure = machine.reduce(CaptureEvent::Failed);
  assert(failure.action == CaptureAction::ProceduralFallback);
  assert(failure.rendererRemainsReady && !failure.desktopAvailable);
  assert(machine.reduce(CaptureEvent::Failed).action == CaptureAction::None);
}

void device_reset_requires_renderer_device_recreation() {
  CaptureMachine machine;
  (void)machine.reduce(CaptureEvent::Start);
  const CaptureDecision reset = machine.reduce(CaptureEvent::DeviceReset);
  assert(reset.action == CaptureAction::RecreateDuplication);
  assert(!reset.rendererRemainsReady);
  assert(machine.phase() == CapturePhase::Recovering);
}

void rotation_maps_all_corners() {
  const CaptureUv topLeft{0, 0};
  const CaptureUv topRight{1, 0};
  const CaptureUv bottomLeft{0, 1};
  const CaptureUv bottomRight{1, 1};
  const auto same = [](CaptureUv lhs, CaptureUv rhs) {
    return lhs.x == rhs.x && lhs.y == rhs.y;
  };
  assert(same(RotateCaptureUv(topLeft, CaptureRotation::Identity), {0, 0}));
  assert(same(RotateCaptureUv(topRight, CaptureRotation::Identity), {1, 0}));
  assert(same(RotateCaptureUv(bottomLeft, CaptureRotation::Identity), {0, 1}));
  assert(same(RotateCaptureUv(bottomRight, CaptureRotation::Identity), {1, 1}));
  assert(same(RotateCaptureUv(topLeft, CaptureRotation::Rotate90), {0, 1}));
  assert(same(RotateCaptureUv(topRight, CaptureRotation::Rotate90), {0, 0}));
  assert(same(RotateCaptureUv(bottomLeft, CaptureRotation::Rotate90), {1, 1}));
  assert(same(RotateCaptureUv(bottomRight, CaptureRotation::Rotate90), {1, 0}));
  assert(same(RotateCaptureUv(topLeft, CaptureRotation::Rotate180), {1, 1}));
  assert(same(RotateCaptureUv(topRight, CaptureRotation::Rotate180), {0, 1}));
  assert(same(RotateCaptureUv(bottomLeft, CaptureRotation::Rotate180), {1, 0}));
  assert(same(RotateCaptureUv(bottomRight, CaptureRotation::Rotate180), {0, 0}));
  assert(same(RotateCaptureUv(topLeft, CaptureRotation::Rotate270), {1, 0}));
  assert(same(RotateCaptureUv(topRight, CaptureRotation::Rotate270), {1, 1}));
  assert(same(RotateCaptureUv(bottomLeft, CaptureRotation::Rotate270), {0, 0}));
  assert(same(RotateCaptureUv(bottomRight, CaptureRotation::Rotate270), {0, 1}));
}

int main() {
  frame_generation_and_timeout();
  lifecycle_is_terminal_and_idempotent();
  deadline_is_terminal_even_before_stop();
  failure_invalidates_the_published_generation();
  capture_retry_is_independent_bounded_and_backed_off();
  capture_stop_retry_is_bounded_and_observable();
  captured_frame_identity_must_match_the_active_owner();
  pause_preserves_external_capture_capability();
  same_monitor_switch_reuses_only_a_live_worker();
  rotation_and_failure_policy();
  device_reset_requires_renderer_device_recreation();
  rotation_maps_all_corners();
}
