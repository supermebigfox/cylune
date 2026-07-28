#include "pet_lifecycle.h"
#include "pet_render_state.h"
#include "pet_visual_state.h"

#include <assert.h>
#include <chrono>
#include <initializer_list>
#include <math.h>
#include <memory>
#include <thread>
#include <vector>

struct FakeRendererBackend {
  bool create_success = true;
  std::vector<uint32_t> draw_results;
  size_t draw_index = 0;
  uint32_t create_calls = 0;
  uint32_t draw_calls = 0;
  uint32_t destroy_calls = 0;
};

static void *fake_renderer_create(void *context, const char *source,
                                  void *layer) {
  (void)source;
  (void)layer;
  auto *fake = static_cast<FakeRendererBackend *>(context);
  ++fake->create_calls;
  return fake->create_success ? fake : nullptr;
}

static uint32_t fake_renderer_draw(void *context, void *handle,
                                   IOSurfaceRef surface,
                                   PetRenderUniforms uniforms) {
  (void)handle;
  (void)surface;
  (void)uniforms;
  auto *fake = static_cast<FakeRendererBackend *>(context);
  ++fake->draw_calls;
  if (fake->draw_index >= fake->draw_results.size()) {
    return PET_RENDER_DRAW_OK;
  }
  return fake->draw_results[fake->draw_index++];
}

static void fake_renderer_destroy(void *context, void *handle) {
  (void)handle;
  auto *fake = static_cast<FakeRendererBackend *>(context);
  ++fake->destroy_calls;
}

static PetRendererBackend fake_renderer_backend(FakeRendererBackend *fake) {
  return {fake, fake_renderer_create, fake_renderer_draw,
          fake_renderer_destroy};
}

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

static void greatest_intersection_selects_the_new_display() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1512.0, 982.0, 2.0, 1},
      {1512.0, 0.0, 1920.0, 1080.0, 1.0, 2},
  };
  const PetPanelFrame panel = {1450.0, 100.0, 220.0, 220.0};

  assert(PetGreatestIntersectionDisplayIndex(panel, displays, 2) == 1);
}

static void disconnected_panel_returns_to_primary_with_safe_inset() {
  const PetScreenFrame primary = {0.0, 0.0, 1512.0, 982.0, 2.0, 7};
  const PetPanelFrame disconnected = {5000.0, -80.0, 220.0, 220.0};

  const PetPanelFrame placed =
      PetClampPanelToDisplay(disconnected, primary, 16.0);

  assert(placed.x == 1276.0);
  assert(placed.y == 16.0);
  assert(placed.width == 220.0);
  assert(placed.height == 220.0);
}

static void drag_persistence_is_emitted_only_once_on_mouse_up() {
  PetDragPersistenceGate gate;

  gate.begin();
  assert(!gate.dragged());
  assert(!gate.should_persist(false));
  gate.mark_dragged();
  assert(gate.dragged());
  assert(!gate.should_persist(false));
  assert(gate.should_persist(true));
  assert(!gate.should_persist(true));
}

static void live_capture_reconfiguration_ignores_fps_and_pending_only_updates() {
  PetCaptureConfigurationGate gate;
  const PetCaptureRegion region = {42, 154.0, 454.0, 272.0, 272.0, 544, 544};
  const PetCaptureConfigurationKey real = {0, true, region};

  assert(gate.should_configure(real, false));
  assert(!gate.should_configure(real, false));
  // FPS and pending count are intentionally absent from the capture key.
  assert(!gate.should_configure(real, false));
  assert(gate.should_configure(real, true));

  PetCaptureConfigurationKey resized = real;
  resized.region.pixel_width = 744;
  resized.region.pixel_height = 744;
  resized.region.source_width = 372.0;
  resized.region.source_height = 372.0;
  assert(gate.should_configure(resized, false));

  PetCaptureConfigurationKey hidden = resized;
  hidden.visible = false;
  assert(gate.should_configure(hidden, false));
}

static void native_failure_reason_is_published_once_until_an_allowed_retry() {
  PetFaultLatch failure;

  assert(failure.report_once());
  assert(!failure.report_once());
  assert(!failure.report_once());

  failure.reset();
  assert(failure.report_once());
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

static void metal_unavailable_disables_real_capture_and_requests_stop() {
  const PetRendererDecision decision =
      PetRendererDecisionForMetalAvailability(false);

  assert(decision.state == PET_RENDERER_UNAVAILABLE);
  assert(!decision.real_effect_available);
  assert(decision.stop_capture);
}

static void stopped_capture_rejects_late_frame_delivery() {
  PetFrameRetention gate;
  assert(!gate.accepting());

  gate.start();
  assert(gate.accepting());

  gate.stop();
  assert(!gate.accepting());
}

static void shutdown_waits_for_delayed_stop_before_release_and_destroy() {
  using namespace std::chrono_literals;

  std::vector<int> actions;
  PetFrameRetention frame_gate;
  frame_gate.start();
  frame_gate.stop();
  auto completion = std::make_shared<PetStopCompletion>();

  std::thread delayed_completion([completion] {
    std::this_thread::sleep_for(40ms);
    completion->complete(true);
  });
  const auto started = std::chrono::steady_clock::now();
  const PetShutdownState state = completion->wait_for(250ms);
  actions.push_back(1);  // Release the retained IOSurface.
  actions.push_back(2);  // Destroy the capture service.
  const auto elapsed = std::chrono::steady_clock::now() - started;
  delayed_completion.join();

  assert(state == PetShutdownState::kComplete);
  assert(elapsed >= 30ms);
  assert(!frame_gate.accepting());
  assert((actions == std::vector<int>{1, 2}));
}

static void shutdown_timeout_is_stable_and_keeps_the_frame_gate_closed() {
  using namespace std::chrono_literals;

  PetFrameRetention frame_gate;
  frame_gate.start();
  frame_gate.stop();
  PetStopCompletion completion;

  assert(completion.wait_for(5ms) == PetShutdownState::kStopTimedOut);
  completion.complete(true);

  assert(completion.state() == PetShutdownState::kStopTimedOut);
  assert(!frame_gate.accepting());
}

static void shutdown_stop_error_is_stable() {
  using namespace std::chrono_literals;

  PetStopCompletion completion;
  completion.complete(false);

  assert(completion.wait_for(50ms) == PetShutdownState::kStopFailed);
  completion.complete(true);
  assert(completion.state() == PetShutdownState::kStopFailed);
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

static void hidden_explicit_real_selection_requests_once_without_streaming() {
  PetPermissionLifecycle permission;
  const PetApplyCapturePlan hidden =
      PetApplyCapturePlanForVisibility(false, true);

  assert(hidden.refresh_capture);
  assert(hidden.request_permission);
  PetPermissionDecision decision =
      permission.preflight(false, hidden.request_permission);
  assert(decision.action == PetPermissionAction::kRequestSystemPermission);
  assert(!PetShouldStartCapture(decision, false));

  const PetApplyCapturePlan shown =
      PetApplyCapturePlanForVisibility(true, false);
  decision = permission.preflight(false, shown.request_permission);
  assert(decision.action == PetPermissionAction::kNone);
  assert(!PetShouldStartCapture(decision, true));

  PetPermissionLifecycle already_granted;
  decision = already_granted.preflight(true, hidden.request_permission);
  assert(decision.action == PetPermissionAction::kEnumerateCapture);
  assert(!PetShouldStartCapture(decision, false));
  decision = already_granted.preflight(true, shown.request_permission);
  assert(decision.action == PetPermissionAction::kEnumerateCapture);
  assert(PetShouldStartCapture(decision, true));
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

static void native_frame_pacing_matches_auto_fixed_and_hidden_modes() {
  assert(PetTargetFps(0, PetRenderActivity::kIdle) == 30);
  assert(PetTargetFps(0, PetRenderActivity::kDropHover) == 60);
  assert(PetTargetFps(0, PetRenderActivity::kSignal) == 60);
  assert(PetTargetFps(30, PetRenderActivity::kDropHover) == 30);
  assert(PetTargetFps(60, PetRenderActivity::kIdle) == 60);
  assert(PetTargetFps(60, PetRenderActivity::kHidden) == 0);
}

static void animation_state_distinguishes_hover_and_each_signal() {
  PetRenderAnimationState state;
  state.set_hover(true, 2.0);
  PetAnimationSnapshot hover = state.sample(2.15, false);
  assert(hover.hover_progress > 0.95);
  assert(hover.activity == PetRenderActivity::kDropHover);

  state.signal(1, 3.0);
  PetAnimationSnapshot swallow = state.sample(3.26, false);
  assert(swallow.swallow_progress > 0.45);
  assert(swallow.swallow_progress < 0.55);
  assert(swallow.activity == PetRenderActivity::kSignal);

  state.signal(3, 4.0);
  PetAnimationSnapshot success = state.sample(4.24, false);
  assert(success.success_progress > 0.45);
  assert(success.success_progress < 0.55);

  state.signal(2, 5.0);
  PetAnimationSnapshot error = state.sample(5.21, false);
  assert(error.error_progress > 0.45);
  assert(error.error_progress < 0.55);
}

static void reduced_motion_uses_short_color_transitions_without_spring() {
  assert(PetSignalTransitionDuration(true, 0.52) == 0.15);
  assert(PetSignalTransitionDuration(false, 0.52) == 0.52);

  PetRenderAnimationState state;
  state.set_hover(true, 1.0);
  state.signal(1, 2.0);

  const PetAnimationSnapshot transition = state.sample(2.075, true);
  assert(transition.hover_progress == 0.0);
  assert(transition.swallow_progress > 0.45);
  assert(transition.swallow_progress < 0.55);
  assert(state.sample(2.16, true).swallow_progress == 0.0);
}

static void lite_reduced_motion_pulse_is_one_150ms_transition() {
  const PetLitePulseAnimation reduced =
      PetLitePulseAnimationForMotion(true);
  assert(reduced.duration_seconds ==
         PetSignalTransitionDuration(true, 0.18));
  assert(reduced.duration_seconds == 0.15);
  assert(!reduced.autoreverses);
  assert(PetLitePulseEffectiveDuration(reduced) == 0.15);

  const PetLitePulseAnimation standard =
      PetLitePulseAnimationForMotion(false);
  assert(standard.duration_seconds == 0.12);
  assert(standard.autoreverses);
  assert(PetLitePulseEffectiveDuration(standard) == 0.24);
}

static void hover_exit_keeps_auto_fps_high_until_the_fade_finishes() {
  PetRenderAnimationState state;
  state.set_hover(true, 1.0);
  (void)state.sample(1.2, false);
  state.set_hover(false, 2.0);

  const PetAnimationSnapshot fading = state.sample(2.075, false);
  assert(fading.hover_progress > 0.45);
  assert(fading.hover_progress < 0.55);
  assert(fading.activity == PetRenderActivity::kDropHover);
  assert(state.sample(2.16, false).activity == PetRenderActivity::kIdle);
}

static void completing_a_drop_clears_the_persistent_hover_state() {
  PetRenderAnimationState state;
  state.set_hover(true, 1.0);
  assert(state.sample(1.2, false).hover_progress > 0.99);

  state.complete_drop(2.0);

  assert(state.sample(2.16, false).hover_progress == 0.0);
  assert(state.sample(2.16, false).activity == PetRenderActivity::kIdle);
}

static void renderer_create_failure_reports_once_after_host_binding() {
  FakeRendererBackend fake;
  fake.create_success = false;
  PetRendererDriver driver;

  assert(!driver.initialize(fake_renderer_backend(&fake), "shader", nullptr));
  assert(fake.create_calls == 1);
  assert(driver.bind_host() == PetRendererStep::kBecameUnavailable);
  assert(driver.bind_host() == PetRendererStep::kUnavailable);
  assert(fake.destroy_calls == 0);
}

static void fatal_and_repeated_draw_failures_degrade_once_and_destroy() {
  PetRenderUniforms uniforms = {};
  FakeRendererBackend fatal;
  fatal.draw_results = {PET_RENDER_DRAW_FATAL};
  PetRendererDriver fatal_driver;
  assert(fatal_driver.initialize(fake_renderer_backend(&fatal), "shader",
                                 nullptr));
  assert(fatal_driver.bind_host() == PetRendererStep::kRendered);
  assert(fatal_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kBecameUnavailable);
  assert(fatal.destroy_calls == 1);
  assert(fatal_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kUnavailable);
  assert(fatal.destroy_calls == 1);

  FakeRendererBackend transient;
  transient.draw_results = {
      PET_RENDER_DRAW_TRANSIENT, PET_RENDER_DRAW_TRANSIENT,
      PET_RENDER_DRAW_OK,        PET_RENDER_DRAW_TRANSIENT,
      PET_RENDER_DRAW_TRANSIENT, PET_RENDER_DRAW_TRANSIENT,
  };
  PetRendererDriver transient_driver;
  assert(transient_driver.initialize(fake_renderer_backend(&transient),
                                     "shader", nullptr));
  assert(transient_driver.bind_host() == PetRendererStep::kRendered);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kRetry);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kRetry);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kRendered);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kRetry);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kRetry);
  assert(transient_driver.draw(nullptr, uniforms) ==
         PetRendererStep::kBecameUnavailable);
  assert(transient.destroy_calls == 1);
}

static void frame_dispatch_gate_is_initialized_before_the_first_callback() {
  PetFrameDispatchGate gate;
  assert(!gate.try_enqueue());
  gate.set_enabled(true);
  assert(gate.try_enqueue());
  assert(!gate.try_enqueue());
  gate.complete();
  assert(gate.try_enqueue());
  gate.set_enabled(false);
  assert(!gate.try_enqueue());
}

int main() {
  event_horizon_circle_fits_inside_core_hit_target();
  core_hit_target_corners_stay_inside_decorative_effect_circle();
  pet_visual_and_core_hit_target_windows_share_lifecycle();
  capture_region_is_bounded_and_uses_retina_pixels();
  greatest_intersection_selects_the_new_display();
  disconnected_panel_returns_to_primary_with_safe_inset();
  drag_persistence_is_emitted_only_once_on_mouse_up();
  live_capture_reconfiguration_ignores_fps_and_pending_only_updates();
  native_failure_reason_is_published_once_until_an_allowed_retry();
  capture_policy_excludes_media_and_retains_only_the_newest_frame();
  metal_unavailable_disables_real_capture_and_requests_stop();
  stopped_capture_rejects_late_frame_delivery();
  shutdown_waits_for_delayed_stop_before_release_and_destroy();
  shutdown_timeout_is_stable_and_keeps_the_frame_gate_closed();
  shutdown_stop_error_is_stable();
  permission_request_requires_one_explicit_real_mode_action();
  hidden_explicit_real_selection_requests_once_without_streaming();
  permission_change_after_denial_requires_a_clean_restart();
  existing_permission_enumerates_without_requesting_again();
  revoked_existing_permission_falls_back_without_prompting();
  pending_tasks_map_one_to_one_onto_concentric_orbit_dots();
  native_signal_codes_map_to_distinct_visual_effects();
  visual_state_applies_pending_count_and_signal_code();
  native_frame_pacing_matches_auto_fixed_and_hidden_modes();
  animation_state_distinguishes_hover_and_each_signal();
  reduced_motion_uses_short_color_transitions_without_spring();
  lite_reduced_motion_pulse_is_one_150ms_transition();
  hover_exit_keeps_auto_fps_high_until_the_fade_finishes();
  completing_a_drop_clears_the_persistent_hover_state();
  renderer_create_failure_reports_once_after_host_binding();
  fatal_and_repeated_draw_failures_degrade_once_and_destroy();
  frame_dispatch_gate_is_initialized_before_the_first_callback();
}
