/*
 Black-hole lensing logic was modified and ported to Metal for this project
 from ideas in cabbagehao/blackhole-timer and s0xDk/ghostty-blackhole.
 Both sources are MIT licensed:
 Copyright (c) 2026 s13k <s13k@pm.me>
 Full license text and modification notes: THIRD_PARTY_NOTICES.md
 Unrelated timer, terminal, and product code was not copied.
*/

#include <metal_stdlib>
using namespace metal;

struct PetVertexOutput {
  float4 position [[position]];
  float2 uv;
};

struct PetUniforms {
  float2 viewport_px;
  float time_seconds;
  float lens_strength;
  float hover_progress;
  float swallow_progress;
  float success_progress;
  float error_progress;
  uint pending_count;
  uint mode;
  uint reduce_motion;
};

vertex PetVertexOutput pet_vertex(uint vertex_id [[vertex_id]]) {
  const float2 positions[3] = {
      float2(-1.0, -1.0), float2(3.0, -1.0), float2(-1.0, 3.0)};
  const float2 coordinates[3] = {
      float2(0.0, 1.0), float2(2.0, 1.0), float2(0.0, -1.0)};
  PetVertexOutput output;
  output.position = float4(positions[vertex_id], 0.0, 1.0);
  output.uv = coordinates[vertex_id];
  return output;
}

static float ring_mask(float radius, float center, float width) {
  return 1.0 - smoothstep(width, width * 2.0, abs(radius - center));
}

static float3 spectral_ring(float angle, float pulse) {
  const float3 cyan = float3(0.10, 0.82, 1.00);
  const float3 violet = float3(0.67, 0.22, 1.00);
  const float3 rose = float3(1.00, 0.22, 0.50);
  const float phase = fract(angle / 6.2831853 + 0.5 + pulse * 0.025);
  return phase < 0.5 ? mix(cyan, violet, phase * 2.0)
                     : mix(violet, rose, (phase - 0.5) * 2.0);
}

static float pending_dot(float2 p, uint index, uint count, float time_seconds,
                         bool reduce_motion) {
  uint ring_index = 0;
  uint ring_start = 0;
  uint capacity = 8;
  while (index >= ring_start + capacity) {
    ring_start += capacity;
    ring_index += 1;
    capacity = 8 + ring_index * 4;
  }
  const uint remaining = count - ring_start;
  const uint dots_in_ring = min(remaining, capacity);
  uint ring_count = 0;
  uint left = count;
  while (left > 0) {
    left = left > (8 + ring_count * 4)
               ? left - (8 + ring_count * 4)
               : 0;
    ring_count += 1;
  }
  const float radius =
      ring_count <= 1 ? 0.78
                      : 0.62 + 0.28 * float(ring_index) /
                                   float(max(1u, ring_count - 1));
  const float stagger = (ring_index & 1u) == 0u ? 0.0 : 0.5;
  const float orbit = reduce_motion ? 0.0 : time_seconds * 0.55;
  const float angle = -1.5707963 +
                      6.2831853 *
                          (float(index - ring_start) + stagger) /
                          float(max(1u, dots_in_ring)) +
                      orbit * ((ring_index & 1u) == 0u ? 1.0 : -1.0);
  const float2 center = float2(cos(angle), sin(angle)) * radius;
  return 1.0 - smoothstep(0.025, 0.042, distance(p, center));
}

fragment float4 pet_fragment(PetVertexOutput input [[stage_in]],
                             texture2d<float> capture [[texture(0)]],
                             sampler capture_sampler [[sampler(0)]],
                             constant PetUniforms &uniforms [[buffer(0)]]) {
  float2 p = (input.uv - 0.5) * 2.0;
  const float motion = uniforms.reduce_motion == 0u ? 1.0 : 0.0;
  const float animation_scale =
      1.0 + uniforms.hover_progress * 0.12 * motion -
      sin(uniforms.swallow_progress * 3.1415927) * 0.18 * motion;
  p /= max(0.72, animation_scale);
  const float radius = length(p);
  if (radius > 0.985) {
    return float4(0.0);
  }

  const float angle = atan2(p.y, p.x);
  const float event_horizon = 0.355;
  const float photon_radius = 0.505;
  const float lens_outer = 0.94;
  float4 color = float4(0.0);

  if (uniforms.mode == 0u && radius < lens_outer &&
      radius > event_horizon) {
    const float safe_radius = max(radius, event_horizon + 0.015);
    const float bend =
        uniforms.lens_strength * 0.055 /
        max(0.055, safe_radius - event_horizon);
    const float2 direction = p / max(radius, 0.001);
    const float2 bent = p + direction * bend;
    const float2 capture_uv =
        clamp(0.5 + bent * (0.5 / 1.24), float2(0.002), float2(0.998));
    color = capture.sample(capture_sampler, capture_uv);
    color.a = smoothstep(lens_outer, lens_outer - 0.055, radius);
  }

  const float shadow =
      1.0 - smoothstep(event_horizon - 0.015, event_horizon + 0.025, radius);
  color.rgb *= 1.0 - shadow;
  color.a = max(color.a, shadow);

  const float photon =
      ring_mask(radius, photon_radius, 0.018) *
      (0.76 + uniforms.hover_progress * 0.42);
  const float disk_wave =
      0.5 + 0.5 * sin(angle * 3.0 - uniforms.time_seconds * 1.4);
  const float disk_radius = photon_radius + 0.095 + 0.025 * disk_wave;
  const float accretion =
      ring_mask(radius, disk_radius, 0.032) *
      smoothstep(0.12, 0.92, abs(p.y) + 0.17);
  const float3 ring_color =
      spectral_ring(angle, uniforms.time_seconds) *
      (1.0 + uniforms.hover_progress * 0.35);
  color.rgb += ring_color * photon * 1.35;
  color.rgb += mix(float3(1.0, 0.25, 0.06), ring_color, disk_wave) *
               accretion * 0.82;
  color.a = max(color.a, max(photon, accretion));

  if (uniforms.swallow_progress > 0.0) {
    const float transition =
        sin(uniforms.swallow_progress * 3.1415927);
    const float flash = uniforms.reduce_motion == 0u
                            ? pow(max(0.0, 1.0 -
                                               abs(uniforms.swallow_progress -
                                                   0.5) *
                                                   8.0),
                                  2.0)
                            : 0.0;
    color.rgb += float3(1.0, 0.58, 0.08) * transition * 0.55;
    color.rgb += float3(1.0) * flash * 0.75;
    color.a = max(color.a, transition * 0.6);
  }

  if (uniforms.success_progress > 0.0) {
    const float pulse_radius =
        0.54 + uniforms.success_progress * 0.34;
    const float pulse = ring_mask(radius, pulse_radius, 0.022) *
                        (1.0 - uniforms.success_progress);
    color.rgb += float3(0.16, 1.0, 0.43) * pulse * 1.4;
    color.a = max(color.a, pulse);
  }
  if (uniforms.error_progress > 0.0) {
    const float ripple_radius =
        0.48 + uniforms.error_progress * 0.48;
    const float ripple = ring_mask(radius, ripple_radius, 0.025) *
                         (1.0 - uniforms.error_progress);
    color.rgb += float3(1.0, 0.07, 0.10) * ripple * 1.5;
    color.a = max(color.a, ripple);
  }

  for (uint index = 0; index < uniforms.pending_count; ++index) {
    const float dot = pending_dot(
        p, index, uniforms.pending_count, uniforms.time_seconds,
        uniforms.reduce_motion != 0u);
    color.rgb += float3(1.0, 0.56, 0.06) * dot * 1.2;
    color.a = max(color.a, dot);
  }

  const float edge_fade = 1.0 - smoothstep(0.94, 0.985, radius);
  color.a *= edge_fade;
  color.rgb *= color.a;
  return color;
}
