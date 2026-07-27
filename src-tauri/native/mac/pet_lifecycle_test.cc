#include "pet_lifecycle.h"
#include "pet_visual_state.h"

#include <assert.h>
#include <initializer_list>
#include <math.h>

static void event_horizon_circle_fits_inside_core_hit_target() {
  for (const double effectDiameter : {120.0, 220.0, 360.0}) {
    const PetEventHorizonGeometry geometry =
        PetEventHorizonGeometryForEffectDiameter(effectDiameter);
    const double eventHorizonRadius = geometry.event_horizon_diameter / 2.0;
    const double hitTargetHalfSide = geometry.core_hit_target_side / 2.0;
    assert(geometry.event_horizon_diameter > 0.0);
    assert(geometry.event_horizon_diameter ==
           geometry.core_hit_target_side);
    assert(eventHorizonRadius <= hitTargetHalfSide);
  }
}

static void core_hit_target_corners_stay_inside_decorative_effect_circle() {
  for (const double effectDiameter : {120.0, 220.0, 360.0}) {
    const PetEventHorizonGeometry geometry =
        PetEventHorizonGeometryForEffectDiameter(effectDiameter);
    const double hitTargetHalfSide = geometry.core_hit_target_side / 2.0;
    const double cornerDistance = hypot(hitTargetHalfSide, hitTargetHalfSide);
    assert(geometry.decorative_effect_diameter == effectDiameter);
    assert(cornerDistance < geometry.decorative_effect_diameter / 2.0);
  }
}

static void pet_visual_and_core_hit_target_windows_share_lifecycle() {
  PetWindowLifecycle lifecycle;
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(!lifecycle.destroyed());

  assert(lifecycle.show());
  assert(lifecycle.visual_visible());
  assert(lifecycle.core_hit_target_visible());
  assert(!lifecycle.show());

  assert(lifecycle.hide());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(!lifecycle.hide());

  assert(lifecycle.show());
  assert(lifecycle.destroy());
  assert(!lifecycle.visual_visible());
  assert(!lifecycle.core_hit_target_visible());
  assert(lifecycle.destroyed());
  assert(!lifecycle.show());
  assert(!lifecycle.destroy());
}

static void capture_region_is_bounded_and_uses_retina_pixels() {
  const PetScreenFrame display = {-1280.0, 0.0, 1280.0, 800.0, 2.0, 42};
  const PetPanelFrame panel = {-1100.0, 100.0, 220.0, 220.0};

  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);

  assert(region.display_id == 42);
  assert(region.source_x == 154.0);
  assert(region.source_y == 454.0);
  assert(region.source_width == 272.0);
  assert(region.source_height == 272.0);
  assert(region.pixel_width == 544);
  assert(region.pixel_height == 544);
}

static void capture_policy_excludes_media_and_retains_only_the_newest_frame() {
  const PetCapturePolicy policy = PetSafeCapturePolicy();

  assert(!policy.captures_audio);
  assert(!policy.captures_microphone);
  assert(!policy.shows_cursor);
  assert(policy.excludes_own_process);
  assert(policy.queue_depth == 1);
  assert(policy.maximum_retained_frames == 1);
}

static void stopped_capture_rejects_late_frame_delivery() {
  PetFrameRetention gate;
  assert(!gate.accepting());

  gate.start();
  assert(gate.accepting());

  gate.stop();
  assert(!gate.accepting());
}

static void permission_request_requires_one_explicit_real_mode_action() {
  PetPermissionLifecycle permission;

  PetPermissionDecision decision = permission.preflight(false, false);
  assert(decision.state == PET_CAPTURE_NOT_DETERMINED);
  assert(decision.action == PetPermissionAction::kNone);

  decision = permission.preflight(false, true);
  assert(decision.state == PET_CAPTURE_NOT_DETERMINED);
  assert(decision.action == PetPermissionAction::kRequestSystemPermission);

  decision = permission.request_result(false);
  assert(decision.state == PET_CAPTURE_DENIED);
  assert(decision.action == PetPermissionAction::kNone);

  decision = permission.preflight(false, true);
  assert(decision.state == PET_CAPTURE_DENIED);
  assert(decision.action == PetPermissionAction::kNone);
}

static void permission_change_after_denial_requires_a_clean_restart() {
  PetPermissionLifecycle permission;
  permission.preflight(false, true);
  permission.request_result(false);

  const PetPermissionDecision decision = permission.preflight(true, false);

  assert(decision.state == PET_CAPTURE_RESTART_REQUIRED);
  assert(decision.action == PetPermissionAction::kNone);
}

static void existing_permission_enumerates_without_requesting_again() {
  PetPermissionLifecycle permission;

  const PetPermissionDecision decision = permission.preflight(true, false);

  assert(decision.state == PET_CAPTURE_READY);
  assert(decision.action == PetPermissionAction::kEnumerateCapture);
}

static void revoked_existing_permission_falls_back_without_prompting() {
  PetPermissionLifecycle permission;
  permission.preflight(true, false);

  const PetPermissionDecision decision = permission.preflight(false, false);

  assert(decision.state == PET_CAPTURE_DENIED);
  assert(decision.action == PetPermissionAction::kNone);
}

static void pending_tasks_map_one_to_one_onto_concentric_orbit_dots() {
  assert(PetPendingDotCount(0) == 0);
  assert(PetPendingDotCount(37) == 37);
  assert(PetPendingRingCount(8) == 1);
  assert(PetPendingRingCount(9) == 2);
  assert(PetPendingRingCount(37) == 4);

  const PetPendingDotPlacement eighth =
      PetPendingDotPlacementForIndex(7, 37);
  const PetPendingDotPlacement ninth =
      PetPendingDotPlacementForIndex(8, 37);
  const PetPendingDotPlacement thirty_seventh =
      PetPendingDotPlacementForIndex(36, 37);
  assert(eighth.ring_index == 0);
  assert(eighth.dots_in_ring == 8);
  assert(ninth.ring_index == 1);
  assert(ninth.dots_in_ring == 12);
  assert(thirty_seventh.ring_index == 3);
  assert(thirty_seventh.dots_in_ring == 1);
  assert(eighth.normalized_radius < ninth.normalized_radius);
  assert(ninth.normalized_radius < thirty_seventh.normalized_radius);
}

static void native_signal_codes_map_to_distinct_visual_effects() {
  assert(PetVisualSignalForCode(1) ==
         PetVisualSignalEffect::kImportSwallow);
  assert(PetVisualSignalForCode(2) ==
         PetVisualSignalEffect::kFailureRedRipple);
  assert(PetVisualSignalForCode(3) ==
         PetVisualSignalEffect::kSettlementGreenRing);
  assert(PetVisualSignalForCode(0) == PetVisualSignalEffect::kNone);
  assert(PetVisualSignalForCode(99) == PetVisualSignalEffect::kNone);
}

static void visual_state_applies_pending_count_and_signal_code() {
  PetVisualState state;
  state.apply_pending_count(21);
  assert(state.pending_count() == 21);
  assert(state.pending_dot_count() == 21);

  state.apply_signal(3);
  assert(state.signal_effect() ==
         PetVisualSignalEffect::kSettlementGreenRing);
  state.apply_signal(2);
  assert(state.signal_effect() == PetVisualSignalEffect::kFailureRedRipple);
}

int main() {
  event_horizon_circle_fits_inside_core_hit_target();
  core_hit_target_corners_stay_inside_decorative_effect_circle();
  pet_visual_and_core_hit_target_windows_share_lifecycle();
  capture_region_is_bounded_and_uses_retina_pixels();
  capture_policy_excludes_media_and_retains_only_the_newest_frame();
  stopped_capture_rejects_late_frame_delivery();
  permission_request_requires_one_explicit_real_mode_action();
  permission_change_after_denial_requires_a_clean_restart();
  existing_permission_enumerates_without_requesting_again();
  revoked_existing_permission_falls_back_without_prompting();
  pending_tasks_map_one_to_one_onto_concentric_orbit_dots();
  native_signal_codes_map_to_distinct_visual_effects();
  visual_state_applies_pending_count_and_signal_code();
}
