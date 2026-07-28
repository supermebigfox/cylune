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

struct PetPendingInstance {
  float2 center;
  float diameter;
  float padding;
};

struct PetPendingVertexOutput {
  float4 position [[position]];
  float2 local;
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
  const float visual_time =
      uniforms.reduce_motion != 0u ? 0.0 : uniforms.time_seconds;
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
      0.5 + 0.5 * sin(angle * 3.0 - visual_time * 1.4);
  const float disk_radius = photon_radius + 0.095 + 0.025 * disk_wave;
  const float accretion =
      ring_mask(radius, disk_radius, 0.032) *
      smoothstep(0.12, 0.92, abs(p.y) + 0.17);
  const float3 ring_color =
      spectral_ring(angle, visual_time) *
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
        uniforms.reduce_motion != 0u
            ? 0.72
            : 0.54 + uniforms.success_progress * 0.34;
    const float pulse_opacity =
        uniforms.reduce_motion != 0u
            ? 1.0 - abs(uniforms.success_progress * 2.0 - 1.0)
            : 1.0 - uniforms.success_progress;
    const float pulse =
        ring_mask(radius, pulse_radius, 0.022) * pulse_opacity;
    color.rgb += float3(0.16, 1.0, 0.43) * pulse * 1.4;
    color.a = max(color.a, pulse);
  }
  if (uniforms.error_progress > 0.0) {
    const float ripple_radius =
        uniforms.reduce_motion != 0u
            ? 0.76
            : 0.48 + uniforms.error_progress * 0.48;
    const float ripple_opacity =
        uniforms.reduce_motion != 0u
            ? 1.0 - abs(uniforms.error_progress * 2.0 - 1.0)
            : 1.0 - uniforms.error_progress;
    const float ripple =
        ring_mask(radius, ripple_radius, 0.025) * ripple_opacity;
    color.rgb += float3(1.0, 0.07, 0.10) * ripple * 1.5;
    color.a = max(color.a, ripple);
  }

  const float edge_fade = 1.0 - smoothstep(0.94, 0.985, radius);
  color.a *= edge_fade;
  color.rgb *= color.a;
  return color;
}

vertex PetPendingVertexOutput pet_pending_vertex(
    uint vertex_id [[vertex_id]], uint instance_id [[instance_id]],
    const device PetPendingInstance *instances [[buffer(1)]],
    constant PetUniforms &uniforms [[buffer(2)]]) {
  const float2 corners[6] = {
      float2(-1.0, -1.0), float2(1.0, -1.0), float2(-1.0, 1.0),
      float2(-1.0, 1.0),  float2(1.0, -1.0),  float2(1.0, 1.0),
  };
  const PetPendingInstance instance = instances[instance_id];
  const float orbit =
      uniforms.reduce_motion != 0u ? 0.0 : uniforms.time_seconds * 0.55;
  const float cosine = cos(orbit);
  const float sine = sin(orbit);
  const float2 center =
      float2(instance.center.x * cosine - instance.center.y * sine,
             instance.center.x * sine + instance.center.y * cosine);
  const float2 local = corners[vertex_id];
  const float2 p = center + local * instance.diameter * 0.5;
  PetPendingVertexOutput output;
  output.position = float4(p.x, -p.y, 0.0, 1.0);
  output.local = local;
  return output;
}

fragment float4 pet_pending_fragment(
    PetPendingVertexOutput input [[stage_in]]) {
  const float alpha =
      1.0 - smoothstep(0.68, 1.0, length(input.local));
  return float4(float3(1.0, 0.56, 0.06) * alpha * 1.2, alpha);
}
