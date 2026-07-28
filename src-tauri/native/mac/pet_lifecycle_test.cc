#include "pet_lifecycle.h"
#include "pet_drop_state.h"
#include "pet_render_state.h"
#include "pet_visual_state.h"

#include <assert.h>
#include <cstddef>
#include <chrono>
#include <initializer_list>
#include <math.h>
#include <memory>
#include <thread>
#include <vector>

static bool close_to(double lhs, double rhs, double epsilon = 1e-6) {
  return fabs(lhs - rhs) <= epsilon;
}

static void center_selects_display_and_maps_negative_coordinates() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1440.0, 900.0, 2.0, 10},
      {-1920.0, 0.0, 1920.0, 1080.0, 1.0, 20},
  };
  assert(PetDisplayIndexForPoint({720.0, 450.0}, displays, 2, 1) == 0);
  assert(PetDisplayIndexForPoint({-960.0, 540.0}, displays, 2, 0) == 1);
  const PetScreenPoint uv =
      PetCenterUVForDisplay({-960.0, 540.0}, displays[1]);
  assert(close_to(uv.x, 0.5));
  assert(close_to(uv.y, 0.5));
}

static void exact_display_seam_keeps_current_display() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1440.0, 900.0, 2.0, 10},
      {1440.0, 0.0, 1920.0, 1080.0, 1.0, 20},
  };
  assert(PetDisplayIndexForPoint({1440.0, 400.0}, displays, 2, 0) == 0);
  assert(PetDisplayIndexForPoint({1440.0, 400.0}, displays, 2, 1) == 1);
  assert(PetDisplayIndexForPoint({1440.1, 400.0}, displays, 2, 0) == 1);
}

static void full_display_capture_never_changes_with_center() {
  const PetScreenFrame display = {-1920.0, 0.0, 1920.0, 1080.0, 2.0, 20};
  const PetCaptureRegion region = PetFullDisplayCaptureRegion(display);
  assert(region.display_id == 20);
  assert(close_to(region.source_x, 0.0));
  assert(close_to(region.source_y, 0.0));
  assert(close_to(region.source_width, 1920.0));
  assert(close_to(region.source_height, 1080.0));
  assert(region.pixel_width == 3840);
  assert(region.pixel_height == 2160);
  assert(close_to(region.panel_origin_uv[0], 0.0));
  assert(close_to(region.panel_origin_uv[1], 0.0));
  assert(close_to(region.panel_extent_uv[0], 1.0));
  assert(close_to(region.panel_extent_uv[1], 1.0));
}

static void moving_center_does_not_reconfigure_full_display_capture() {
  const PetScreenFrame display = {0.0, 0.0, 1512.0, 982.0, 2.0, 42};
  const PetCaptureRegion first = PetFullDisplayCaptureRegion(display);
  const PetCaptureRegion after_drag =
      PetFullDisplayCaptureRegion(display);
  PetCaptureConfigurationGate gate;
  assert(gate.should_configure({0, true, first}, false));
  assert(!gate.should_configure({0, true, after_drag}, false));
}

static void adjacent_panes_receive_complementary_seam_centers() {
  const PetScreenFrame left = {0.0, 0.0, 1440.0, 900.0, 2.0, 10};
  const PetScreenFrame right = {1440.0, 0.0, 1920.0, 1080.0, 1.0, 20};
  const PetScreenPoint left_uv =
      PetCenterUVForDisplay({1440.0, 450.0}, left);
  const PetScreenPoint right_uv =
      PetCenterUVForDisplay({1440.0, 450.0}, right);
  assert(close_to(left_uv.x, 1.0));
  assert(close_to(right_uv.x, 0.0));
}

static void disconnected_center_recovers_only_when_off_every_display() {
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 10};
  const PetScreenPoint retained =
      PetRecoverCenter({0.0, 900.0}, &display, 1);
  assert(close_to(retained.x, 0.0));
  assert(close_to(retained.y, 900.0));
  const PetScreenPoint recovered =
      PetRecoverCenter({4000.0, 3000.0}, &display, 1);
  assert(close_to(recovered.x, 720.0));
  assert(close_to(recovered.y, 450.0));
}

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

static void visual_style_values_stay_within_stable_native_contract() {
  static_assert(sizeof(PetConfig) == 64);
  static_assert(offsetof(PetConfig, visual_style) == 62);
  static_assert(sizeof(PetRenderUniforms) == 160);
  static_assert(offsetof(PetRenderUniforms, center_uv) == 24);
  static_assert(offsetof(PetRenderUniforms, visual_style) == 148);
  assert(PetVisualStyleIsValid(0));
  assert(PetVisualStyleIsValid(1));
  assert(!PetVisualStyleIsValid(2));
  assert(!PetVisualStyleIsValid(255));
}

static void approved_geometry_uses_a_small_circular_core() {
  const PetEffectGeometry small = PetEffectGeometryForSize(120.0);
  const PetEffectGeometry medium = PetEffectGeometryForSize(220.0);
  const PetEffectGeometry large = PetEffectGeometryForSize(360.0);
  assert(close_to(small.shadow_radius, 9.0));
  assert(close_to(medium.shadow_radius, 16.5));
  assert(close_to(large.shadow_radius, 27.0));
  assert(close_to(small.hit_radius, 22.0));
  assert(close_to(medium.hit_radius, 22.0));
  assert(close_to(large.hit_radius, 31.05));
  assert(PetPointInsideCore(60.0, 60.0, small));
  assert(PetPointInsideCore(81.9, 60.0, small));
  assert(!PetPointInsideCore(82.1, 60.0, small));
  assert(!PetPointInsideCore(0.0, 0.0, small));
}

static void size_changes_radius_without_changing_the_full_display_pane() {
  const PetEffectGeometry small = PetEffectGeometryForSize(300.0);
  const PetEffectGeometry large = PetEffectGeometryForSize(900.0);
  assert(close_to(small.shadow_radius, 22.5));
  assert(close_to(large.shadow_radius, 67.5));
  assert(close_to(small.hit_radius, 25.875));
  assert(close_to(large.hit_radius, 77.625));
}

static void large_sizes_keep_logical_geometry_but_cap_the_drawable() {
  assert(close_to(PetDrawableLogicalSide(300.0), 300.0));
  assert(close_to(PetDrawableLogicalSide(360.0), 360.0));

  const PetScreenFrame display = {0.0, 0.0, 2560.0, 1600.0, 2.0, 42};
  for (const double size : {600.0, 900.0}) {
    const PetEffectGeometry geometry = PetEffectGeometryForSize(size);
    assert(close_to(geometry.panel_side, size));
    assert(close_to(geometry.shadow_radius, size * 0.075));
    assert(close_to(PetDrawableLogicalSide(size), 360.0));

    const PetCaptureRegion capture =
        PetCaptureRegionForPanel({800.0, 350.0, size, size}, display);
    assert(close_to(capture.source_width, size * 1.60));
    assert(close_to(capture.source_height, size * 1.60));
    assert(close_to(capture.panel_extent_uv[0], 0.625));
    assert(close_to(capture.panel_extent_uv[1], 0.625));
  }
}

static void centered_capture_maps_the_panel_into_the_middle_five_eighths() {
  const PetPanelFrame panel = {100.0, 300.0, 220.0, 220.0};
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 42};
  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);
  assert(close_to(region.source_x, 34.0));
  assert(close_to(region.source_y, 314.0));
  assert(close_to(region.source_width, 352.0));
  assert(close_to(region.source_height, 352.0));
  assert(region.pixel_width == 704);
  assert(region.pixel_height == 704);
  assert(close_to(region.panel_origin_uv[0], 0.1875));
  assert(close_to(region.panel_origin_uv[1], 0.1875));
  assert(close_to(region.panel_extent_uv[0], 0.625));
  assert(close_to(region.panel_extent_uv[1], 0.625));
}

static void left_edge_capture_does_not_stretch_the_desktop() {
  const PetPanelFrame panel = {0.0, 300.0, 220.0, 220.0};
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 42};
  const PetCaptureRegion region = PetCaptureRegionForPanel(panel, display);
  assert(close_to(region.source_x, 0.0));
  assert(close_to(region.source_width, 286.0));
  assert(close_to(region.panel_origin_uv[0], 0.0));
  assert(close_to(region.panel_extent_uv[0], 220.0 / 286.0));
  assert(close_to(region.panel_origin_uv[1], 66.0 / 352.0));
  assert(close_to(region.panel_extent_uv[1], 220.0 / 352.0));
}

static void other_screen_edges_preserve_the_clipped_panel_mapping() {
  const PetScreenFrame display = {0.0, 0.0, 1440.0, 900.0, 2.0, 42};

  const PetCaptureRegion right = PetCaptureRegionForPanel(
      {1220.0, 300.0, 220.0, 220.0}, display);
  assert(close_to(right.source_x, 1154.0));
  assert(close_to(right.source_width, 286.0));
  assert(close_to(right.panel_origin_uv[0], 66.0 / 286.0));
  assert(close_to(right.panel_extent_uv[0], 220.0 / 286.0));

  const PetCaptureRegion top = PetCaptureRegionForPanel(
      {610.0, 680.0, 220.0, 220.0}, display);
  assert(close_to(top.source_y, 0.0));
  assert(close_to(top.source_height, 286.0));
  assert(close_to(top.panel_origin_uv[1], 0.0));
  assert(close_to(top.panel_extent_uv[1], 220.0 / 286.0));

  const PetCaptureRegion bottom = PetCaptureRegionForPanel(
      {610.0, 0.0, 220.0, 220.0}, display);
  assert(close_to(bottom.source_y, 614.0));
  assert(close_to(bottom.source_height, 286.0));
  assert(close_to(bottom.panel_origin_uv[1], 66.0 / 286.0));
  assert(close_to(bottom.panel_extent_uv[1], 220.0 / 286.0));
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
  assert(region.source_x == 114.0);
  assert(region.source_y == 414.0);
  assert(region.source_width == 352.0);
  assert(region.source_height == 352.0);
  assert(region.pixel_width == 704);
  assert(region.pixel_height == 704);
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

static void missing_saved_display_uses_system_primary_not_focused_screen() {
  const PetScreenFrame displays[] = {
      {0.0, 0.0, 1512.0, 982.0, 2.0, 1},
      {1512.0, 0.0, 1920.0, 1080.0, 1.0, 2},
      {-1280.0, 0.0, 1280.0, 800.0, 1.0, 3},
  };
  const size_t focused_display_index = 2;

  const size_t selected =
      PetSavedDisplayOrPrimaryIndex(99, displays, 3);

  assert(focused_display_index != 0);
  assert(selected == 0);
}

static void backing_scale_updates_drawable_pixels_without_logical_resize() {
  const PetDrawableMetrics one_x =
      PetDrawableMetricsForLogicalSize(220.0, 220.0, 1.0);
  const PetDrawableMetrics two_x =
      PetDrawableMetricsForLogicalSize(220.0, 220.0, 2.0);

  assert(one_x.contents_scale == 1.0);
  assert(one_x.pixel_width == 220.0);
  assert(one_x.pixel_height == 220.0);
  assert(two_x.contents_scale == 2.0);
  assert(two_x.pixel_width == 440.0);
  assert(two_x.pixel_height == 440.0);
  assert(one_x.logical_width == two_x.logical_width);
  assert(one_x.logical_height == two_x.logical_height);
}

static void reverse_main_queue_delivery_discards_the_older_apply() {
  PetApplyGenerationGate generation;
  PetConfig old_config = {};
  old_config.size = 220.0;
  old_config.x = 100.0;
  old_config.y = 80.0;
  old_config.display_id = 1;
  PetConfig new_config = old_config;
  new_config.size = 300.0;
  new_config.x = 1700.0;
  new_config.y = 120.0;
  new_config.display_id = 2;

  const uint64_t queued_worker_old = generation.issue();
  const uint64_t inline_main_new = generation.issue();
  PetConfig applied = {};
  if (generation.accept(inline_main_new)) {
    applied = new_config;
  }
  if (generation.accept(queued_worker_old)) {
    applied = old_config;
  }

  assert(applied.size == 300.0);
  assert(applied.x == 1700.0);
  assert(applied.y == 120.0);
  assert(applied.display_id == 2);
  assert(generation.last_accepted() == inline_main_new);
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
  const PetCaptureRegion region = {
      42, 154.0, 454.0, 272.0, 272.0, 544, 544, {0.0f, 0.0f}, {1.0f, 1.0f}};
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

static void same_display_drag_reaims_without_restarting_capture() {
  const PetCaptureRegion start = {
      42, 154.0, 454.0, 352.0, 352.0, 704, 704,
      {0.1875f, 0.1875f}, {0.625f, 0.625f}};
  PetCaptureRegion moved = start;
  moved.source_x += 180.0;
  moved.source_y += 90.0;

  assert(PetCaptureUpdateActionFor(start, start, true, true) ==
         PetCaptureUpdateAction::kNone);
  assert(PetCaptureUpdateActionFor(start, moved, true, true) ==
         PetCaptureUpdateAction::kReaim);
  assert(PetCaptureUpdateActionFor(start, moved, true, false) ==
         PetCaptureUpdateAction::kRestart);

  PetCaptureRegion other_display = moved;
  other_display.display_id = 77;
  assert(PetCaptureUpdateActionFor(start, other_display, true, true) ==
         PetCaptureUpdateAction::kRestart);
  assert(PetCaptureUpdateActionFor(start, moved, false, true) ==
         PetCaptureUpdateAction::kRestart);
}

static void missing_live_frame_falls_back_without_a_black_capture_disc() {
  assert(PetEffectiveRenderMode(0, true) == 0);
  assert(PetEffectiveRenderMode(0, false) == 1);
  assert(PetEffectiveRenderMode(1, true) == 1);
  assert(PetEffectiveRenderMode(1, false) == 1);
}

static void capture_restart_coalesces_to_the_latest_drag_target() {
  PetCaptureRestartGate gate;
  PetCaptureRegion first = {
      42, 100.0, 100.0, 352.0, 352.0, 704, 704, {0.0f, 0.0f}, {1.0f, 1.0f}};
  PetCaptureRegion latest = first;
  latest.display_id = 77;
  latest.source_x = 240.0;

  assert(gate.request(first, 30));
  assert(gate.in_flight());
  assert(!gate.request(latest, 60));
  assert(gate.desired_region().display_id == 77);
  assert(gate.desired_region().source_x == 240.0);
  assert(gate.desired_fps() == 60);

  gate.complete();
  assert(!gate.in_flight());
  assert(gate.request(latest, 60));
  gate.cancel();
  assert(!gate.in_flight());
}

static void stale_reaim_token_cannot_clear_a_new_generation() {
  PetCaptureReaimGate gate;
  const uint64_t old_token = gate.begin();
  assert(old_token != 0);
  assert(gate.pending());
  assert(gate.begin() == 0);

  gate.cancel();
  const uint64_t new_token = gate.begin();
  assert(new_token != 0 && new_token != old_token);
  assert(!gate.complete(old_token));
  assert(gate.pending());
  assert(gate.complete(new_token));
  assert(!gate.pending());
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

static void stale_or_suspended_capture_frames_are_never_reused() {
  PetCaptureFrameFreshness freshness;
  assert(!freshness.reusable_at(10.0));

  freshness.complete_at(10.0);
  assert(freshness.reusable_at(10.49));
  assert(!freshness.reusable_at(10.51));

  freshness.complete_at(20.0);
  freshness.idle_at(20.4);
  assert(freshness.reusable_at(20.89));
  assert(!freshness.reusable_at(20.91));

  freshness.complete_at(30.0);
  freshness.invalidate();
  assert(!freshness.reusable_at(30.01));
}

static void desktop_pet_stays_below_normal_application_windows() {
  const PetWindowPresentation desktop =
      PetWindowPresentationForAlwaysOnTop(false);
  assert(desktop.layer == PetWindowLayer::kDesktop);
  assert(!desktop.full_screen_auxiliary);
  assert(desktop.core_level_offset == 1);

  const PetWindowPresentation floating =
      PetWindowPresentationForAlwaysOnTop(true);
  assert(floating.layer == PetWindowLayer::kFloating);
  assert(floating.full_screen_auxiliary);
  assert(floating.core_level_offset == 1);
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

static void a_drop_session_requires_the_same_generation_path_and_core_point() {
  PetDropSession session;
  const uint64_t generation =
      session.enter("/tmp/first.gcode.3mf", PET_FILE_3MF);
  assert(generation != 0);
  assert(session.can_submit(generation, "/tmp/first.gcode.3mf", true));
  assert(!session.can_submit(generation + 1, "/tmp/first.gcode.3mf", true));
  assert(!session.can_submit(generation, "/tmp/second.gcode.3mf", true));
  assert(!session.can_submit(generation, "/tmp/first.gcode.3mf", false));
  assert(session.submit(generation, "/tmp/first.gcode.3mf", true));
  assert(!session.submit(generation, "/tmp/first.gcode.3mf", true));
}

static void import_wait_never_crosses_before_acknowledgment() {
  PetDropState state;
  assert(state.begin_wait(7, {0.72f, 0.44f}, PET_FILE_3MF, 10.0));
  const PetDropSnapshot waiting = state.sample(110.0, false);
  assert(waiting.phase == PetDropPhase::kImportPending);
  assert(waiting.faller_progress == 0.0f);
  assert(waiting.absorption_progress == 0.0f);
  assert(!waiting.deliver_once);
}

static void accepted_import_runs_the_complete_reference_timing() {
  PetDropState state;
  assert(state.begin_wait(7, {0.72f, 0.44f}, PET_FILE_3MF, 10.0));
  assert(state.finish(7, PET_DROP_ACCEPTED, 20.0));
  assert(state.sample(22.069, false).fragment_count == 0);
  assert(state.sample(22.071, false).fragment_count == 12);
  assert(state.sample(22.30, false).faller_progress > 0.49f);
  assert(state.sample(22.30, false).faller_progress < 0.51f);
  const PetDropSnapshot crossing = state.sample(23.772, false);
  assert(crossing.deliver_once);
  assert(crossing.absorption_progress == 0.0f);
  assert(!state.sample(23.773, false).deliver_once);
  assert(state.sample(24.047, false).fragment_count == 12);
  assert(state.sample(24.049, false).fragment_count == 0);
  const PetDropSnapshot jet_mid = state.sample(24.222, false);
  assert(jet_mid.absorption_progress > 0.49f);
  assert(jet_mid.absorption_progress < 0.51f);
  assert(state.sample(24.671, false).phase == PetDropPhase::kSwallow);
  assert(state.sample(24.673, false).phase == PetDropPhase::kIdle);
}

static void stale_ack_and_reduced_motion_are_bounded() {
  PetDropState state;
  assert(state.begin_wait(12, {0.6f, 0.5f}, PET_FILE_GCODE, 1.0));
  assert(!state.finish(11, PET_DROP_ACCEPTED, 2.0));
  assert(state.sample(20.0, false).phase == PetDropPhase::kImportPending);
  assert(state.finish(12, PET_DROP_ACCEPTED, 21.0));
  const PetDropSnapshot reduced = state.sample(21.075, true);
  assert(reduced.reduced_fade > 0.49f && reduced.reduced_fade < 0.51f);
  assert(reduced.fragment_count == 0);
  assert(reduced.absorption_progress == 0.0f);
  assert(state.sample(21.151, true).phase == PetDropPhase::kIdle);
}

static void rejected_import_recoils_without_delivery() {
  PetDropState state;
  assert(state.begin_wait(4, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  assert(state.finish(4, PET_DROP_REJECTED, 2.0));
  const PetDropSnapshot recoil = state.sample(2.18, false);
  assert(recoil.phase == PetDropPhase::kImportRejected);
  assert(recoil.error_progress > 0.42f && recoil.error_progress < 0.44f);
  assert(recoil.fragment_count == 0);
  assert(!recoil.deliver_once);
  assert(recoil.absorption_progress == 0.0f);
  PetImpactState impact;
  if (recoil.deliver_once) {
    impact.strike(2.18, recoil.origin, recoil.file_kind);
  }
  const PetImpactSnapshot no_impact = impact.sample(2.18);
  assert(!no_impact.active);
  assert(no_impact.impact_level == 0.0f);
  assert(no_impact.feed_strength == 0.0f);
  assert(state.sample(2.421, false).phase == PetDropPhase::kIdle);
}

static void cancellation_clears_every_visual_without_delivery() {
  PetDropState state;
  assert(state.begin_wait(8, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  state.cancel();
  const PetDropSnapshot cancelled = state.sample(100.0, false);
  assert(cancelled.phase == PetDropPhase::kIdle);
  assert(cancelled.fragment_count == 0);
  assert(cancelled.absorption_progress == 0.0f);
  assert(!cancelled.deliver_once);
  PetImpactState impact;
  if (cancelled.deliver_once) {
    impact.strike(100.0, cancelled.origin, cancelled.file_kind);
  }
  const PetImpactSnapshot no_impact = impact.sample(100.0);
  assert(!no_impact.active);
  assert(no_impact.impact_level == 0.0f);
  assert(no_impact.feed_strength == 0.0f);
  assert(!state.finish(8, PET_DROP_ACCEPTED, 101.0));
}

static void import_wait_requires_idle_and_a_nonzero_generation() {
  PetDropState state;
  assert(!state.begin_wait(0, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  assert(state.begin_wait(1, {0.8f, 0.5f}, PET_FILE_3MF, 1.0));
  assert(!state.begin_wait(2, {0.8f, 0.5f}, PET_FILE_GCODE, 2.0));
  state.cancel();
  assert(state.begin_wait(2, {0.8f, 0.5f}, PET_FILE_GCODE, 3.0));
}

static void motion_policy_is_latched_without_losing_delivery() {
  PetDropState standard;
  assert(standard.begin_wait(21, {0.72f, 0.44f}, PET_FILE_3MF, 1.0));
  assert(standard.finish(21, PET_DROP_ACCEPTED, 2.0));
  assert(standard.sample(2.0, false).phase == PetDropPhase::kSwallow);
  const PetDropSnapshot toggled = standard.sample(2.20, true);
  assert(toggled.phase == PetDropPhase::kSwallow);
  assert(toggled.faller_progress > 0.04f);
  assert(toggled.reduced_fade == 0.0f);
  assert(!toggled.deliver_once);
  assert(standard.sample(5.772, true).deliver_once);
  assert(standard.sample(6.673, true).phase == PetDropPhase::kIdle);
  assert(standard.begin_wait(22, {0.6f, 0.5f}, PET_FILE_GCODE, 7.0));

  PetDropState reduced;
  assert(reduced.begin_wait(31, {0.72f, 0.44f}, PET_FILE_3MF, 10.0));
  assert(reduced.finish(31, PET_DROP_ACCEPTED, 11.0));
  const PetDropSnapshot first = reduced.sample(11.0, true);
  assert(first.deliver_once);
  assert(first.reduced_fade == 0.0f);
  assert(reduced.sample(11.151, false).phase == PetDropPhase::kIdle);
}

static void render_uniforms_follow_the_latched_drop_motion_policy() {
  PetDropState standard;
  assert(standard.begin_wait(41, {0.72f, 0.44f}, PET_FILE_3MF, 1.0));
  assert(standard.finish(41, PET_DROP_ACCEPTED, 2.0));
  const PetDropRenderSnapshot standard_start =
      standard.sample_render(2.0, false);
  assert(!standard_start.reduce_motion);
  const PetDropRenderSnapshot standard_after_toggle =
      standard.sample_render(2.20, true);
  assert(!standard_after_toggle.reduce_motion);
  PetRenderUniforms standard_uniforms = {};
  PetApplyDropMotionUniforms(standard_after_toggle, standard_uniforms);
  assert(standard_uniforms.reduce_motion == 0);
  assert(standard_uniforms.drop_progress ==
         standard_after_toggle.drop.faller_progress);
  assert(standard_uniforms.drop_progress > 0.04f);

  const PetDropRenderSnapshot jet_mid =
      standard.sample_render(6.222, true);
  assert(!jet_mid.reduce_motion);
  PetRenderUniforms jet_uniforms = {};
  PetApplyDropMotionUniforms(jet_mid, jet_uniforms);
  assert(jet_uniforms.reduce_motion == 0);
  assert(jet_mid.drop.absorption_progress > 0.49f);
  assert(jet_mid.drop.absorption_progress < 0.51f);

  PetDropState reduced;
  assert(reduced.begin_wait(51, {0.72f, 0.44f}, PET_FILE_GCODE, 10.0));
  assert(reduced.finish(51, PET_DROP_ACCEPTED, 11.0));
  const PetDropRenderSnapshot reduced_start =
      reduced.sample_render(11.0, true);
  assert(reduced_start.reduce_motion);
  const PetDropRenderSnapshot reduced_after_toggle =
      reduced.sample_render(11.075, false);
  assert(reduced_after_toggle.reduce_motion);
  PetRenderUniforms reduced_uniforms = {};
  PetApplyDropMotionUniforms(reduced_after_toggle, reduced_uniforms);
  assert(reduced_uniforms.reduce_motion == 1);
  assert(reduced_uniforms.drop_progress ==
         reduced_after_toggle.drop.reduced_fade);
  assert(reduced_uniforms.drop_progress > 0.49f);
  assert(reduced_uniforms.drop_progress < 0.51f);
  assert(reduced_after_toggle.drop.absorption_progress == 0.0f);
}

static void impact_and_afterglow_use_the_reference_lifetimes() {
  PetImpactState impact;
  assert(!impact.sample(1.0).active);

  impact.strike(2.0, {0.72f, 0.44f}, PET_FILE_3MF);
  const PetImpactSnapshot attack = impact.sample(2.06);
  assert(attack.active);
  assert(attack.impact_level > 0.0f);
  assert(attack.feed_strength > 0.9f);

  const PetImpactSnapshot impact_tail = impact.sample(5.999);
  assert(impact_tail.active);
  assert(impact_tail.impact_level > 0.0f);
  const PetImpactSnapshot feed_tail = impact.sample(6.001);
  assert(feed_tail.active);
  assert(feed_tail.impact_level == 0.0f);
  assert(feed_tail.feed_strength > 0.0f);
  assert(impact.sample(15.999).active);
  assert(!impact.sample(16.001).active);

  impact.clear();
  const PetImpactSnapshot cleared = impact.sample(100.0);
  assert(!cleared.active);
  assert(cleared.impact_level == 0.0f);
  assert(cleared.feed_strength == 0.0f);
}

static void external_file_states_keep_auto_fps_at_sixty() {
  assert(PetResolveRenderActivity(PetRenderActivity::kIdle,
                                  PetDropPhase::kImportPending, false) ==
         PetRenderActivity::kSignal);
  assert(PetResolveRenderActivity(PetRenderActivity::kIdle,
                                  PetDropPhase::kSwallow, false) ==
         PetRenderActivity::kSignal);
  assert(PetResolveRenderActivity(PetRenderActivity::kIdle,
                                  PetDropPhase::kImportRejected, false) ==
         PetRenderActivity::kSignal);
  assert(PetResolveRenderActivity(PetRenderActivity::kIdle,
                                  PetDropPhase::kIdle, true) ==
         PetRenderActivity::kSignal);
  assert(PetResolveRenderActivity(PetRenderActivity::kIdle,
                                  PetDropPhase::kIdle, false) ==
         PetRenderActivity::kIdle);
}

int main() {
  center_selects_display_and_maps_negative_coordinates();
  exact_display_seam_keeps_current_display();
  full_display_capture_never_changes_with_center();
  moving_center_does_not_reconfigure_full_display_capture();
  adjacent_panes_receive_complementary_seam_centers();
  disconnected_center_recovers_only_when_off_every_display();
  visual_style_values_stay_within_stable_native_contract();
  approved_geometry_uses_a_small_circular_core();
  size_changes_radius_without_changing_the_full_display_pane();
  large_sizes_keep_logical_geometry_but_cap_the_drawable();
  centered_capture_maps_the_panel_into_the_middle_five_eighths();
  left_edge_capture_does_not_stretch_the_desktop();
  other_screen_edges_preserve_the_clipped_panel_mapping();
  pet_visual_and_core_hit_target_windows_share_lifecycle();
  capture_region_is_bounded_and_uses_retina_pixels();
  greatest_intersection_selects_the_new_display();
  disconnected_panel_returns_to_primary_with_safe_inset();
  missing_saved_display_uses_system_primary_not_focused_screen();
  backing_scale_updates_drawable_pixels_without_logical_resize();
  reverse_main_queue_delivery_discards_the_older_apply();
  drag_persistence_is_emitted_only_once_on_mouse_up();
  live_capture_reconfiguration_ignores_fps_and_pending_only_updates();
  same_display_drag_reaims_without_restarting_capture();
  missing_live_frame_falls_back_without_a_black_capture_disc();
  capture_restart_coalesces_to_the_latest_drag_target();
  stale_reaim_token_cannot_clear_a_new_generation();
  native_failure_reason_is_published_once_until_an_allowed_retry();
  capture_policy_excludes_media_and_retains_only_the_newest_frame();
  metal_unavailable_disables_real_capture_and_requests_stop();
  stopped_capture_rejects_late_frame_delivery();
  stale_or_suspended_capture_frames_are_never_reused();
  desktop_pet_stays_below_normal_application_windows();
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
  a_drop_session_requires_the_same_generation_path_and_core_point();
  import_wait_never_crosses_before_acknowledgment();
  accepted_import_runs_the_complete_reference_timing();
  stale_ack_and_reduced_motion_are_bounded();
  rejected_import_recoils_without_delivery();
  cancellation_clears_every_visual_without_delivery();
  import_wait_requires_idle_and_a_nonzero_generation();
  motion_policy_is_latched_without_losing_delivery();
  render_uniforms_follow_the_latched_drop_motion_policy();
  impact_and_afterglow_use_the_reference_lifetimes();
  external_file_states_keep_auto_fps_at_sixty();
}
