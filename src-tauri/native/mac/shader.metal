/*
 Black-hole optics are a Metal port of rrrjqy66/BlackHoleTrash
 commit 229d93213cd3e57364b4c6655cfb2c75b7ea4d18 (MIT).
 Original copyright: Copyright (c) 2026 GreenScreen410.
 Fusion material parameters are adapted from cabbagehao/blackhole-timer
 commit f3cc9cc349540ad6d274cd8074cf050b9b0c0200 (MIT).
 This application replaces recycling with acknowledged local import.
 Full notices: THIRD_PARTY_NOTICES.md.
*/

#include <metal_stdlib>
using namespace metal;

constant float kLensDepth = 13.0f;
constant int kGeodesicSteps = 48;
constant float kCriticalImpact = 2.5980762f;
constant uint kPetVisualStyleGargantua = 0u;
constant uint kPetVisualStyleFusion = 1u;

struct PetVertexOutput {
  float4 position [[position]];
  float2 uv;
};

struct PetUniforms {
  float2 viewport_px;
  float2 capture_origin_uv;
  float2 capture_extent_uv;
  float time_seconds;
  float hole_radius_uv;
  float temperature;
  float inclination;
  float roll;
  float disk_inner;
  float disk_outer;
  float disk_opacity;
  float doppler;
  float beaming;
  float gain;
  float contrast;
  float wind;
  float speed;
  float exposure;
  float stars;
  float spin;
  float spin_phase;
  float2 drop_origin_uv;
  float drop_progress;
  float absorption_progress;
  float success_progress;
  float error_progress;
  uint pending_count;
  uint mode;
  uint reduce_motion;
  uint drop_phase;
  uint file_kind;
  uint visual_style;
  uint padding[2];
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
      float2(-1.0f, -1.0f), float2(3.0f, -1.0f),
      float2(-1.0f, 3.0f)};
  const float2 coordinates[3] = {
      float2(0.0f, 1.0f), float2(2.0f, 1.0f),
      float2(0.0f, -1.0f)};
  PetVertexOutput output;
  output.position = float4(positions[vertex_id], 0.0f, 1.0f);
  output.uv = coordinates[vertex_id];
  return output;
}

static float2 rotate2d(float2 value, float angle) {
  const float cosine = cos(angle);
  const float sine = sin(angle);
  return float2(cosine * value.x - sine * value.y,
                sine * value.x + cosine * value.y);
}

static float gmod(float value, float period) {
  return value - period * floor(value / period);
}

static float2 mirror_uv(float2 uv) {
  const float2 mirrored = uv - 2.0f * floor(uv / 2.0f);
  return 1.0f - abs(1.0f - mirrored);
}

static float hash21(float2 input) {
  float2 value = fract(input * float2(234.34f, 435.345f));
  value += dot(value, value + 34.23f);
  return fract(value.x * value.y);
}

static float wrapped_value_noise(float2 point, float period_y) {
  const float2 cell = floor(point);
  float2 fraction = fract(point);
  fraction = fraction * fraction * (3.0f - 2.0f * fraction);
  const float y0 = gmod(cell.y, period_y);
  const float y1 = gmod(cell.y + 1.0f, period_y);
  return mix(
      mix(hash21(float2(cell.x, y0)),
          hash21(float2(cell.x + 1.0f, y0)), fraction.x),
      mix(hash21(float2(cell.x, y1)),
          hash21(float2(cell.x + 1.0f, y1)), fraction.x),
      fraction.y);
}

float3 blackbody(float temperature) {
  const float scaled =
      clamp(temperature, 1500.0f, 40000.0f) / 100.0f;
  float red = 1.0f;
  if (scaled > 66.0f) {
    red = clamp(1.292936f * pow(scaled - 60.0f, -0.1332047f),
                0.0f, 1.0f);
  }
  float green = 0.0f;
  if (scaled <= 66.0f) {
    green = clamp(0.3900816f * log(scaled) - 0.6318414f,
                  0.0f, 1.0f);
  } else {
    green = clamp(1.1298909f * pow(scaled - 60.0f, -0.0755148f),
                  0.0f, 1.0f);
  }
  float blue = 1.0f;
  if (scaled < 66.0f) {
    if (scaled <= 19.0f) {
      blue = 0.0f;
    } else {
      blue = clamp(0.5432068f * log(scaled - 10.0f) - 1.1962540f,
                   0.0f, 1.0f);
    }
  }
  return float3(red, green, blue);
}

static float3 star_field(float3 direction,
                         constant PetUniforms &uniforms) {
  const float2 sphere =
      float2(atan2(direction.x, -direction.z),
             asin(clamp(direction.y, -1.0f, 1.0f)));
  const float2 grid = sphere * 40.0f;
  const float2 cell = floor(grid);
  const float hash = hash21(cell);
  if (hash < 0.92f) {
    return float3(0.0f);
  }
  const float2 local = fract(grid) - 0.5f;
  const float2 offset =
      (float2(hash21(cell + 17.3f), hash21(cell + 31.7f)) - 0.5f) *
      0.7f;
  const float spark =
      smoothstep(0.10f, 0.0f, length(local - offset));
  const float visual_time =
      uniforms.reduce_motion == 0u ? uniforms.time_seconds : 0.0f;
  const float twinkle =
      0.7f +
      0.3f *
          sin(visual_time * (0.5f + 2.0f * hash21(cell + 5.1f)) +
              40.0f * hash);
  const float3 tint =
      mix(float3(1.0f, 0.82f, 0.60f),
          float3(0.75f, 0.85f, 1.0f), hash21(cell + 2.9f));
  return tint * spark * twinkle * ((hash - 0.92f) / 0.08f);
}

float3 captured_background(float2 local_uv,
                           texture2d<float> capture,
                           sampler capture_sampler,
                           constant PetUniforms &uniforms) {
  if (uniforms.mode != 0u) {
    return float3(0.0f);
  }
  const float2 capture_uv =
      mirror_uv(uniforms.capture_origin_uv +
                local_uv * uniforms.capture_extent_uv);
  return capture.sample(capture_sampler, capture_uv, level(0.0f)).rgb;
}

float4 shade_crossing(float3 position, float3 velocity,
                      float3 normal, float3 disk_axis,
                      constant PetUniforms &uniforms,
                      float transmittance) {
  const float inner = max(uniforms.disk_inner, 1.6f);
  const float outer = max(uniforms.disk_outer, inner + 0.5f);
  const float radius = length(position);
  if (radius <= inner || radius >= outer) {
    return float4(0.0f);
  }

  const bool fusion =
      uniforms.visual_style == kPetVisualStyleFusion;
  const float band =
      fusion
          ? smoothstep(inner, inner * 1.12f, radius) *
                (1.0f -
                 smoothstep(outer * 0.82f, outer, radius))
          : smoothstep(inner, inner * 1.25f, radius) *
                (1.0f -
                 smoothstep(outer * 0.70f, outer, radius));
  const float phi = atan2(dot(position, disk_axis), position.x);
  const float turns = phi / 6.2831853f;
  const float kepler = pow(inner / radius, 1.5f);
  const float local_gravity =
      sqrt(max(1.0f - 1.5f / radius, 0.02f));
  const float direction = uniforms.speed < 0.0f ? -1.0f : 1.0f;
  const float visual_time =
      uniforms.reduce_motion == 0u ? uniforms.time_seconds : 0.0f;
  const float swirl =
      radius * uniforms.wind * 0.12f -
      visual_time * kepler * abs(uniforms.speed) * local_gravity *
          direction -
      uniforms.spin_phase * kepler;
  float streaks =
      wrapped_value_noise(
          float2(radius * 2.8f,
                 turns * 19.0f + swirl * 3.0f),
          19.0f) *
          0.65f +
      wrapped_value_noise(
          float2(radius, turns * 9.0f + swirl * 1.5f + 7.0f),
          9.0f) *
          0.35f;
  streaks = 0.35f + uniforms.contrast * streaks * streaks;

  const float3 gas_direction =
      normalize(cross(normal, position)) * direction;
  const float beta =
      clamp(rsqrt(max(2.0f * (radius - 1.0f), 0.2f)),
            0.0f, 0.99f);
  float shift =
      local_gravity /
      max(1.0f + beta * dot(gas_direction, velocity), 0.05f);
  shift = mix(1.0f, shift, uniforms.doppler);

  const float profile_base =
      max(1.0f - sqrt(inner / radius), 0.0f);
  const float temperature_profile =
      pow(inner / radius, 0.75f) * pow(profile_base, 0.25f) /
      0.488f;
  float3 thermal =
      blackbody(uniforms.temperature * temperature_profile * shift);
  if (fusion) {
    thermal =
        mix(thermal, float3(1.0f, 0.91f, 0.70f), 0.12f);
  }
  const float boost = pow(shift, uniforms.beaming);
  float density = band * streaks;
  if (fusion) {
    density = band * (0.62f + 0.58f * streaks);
  }
  const float3 emission =
      transmittance * thermal *
      (uniforms.gain * 2.2f * density * temperature_profile *
       temperature_profile * boost);
  return float4(emission, density);
}

static float drag_twist(float impact, float spin,
                        constant PetUniforms &uniforms) {
  const float active_phase =
      abs(spin) >= 0.005f ? uniforms.spin_phase : 0.0f;
  return (1.3f * spin + active_phase) /
         (1.0f +
          0.8f * pow(impact / kCriticalImpact, 2.0f));
}

float3 weak_deflection_background(float2 p, float b,
                                  texture2d<float> capture,
                                  sampler capture_sampler,
                                  constant PetUniforms &uniforms) {
  const float aspect =
      uniforms.viewport_px.x / max(uniforms.viewport_px.y, 1.0f);
  const float2 center = float2(0.5f);
  const float radius = length(p);
  const float hole_radius = max(uniforms.hole_radius_uv, 1e-4f);
  const float world_scale = kCriticalImpact / hole_radius;
  const float inner = max(uniforms.disk_inner, 1.6f);
  const float outer = max(uniforms.disk_outer, inner + 0.5f);
  const float maximum_impact = outer + 3.0f;
  const float camera_depth = max(14.0f, outer + 5.0f);
  const float window =
      exp(-pow(radius / (7.0f * hole_radius), 2.0f));
  const float finite_camera =
      camera_depth * rsqrt(camera_depth * camera_depth + b * b);
  const float deflection =
      (2.0f / (world_scale * world_scale)) /
      max(radius, 1e-4f) *
      (1.29f * finite_camera + 0.07f) *
      max(kLensDepth - 2.14f * finite_camera + 0.75f, 0.0f) *
      window;
  const float2 direction = p / max(radius, 1e-5f);
  const float aberration =
      0.035f * smoothstep(1.0f, 2.0f, b / maximum_impact);
  const float spin_direction =
      uniforms.speed < 0.0f ? -1.0f : 1.0f;
  const float twist =
      drag_twist(b, uniforms.spin * spin_direction, uniforms);
  const float2 sample_red =
      rotate2d(p - direction * deflection * (1.0f - aberration),
               twist);
  const float2 sample_green =
      rotate2d(p - direction * deflection, twist);
  const float2 sample_blue =
      rotate2d(p - direction * deflection * (1.0f + aberration),
               twist);
  float3 color =
      float3(captured_background(
                 center + sample_red / float2(aspect, 1.0f),
                 capture, capture_sampler, uniforms)
                 .r,
             captured_background(
                 center + sample_green / float2(aspect, 1.0f),
                 capture, capture_sampler, uniforms)
                 .g,
             captured_background(
                 center + sample_blue / float2(aspect, 1.0f),
                 capture, capture_sampler, uniforms)
                 .b);
  const float2 projected =
      rotate2d(float2(p.x, -p.y), uniforms.roll) * world_scale;
  const float3 ray =
      normalize(float3(-(projected / max(b, 1e-5f)) *
                           (2.0f / max(b, 1e-5f)),
                       -1.0f));
  color += star_field(ray, uniforms) * uniforms.stars * window;
  return color;
}

float3 trace_schwarzschild(float2 p,
                          texture2d<float> capture,
                          sampler capture_sampler,
                          constant PetUniforms &uniforms,
                          thread float &alpha) {
  const float aspect =
      uniforms.viewport_px.x / max(uniforms.viewport_px.y, 1.0f);
  const float2 center = float2(0.5f);
  const float radius = length(p);
  const float inner = max(uniforms.disk_inner, 1.6f);
  const float outer = max(uniforms.disk_outer, inner + 0.5f);
  const float hole_radius = max(uniforms.hole_radius_uv, 1e-4f);
  const float world_scale = kCriticalImpact / hole_radius;
  const float2 projected =
      rotate2d(float2(p.x, -p.y), uniforms.roll) * world_scale;
  const float impact = length(projected);
  const float camera_depth = max(14.0f, outer + 5.0f);
  const float window =
      exp(-pow(radius / (7.0f * hole_radius), 2.0f));

  float3 position = float3(projected, camera_depth);
  float3 velocity = float3(0.0f, 0.0f, -1.0f);
  const float angular_momentum_squared =
      dot(projected, projected);
  const float cosine = cos(uniforms.inclination);
  const float sine = sin(uniforms.inclination);
  const float3 normal = float3(0.0f, sine, cosine);
  const float3 disk_axis = float3(0.0f, cosine, -sine);
  const float spin_direction =
      uniforms.speed < 0.0f ? -1.0f : 1.0f;

  float3 emitted = float3(0.0f);
  float transmittance = 1.0f;
  bool captured = false;
  float previous_side = dot(position, normal);
  float3 previous_position = position;

  for (int step = 0; step < kGeodesicSteps; ++step) {
    float radius_squared = dot(position, position);
    if (radius_squared < 1.0f) {
      captured = true;
      break;
    }
    if (position.z < -camera_depth && velocity.z < 0.0f) {
      break;
    }
    if (radius_squared >
        4.0f * camera_depth * camera_depth) {
      break;
    }
    float world_radius = sqrt(radius_squared);
    const float delta =
        clamp(0.16f * world_radius, 0.03f, 1.5f);
    float3 acceleration =
        -1.5f * angular_momentum_squared * position /
        (radius_squared * radius_squared * world_radius);
    velocity += acceleration * (0.5f * delta);
    position += velocity * delta;
    radius_squared = dot(position, position);
    world_radius = sqrt(radius_squared);
    acceleration =
        -1.5f * angular_momentum_squared * position /
        (radius_squared * radius_squared * world_radius);
    velocity += acceleration * (0.5f * delta);

    const float side = dot(position, normal);
    if (side * previous_side < 0.0f &&
        transmittance > 0.02f) {
      const float crossing_fraction =
          previous_side / (previous_side - side);
      const float3 crossing_position =
          mix(previous_position, position, crossing_fraction);
      const float4 crossing =
          shade_crossing(crossing_position, normalize(velocity),
                         normal, disk_axis, uniforms,
                         transmittance);
      emitted += crossing.rgb;
      transmittance *=
          1.0f -
          clamp(uniforms.disk_opacity * crossing.a, 0.0f, 1.0f);
    }
    previous_side = side;
    previous_position = position;
  }
  if (!captured && dot(position, position) < 4.0f) {
    captured = true;
  }

  float3 background = float3(0.0f);
  if (!captured) {
    const float3 direction = normalize(velocity);
    background +=
        star_field(direction, uniforms) * uniforms.stars * window;
    if (direction.z < -0.05f) {
      const float plane_distance =
          (-kLensDepth - position.z) / direction.z;
      const float3 plane_position =
          position + direction * plane_distance;
      const float2 unrolled =
          rotate2d(plane_position.xy, -uniforms.roll) /
          world_scale;
      float2 sample_position =
          float2(unrolled.x, -unrolled.y);
      sample_position =
          rotate2d(
              sample_position,
              drag_twist(impact,
                         uniforms.spin * spin_direction,
                         uniforms));
      const float2 local_uv =
          center +
          (p + (sample_position - p) * window) /
              float2(aspect, 1.0f);
      const float toward =
          smoothstep(0.05f, 0.35f, -direction.z);
      background +=
          captured_background(local_uv, capture, capture_sampler,
                              uniforms) *
          toward;
    }
  }

  const float3 disk =
      1.0f - exp(-emitted * uniforms.exposure);
  const float3 color = background * transmittance + disk;
  if (uniforms.mode == 0u || captured) {
    alpha = 1.0f;
  } else {
    alpha = clamp(
        max(1.0f - transmittance,
            max(color.r, max(color.g, color.b))),
        0.0f, 1.0f);
  }
  return color;
}

static float ring_mask(float radius, float center, float width) {
  return 1.0f -
         smoothstep(width, width * 2.0f, abs(radius - center));
}

fragment float4 pet_fragment(PetVertexOutput input [[stage_in]],
                             texture2d<float> capture [[texture(0)]],
                             sampler capture_sampler [[sampler(0)]],
                             constant PetUniforms &uniforms [[buffer(0)]]) {
  const float aspect =
      uniforms.viewport_px.x / max(uniforms.viewport_px.y, 1.0f);
  const float2 p =
      (input.uv - 0.5f) * float2(aspect, 1.0f);
  const float panel_radius = length(p);
  const float feather_start =
      uniforms.visual_style == kPetVisualStyleFusion
          ? 0.42f
          : 0.46f;
  const float outer_mask =
      1.0f - smoothstep(feather_start, 0.495f, panel_radius);
  if (outer_mask <= 0.0f) {
    return float4(0.0f);
  }

  const float hole_radius =
      max(uniforms.hole_radius_uv, 1e-4f);
  const float world_scale = kCriticalImpact / hole_radius;
  const float2 projected =
      rotate2d(float2(p.x, -p.y), uniforms.roll) * world_scale;
  const float impact = length(projected);
  const float inner = max(uniforms.disk_inner, 1.6f);
  const float outer = max(uniforms.disk_outer, inner + 0.5f);
  const float maximum_impact = outer + 3.0f;

  float alpha = 0.0f;
  float3 color = float3(0.0f);
  if (impact >= maximum_impact) {
    color =
        weak_deflection_background(p, impact, capture,
                                   capture_sampler, uniforms);
    alpha = uniforms.mode == 0u ? 1.0f : 0.0f;
  } else {
    color =
        trace_schwarzschild(p, capture, capture_sampler,
                            uniforms, alpha);
  }

  if (uniforms.visual_style == kPetVisualStyleFusion) {
    const float rim =
        smoothstep(kCriticalImpact,
                   kCriticalImpact + 0.05f, impact) *
        (1.0f -
         smoothstep(kCriticalImpact + 0.10f,
                    kCriticalImpact + 0.42f, impact));
    const float3 rim_color =
        float3(1.0f, 0.91f, 0.70f) * rim * 0.12f;
    color += rim_color;
    alpha = max(alpha, rim * 0.16f);
  }

  if (uniforms.absorption_progress > 0.0f) {
    const float transition =
        sin(uniforms.absorption_progress * 3.1415927f);
    const float flash =
        uniforms.reduce_motion == 0u
            ? pow(max(0.0f,
                      1.0f -
                          abs(uniforms.absorption_progress - 0.5f) *
                              8.0f),
                  2.0f)
            : 0.0f;
    const float3 addition =
        float3(1.0f, 0.58f, 0.08f) * transition * 0.55f +
        flash * 0.75f;
    color += addition;
    alpha = max(alpha, max(transition * 0.6f,
                           max(addition.r,
                               max(addition.g, addition.b))));
  }

  const float normalized_radius = panel_radius * 2.0f;
  if (uniforms.success_progress > 0.0f) {
    const float pulse_radius =
        uniforms.reduce_motion != 0u
            ? 0.72f
            : 0.54f + uniforms.success_progress * 0.34f;
    const float pulse_opacity =
        uniforms.reduce_motion != 0u
            ? 1.0f -
                  abs(uniforms.success_progress * 2.0f - 1.0f)
            : 1.0f - uniforms.success_progress;
    const float pulse =
        ring_mask(normalized_radius, pulse_radius, 0.022f) *
        pulse_opacity;
    const float3 addition =
        float3(0.16f, 1.0f, 0.43f) * pulse * 1.4f;
    color += addition;
    alpha = max(alpha,
                max(pulse, max(addition.r,
                               max(addition.g, addition.b))));
  }
  if (uniforms.error_progress > 0.0f) {
    const float ripple_radius =
        uniforms.reduce_motion != 0u
            ? 0.76f
            : 0.48f + uniforms.error_progress * 0.48f;
    const float ripple_opacity =
        uniforms.reduce_motion != 0u
            ? 1.0f -
                  abs(uniforms.error_progress * 2.0f - 1.0f)
            : 1.0f - uniforms.error_progress;
    const float ripple =
        ring_mask(normalized_radius, ripple_radius, 0.025f) *
        ripple_opacity;
    const float3 addition =
        float3(1.0f, 0.07f, 0.10f) * ripple * 1.5f;
    color += addition;
    alpha = max(alpha,
                max(ripple, max(addition.r,
                                max(addition.g, addition.b))));
  }

  color = clamp(color, float3(0.0f), float3(1.0f));
  alpha = clamp(max(alpha, max(color.r, max(color.g, color.b))),
                0.0f, 1.0f);
  return float4(color, alpha) * outer_mask;
}

vertex PetPendingVertexOutput pet_pending_vertex(
    uint vertex_id [[vertex_id]], uint instance_id [[instance_id]],
    const device PetPendingInstance *instances [[buffer(1)]],
    constant PetUniforms &uniforms [[buffer(2)]]) {
  const float2 corners[6] = {
      float2(-1.0f, -1.0f), float2(1.0f, -1.0f),
      float2(-1.0f, 1.0f),  float2(-1.0f, 1.0f),
      float2(1.0f, -1.0f),  float2(1.0f, 1.0f),
  };
  const PetPendingInstance instance = instances[instance_id];
  const float orbit =
      uniforms.reduce_motion != 0u
          ? 0.0f
          : uniforms.time_seconds * 0.55f;
  const float cosine = cos(orbit);
  const float sine = sin(orbit);
  const float2 center =
      float2(instance.center.x * cosine -
                 instance.center.y * sine,
             instance.center.x * sine +
                 instance.center.y * cosine);
  const float2 local = corners[vertex_id];
  const float2 point =
      center + local * instance.diameter * 0.5f;
  PetPendingVertexOutput output;
  output.position =
      float4(point.x, -point.y, 0.0f, 1.0f);
  output.local = local;
  return output;
}

fragment float4 pet_pending_fragment(
    PetPendingVertexOutput input [[stage_in]]) {
  const float alpha =
      1.0f - smoothstep(0.68f, 1.0f, length(input.local));
  return float4(float3(1.0f, 0.56f, 0.06f) * alpha * 1.2f,
                alpha);
}
