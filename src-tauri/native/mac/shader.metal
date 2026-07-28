/*
 Black-hole optics are a Metal port of rrrjqy66/BlackHoleTrash
 commit 229d93213cd3e57364b4c6655cfb2c75b7ea4d18 (MIT).
 Original copyright: Copyright (c) 2026 GreenScreen410.
 Fusion material parameters are adapted from cabbagehao/blackhole-timer
 commit f3cc9cc349540ad6d274cd8074cf050b9b0c0200 (MIT).
 The file faller and impact response adapt ZGhey/blackhole-mac
 commit f719aa1139ecc49a728cbb8fac2e60fcfa51996e (MIT).
 Original copyright: Copyright (c) 2026 Jack Zhang.
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
  float2 center_uv;
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
  float impact_level;
  float feed_strength;
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

static float approved_value_noise(float2 point) {
  const float2 cell = floor(point);
  float2 fraction = fract(point);
  fraction = fraction * fraction * (3.0f - 2.0f * fraction);
  return mix(
      mix(hash21(cell), hash21(cell + float2(1.0f, 0.0f)), fraction.x),
      mix(hash21(cell + float2(0.0f, 1.0f)),
          hash21(cell + 1.0f), fraction.x),
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
  const float spot_radius =
      clamp(2.4f, inner + 0.05f, outer - 0.05f);
  const float spot_kepler = pow(inner / spot_radius, 1.5f);
  const float spot_gravity =
      sqrt(max(1.0f - 1.5f / spot_radius, 0.02f));
  const float spot_angle =
      -visual_time * abs(uniforms.speed) * 0.35f *
      spot_kepler * spot_gravity * direction;
  const float spot_delta =
      atan2(sin(phi - spot_angle), cos(phi - spot_angle));
  const float spot_radial =
      (radius - spot_radius) / max(spot_radius * 0.30f, 0.2f);
  const float spot =
      exp(-spot_delta * spot_delta / 0.34f -
          spot_radial * spot_radial * 0.5f) *
      1.8f;
  float3 thermal =
      blackbody(uniforms.temperature * temperature_profile * shift *
                (1.0f + 0.45f * spot));
  if (fusion) {
    thermal =
        mix(thermal, float3(1.0f, 0.91f, 0.70f), 0.12f);
  }
  const float boost = pow(shift, uniforms.beaming);
  float density = band * streaks;
  if (fusion) {
    density = band * (0.62f + 0.58f * streaks);
  }
  density *= 0.55f + 1.5f * spot;
  const float fusion_emissivity =
      fusion
          ? mix(0.10f, 1.0f,
                abs(position.x) / max(radius, 1e-4f))
          : 1.0f;
  const float3 emission =
      transmittance * thermal *
      (uniforms.gain * 2.2f * density * temperature_profile *
       temperature_profile * boost * fusion_emissivity);
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
  const float2 center = uniforms.center_uv;
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
  const float2 center = uniforms.center_uv;
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

static float rounded_box_sdf(float2 point, float2 half_extent,
                             float radius) {
  const float2 q = abs(point) - half_extent + radius;
  return length(max(q, float2(0.0f))) +
         min(max(q.x, q.y), 0.0f) - radius;
}

// Programmatic external-file card. It deliberately carries no filename,
// source path, thumbnail, workspace icon, or file texture.
static float4 procedural_file_card(float2 p,
                                   constant PetUniforms &uniforms) {
  if (uniforms.file_kind == 0u || uniforms.drop_phase == 0u) {
    return float4(0.0f);
  }

  const float aspect =
      uniforms.viewport_px.x / max(uniforms.viewport_px.y, 1.0f);
  const float2 start =
      (uniforms.drop_origin_uv - 0.5f) * float2(aspect, 1.0f);
  const float u = clamp(uniforms.drop_progress, 0.0f, 1.0f);

  // These are the reference Faller stage boundaries.
  float approach = smoothstep(0.00f, 0.25f, u);
  float stretch = smoothstep(0.20f, 0.55f, u);
  // `fragment` is an MSL stage keyword, so the reference stage value uses
  // the equivalent non-reserved local name below.
  float fragment_stage = smoothstep(0.45f, 0.72f, u);
  float merge = smoothstep(0.70f, 0.88f, u);
  float fade = 1.0f - smoothstep(0.88f, 1.00f, u);
  uint fragment_count = u >= 0.45f && u < 0.88f ? 12u : 0u;

  float2 center = start;
  float half_size = 0.065f;
  float alpha = 1.0f;
  float tear = 0.0f;
  const float horizon = max(uniforms.hole_radius_uv, 0.01f);

  if (uniforms.drop_phase == 3u) {
    if (uniforms.reduce_motion != 0u) {
      alpha = 1.0f - u;
      stretch = 0.0f;
      fragment_stage = 0.0f;
      merge = 0.0f;
      fragment_count = 0u;
    } else {
      // ZGhey/blackhole-mac Faller: a rapid radial approach followed by a
      // slower 1.5-turn orbit in the tidal region.
      const float start_radius = max(length(start), 0.34f);
      const float start_angle = atan2(start.y, start.x);
      const float settle = horizon * 1.15f;
      const float radius =
          settle + (start_radius - settle) * pow(1.0f - u, 3.4f);
      const float phi =
          start_angle +
          1.5f * 6.2831853f *
              (1.0f - pow(1.0f - u, 2.4f));
      const float2 reference_center =
          float2(cos(phi), sin(phi)) * radius;
      // Core-target drops can begin inside the reference faller's 0.34
      // staging radius. Blend into that path over the approach stage so the
      // acknowledged origin remains exact at u == 0.
      center = mix(start, reference_center, approach);
      half_size =
          0.065f *
          (0.45f + 0.55f * (radius / max(start_radius, 1e-4f)));
      const float tidal =
          smoothstep(horizon * 1.2f, horizon * 2.2f, radius);
      tear = 1.1f * (1.0f - tidal);
      alpha = fade;
    }
  } else if (uniforms.drop_phase == 4u) {
    const float recoil = clamp(uniforms.error_progress, 0.0f, 1.0f);
    const float2 outward =
        length(start) > 1e-4f ? normalize(start) : float2(1.0f, 0.0f);
    center += outward * sin(recoil * 3.1415927f) * 0.075f;
    half_size *= 1.0f + 0.10f * sin(recoil * 3.1415927f);
    alpha = 1.0f - smoothstep(0.72f, 1.0f, recoil);
    stretch = 0.0f;
    fragment_stage = 0.0f;
    merge = 0.0f;
    fragment_count = 0u;
  } else {
    // Pending/hover stays readable and steady at its submitted origin.
    stretch = 0.0f;
    fragment_stage = 0.0f;
    merge = 0.0f;
    fragment_count = 0u;
  }

  if (alpha <= 0.001f) {
    return float4(0.0f);
  }

  float2 local;
  if (uniforms.drop_phase == 3u && uniforms.reduce_motion == 0u) {
    // Invert the Keplerian shear as a pure function of radius, as in the
    // pinned blackhole-mac faller. Inner debris leads and falls inward first.
    const float r0 = max(length(center), 1e-4f);
    const float rr = max(length(p), 1e-5f);
    const float a0 = atan2(center.y, center.x);
    const float theta = atan2(p.y, p.x);
    const float lead =
        tear * (pow(r0 / rr, 1.5f) - 1.0f);
    float delta = theta - a0 - lead;
    delta = atan2(sin(delta), cos(delta));
    float radial = (rr - r0) / max(half_size, 1e-4f);
    if (radial < 0.0f) {
      radial /= 1.0f + 2.2f * tear;
    }
    const float tangential =
        delta * r0 / max(half_size, 1e-4f);
    const float2 sheared =
        float2(tangential / (1.0f + 0.70f * stretch),
               radial / max(1.0f - 0.30f * stretch, 0.45f));
    const float2 cartesian =
        (p - center) / max(half_size, 1e-4f);
    local = mix(cartesian, sheared, approach);
    local = rotate2d(local, 0.08f * approach * sin(u * 6.2831853f));
  } else {
    local = (p - center) / max(half_size, 1e-4f);
  }

  // Twelve deterministic 4×3 pieces open along seams during disruption.
  float fragment_alpha = 1.0f;
  if (fragment_count == 12u) {
    const float2 grid =
        clamp(local / float2(2.0f, 1.44f) * 0.5f + 0.5f,
              float2(0.0f), float2(0.9999f));
    const float2 cell = floor(grid * float2(4.0f, 3.0f));
    const float cell_id = cell.x + cell.y * 4.0f;
    const float2 jitter =
        float2(hash21(float2(cell_id, 4.1f)),
               hash21(float2(cell_id, 9.7f))) -
        0.5f;
    local -=
        jitter * fragment_stage * (1.0f - 0.35f * merge) * 0.24f;
    const float2 within = fract(grid * float2(4.0f, 3.0f));
    const float seam =
        min(min(within.x, 1.0f - within.x),
            min(within.y, 1.0f - within.y));
    fragment_alpha =
        smoothstep(0.025f, 0.10f, seam);
  }

  const float body_distance =
      rounded_box_sdf(local, float2(1.0f, 0.72f), 0.16f);
  const float body =
      smoothstep(0.06f, -0.04f, body_distance) * fragment_alpha;
  if (body <= 0.001f) {
    return float4(0.0f);
  }

  const bool is_gcode = uniforms.file_kind == 2u;
  float3 card_color =
      is_gcode ? float3(0.16f, 0.82f, 0.52f)
               : float3(0.20f, 0.66f, 1.0f);
  if (uniforms.drop_phase == 4u) {
    card_color = mix(card_color, float3(1.0f, 0.08f, 0.10f),
                     clamp(uniforms.error_progress * 1.4f, 0.0f, 1.0f));
  }
  const float edge =
      smoothstep(0.10f, 0.0f, abs(body_distance));
  const float fold =
      smoothstep(0.22f, 0.02f,
                 max(local.x - 0.57f, -local.y - 0.28f));
  const float mark =
      is_gcode
          ? smoothstep(0.11f, 0.02f,
                       abs(sin((local.y + 0.42f) * 17.0f)) * 0.12f +
                           max(abs(local.x + 0.05f) - 0.48f, 0.0f))
          : smoothstep(0.18f, 0.05f,
                       abs(length((local + float2(0.05f, 0.02f)) *
                                  float2(1.0f, 1.25f)) -
                           0.32f));
  float3 color =
      card_color * (0.68f + 0.24f * edge) +
      float3(1.0f) * (0.22f * fold + 0.32f * mark);
  const float final_alpha = clamp(body * alpha, 0.0f, 1.0f);
  return float4(color * final_alpha, final_alpha);
}

// MSL port of BlackHoleTrash's pinned absorption_jet_overlay, using fixed
// energy 1.0 and omitting cursor graphics.
static float3 absorption_jet_overlay(float3 base, float2 p,
                                     constant PetUniforms &uniforms) {
  const float progress =
      clamp(uniforms.absorption_progress, 0.0f, 1.0f);
  const float energy = 1.0f;
  const float energy01 = 0.0f;
  const float radius = max(uniforms.hole_radius_uv, 1e-4f);

  float2 axis =
      normalize(rotate2d(float2(0.0f, -1.0f), -uniforms.roll));
  if (axis.y > 0.0f) {
    axis = -axis;
  }
  const float2 tangent = float2(-axis.y, axis.x);
  const float axial = dot(p, axis);
  const float transverse = dot(p, tangent);

  float attack = smoothstep(0.0f, 0.13f, progress);
  float decay = 1.0f - smoothstep(0.45f, 1.0f, progress);
  const float envelope = attack * decay;
  float extension = smoothstep(0.0f, 0.24f, progress);
  const float main_length =
      min(radius * mix(11.0f, 14.0f, energy01), 0.52f) *
      extension;
  const float base_width =
      radius * mix(0.38f, 0.50f, energy01);

  const float main_distance = max(axial, 0.0f);
  const float main_fraction =
      clamp(main_distance / max(main_length, radius), 0.0f, 1.0f);
  const float main_cap =
      step(0.0f, axial) *
      (1.0f -
       smoothstep(main_length * 0.82f, max(main_length, radius),
                  main_distance));
  const float main_width =
      base_width * mix(1.55f, 0.42f, main_fraction);
  const float filament =
      0.84f +
      0.16f *
          sin(main_distance / radius * 10.0f -
              uniforms.time_seconds * 34.0f +
              transverse / radius * 2.4f);
  const float main_core =
      exp2(-pow(transverse / max(main_width * 0.20f, 1e-5f), 2.0f) *
           2.2f) *
      main_cap * filament;
  const float main_halo =
      exp2(-pow(transverse / max(main_width, 1e-5f), 2.0f) *
           1.5f) *
      main_cap;

  const float counter_distance = max(-axial, 0.0f);
  const float counter_length = main_length * 0.64f;
  const float counter_fraction =
      clamp(counter_distance / max(counter_length, radius),
            0.0f, 1.0f);
  const float counter_cap =
      step(0.0f, -axial) *
      (1.0f -
       smoothstep(counter_length * 0.78f,
                  max(counter_length, radius), counter_distance));
  const float counter_width =
      base_width * mix(1.35f, 0.50f, counter_fraction);
  const float counter =
      exp2(-pow(transverse / max(counter_width, 1e-5f), 2.0f) *
           1.7f) *
      counter_cap * 0.18f;

  const float radial = length(p);
  float shock_progress = smoothstep(0.02f, 0.72f, progress);
  const float shock_radius =
      radius * mix(1.15f, mix(4.8f, 5.7f, energy01),
                   shock_progress);
  const float shock_width =
      radius * mix(0.30f, 0.09f, shock_progress);
  const float shock =
      exp2(-pow((radial - shock_radius) /
                    max(shock_width, 1e-5f),
                2.0f) *
           2.8f) *
      (1.0f - smoothstep(0.18f, 0.78f, progress));
  float flash_decay = 1.0f - smoothstep(0.0f, 0.28f, progress);
  const float flash =
      exp2(-pow(radial / max(radius * 0.95f, 1e-5f), 2.0f) *
           2.4f) *
      flash_decay;

  float3 light = float3(0.0f);
  light += float3(0.48f, 0.60f, 1.0f) * main_halo *
               envelope * energy;
  light += float3(0.96f, 0.98f, 1.0f) * main_core *
               envelope * energy;
  light += float3(0.58f, 0.48f, 1.0f) * counter *
               envelope * energy;
  light += float3(0.68f, 0.80f, 1.0f) * shock * energy;
  light += float3(0.92f, 0.95f, 1.0f) * flash * energy;
  const float3 contribution =
      float3(1.0f) - exp(-min(light, float3(4.0f)));
  return min(base + contribution, float3(1.25f));
}

static float3 impact_afterglow_overlay(
    float3 base, float2 p, constant PetUniforms &uniforms) {
  const float radius = max(uniforms.hole_radius_uv, 1e-4f);
  const float radial = length(p);
  const float impact = max(uniforms.impact_level, 0.0f);
  const float feed = max(uniforms.feed_strength, 0.0f);
  if (impact <= 0.0001f && feed <= 0.0001f) {
    return base;
  }

  const float impact_ring =
      exp2(-pow((radial - radius * 2.15f) /
                    max(radius * 0.22f, 1e-5f),
                2.0f) *
           2.8f);
  const float impact_flash =
      exp2(-pow(radial / max(radius * 1.25f, 1e-5f), 2.0f) *
           2.2f);

  const float2 source =
      uniforms.drop_origin_uv - float2(0.5f);
  const float source_angle =
      length(source) > 1e-4f ? atan2(source.y, source.x) : 0.0f;
  float delta = atan2(p.y, p.x) - source_angle;
  delta = atan2(sin(delta), cos(delta));
  const float feed_width =
      mix(2.9f, 0.40f, clamp(feed, 0.0f, 1.0f));
  const float feed_arc =
      exp(-delta * delta / max(feed_width * feed_width, 1e-4f)) *
      exp2(-pow((radial - radius * 3.0f) /
                    max(radius * 0.55f, 1e-5f),
                2.0f) *
           1.8f) *
      feed;
  const float3 feed_color =
      uniforms.file_kind == 2u
          ? float3(0.52f, 1.0f, 0.62f)
          : float3(0.42f, 0.72f, 1.0f);
  const float3 addition =
      float3(1.0f, 0.64f, 0.18f) *
          impact * (impact_ring * 1.2f + impact_flash * 0.75f) +
      feed_color * feed_arc * 0.85f;
  return min(base + addition, float3(1.25f));
}

static float4 approved_shade_crossing(
    float3 position, float3 velocity, float3 normal, float3 disk_axis,
    constant PetUniforms &uniforms, float transmittance,
    float visual_time) {
  const float radius = length(position);
  if (radius <= uniforms.disk_inner || radius >= uniforms.disk_outer) {
    return float4(0.0f);
  }
  const float phi =
      atan2(dot(position, disk_axis), position.x);
  const float grain = approved_value_noise(
      float2(radius * 2.8f + phi * uniforms.wind * 0.12f,
             phi * 3.0f -
                 visual_time * uniforms.speed * 0.55f));
  const float contrast_mix =
      clamp(uniforms.contrast * 0.5f, 0.0f, 1.0f);
  const float streak =
      mix(1.0f,
          0.25f +
              1.9f * pow(grain, 1.0f + uniforms.contrast),
          contrast_mix);
  const float band =
      smoothstep(uniforms.disk_inner,
                 uniforms.disk_inner + 0.45f, radius) *
      (1.0f -
       smoothstep(max(uniforms.disk_inner + 0.5f,
                      uniforms.disk_outer - 2.4f),
                  uniforms.disk_outer, radius));
  const float beta =
      clamp(rsqrt(max(2.0f * (radius - 1.0f), 0.2f)),
            0.0f, 0.99f);
  const float gravity =
      sqrt(max(1.0f - 1.5f / radius, 0.02f)) /
      max(1.0f +
              beta *
                  dot(normalize(cross(normal, position)),
                      normalize(velocity)),
          0.05f);
  const float shift = mix(1.0f, gravity, uniforms.doppler);
  const float temperature =
      pow(uniforms.disk_inner / radius, 0.75f) *
      pow(max(1.0f -
                  sqrt(uniforms.disk_inner / radius),
              0.0f),
          0.25f) /
      0.488f;
  const float density = band * streak;
  const float3 emission =
      transmittance *
      blackbody(uniforms.temperature * temperature * shift) *
      (4.8f * uniforms.gain * density * temperature *
       temperature * pow(shift, uniforms.beaming));
  return float4(emission, density);
}

static float4 approved_black_hole(
    float2 uv, texture2d<float> capture,
    sampler capture_sampler,
    constant PetUniforms &uniforms) {
  const float2 resolution =
      max(uniforms.viewport_px, float2(1.0f));
  const float aspect = resolution.x / resolution.y;
  const float visual_time =
      uniforms.reduce_motion == 0u ? uniforms.time_seconds : 0.0f;
  const float hole_radius =
      max(uniforms.hole_radius_uv, 1e-4f);
  const float2 center = uniforms.center_uv;
  const float2 p =
      (uv - center) * float2(aspect, 1.0f);
  const float screen_radius = length(p);
  const float window =
      exp(-pow(screen_radius / (7.0f * hole_radius), 2.0f));
  const float mask =
      1.0f -
      smoothstep(3.5f * hole_radius,
                 4.2f * hole_radius, screen_radius);
  if (mask < 0.002f) {
    return float4(0.0f);
  }

  const float world_scale =
      kCriticalImpact / hole_radius;
  const float2 projected =
      rotate2d(float2(p.x, -p.y), uniforms.roll) *
      world_scale;
  const float impact = length(projected);
  const float maximum_impact =
      uniforms.disk_outer + 3.0f;
  const float camera_depth = 14.0f;

  if (impact > maximum_impact) {
    const float deflection =
        (2.0f / (world_scale * world_scale)) /
        max(screen_radius, 0.0001f) *
        13.0f * window;
    const float2 sampled =
        mirror_uv(center +
                  (p - normalize(p) * deflection) /
                      float2(aspect, 1.0f));
    const float3 background =
        captured_background(sampled, capture,
                            capture_sampler, uniforms);
    return float4(background, mask);
  }

  float3 position = float3(projected, camera_depth);
  float3 velocity = float3(0.0f, 0.0f, -1.0f);
  float3 previous_position = position;
  const float angular_momentum_squared =
      dot(projected, projected);
  const float3 normal =
      float3(0.0f, sin(uniforms.inclination),
             cos(uniforms.inclination));
  const float previous_axis_cosine =
      cos(uniforms.inclination);
  const float3 disk_axis =
      float3(0.0f, previous_axis_cosine,
             -sin(uniforms.inclination));
  float previous_side = dot(position, normal);
  float3 emission = float3(0.0f);
  float transmittance = 1.0f;
  bool captured = false;

  for (uint step = 0u; step < 40u; ++step) {
    float radius_squared = dot(position, position);
    if (radius_squared < 1.0f) {
      captured = true;
      break;
    }
    if (position.z < -camera_depth && velocity.z < 0.0f) {
      break;
    }
    float radius = sqrt(radius_squared);
    const float delta =
        clamp(0.16f * radius, 0.03f, 1.5f);
    float3 acceleration =
        -1.5f * angular_momentum_squared * position /
        (radius_squared * radius_squared * radius);
    velocity += acceleration * (0.5f * delta);
    position += velocity * delta;
    radius_squared = dot(position, position);
    radius = sqrt(radius_squared);
    acceleration =
        -1.5f * angular_momentum_squared * position /
        (radius_squared * radius_squared * radius);
    velocity += acceleration * (0.5f * delta);

    const float side = dot(position, normal);
    if (side * previous_side < 0.0f &&
        transmittance > 0.02f) {
      const float crossing_fraction =
          previous_side / (previous_side - side);
      const float3 crossing_position =
          mix(previous_position, position,
              crossing_fraction);
      const float4 crossing = approved_shade_crossing(
          crossing_position, normalize(velocity), normal,
          disk_axis, uniforms, transmittance, visual_time);
      emission += crossing.rgb;
      transmittance *=
          1.0f -
          clamp(uniforms.disk_opacity * crossing.a,
                0.0f, 0.95f);
    }
    previous_side = side;
    previous_position = position;
  }

  float3 background =
      captured_background(uv, capture, capture_sampler,
                          uniforms);
  const bool shadow =
      captured &&
      screen_radius < hole_radius * 1.06f;
  if (!shadow && !captured) {
    const float3 direction = normalize(velocity);
    if (direction.z < -0.05f) {
      const float plane_distance =
          (-13.0f - position.z) / direction.z;
      const float2 sky =
          rotate2d((position +
                    direction * plane_distance)
                       .xy,
                   -uniforms.roll) /
          world_scale;
      const float2 sampled =
          mirror_uv(center +
                    (p +
                     (float2(sky.x, -sky.y) - p) *
                         window) /
                        float2(aspect, 1.0f));
      background =
          captured_background(sampled, capture,
                              capture_sampler, uniforms);
    }
  }
  if (uniforms.stars > 0.0f) {
    const float star =
        pow(hash21(floor(uv * resolution / 5.0f)),
            32.0f) *
        uniforms.stars;
    background += float3(0.55f, 0.72f, 1.0f) * star;
  }
  const float disk_absorption =
      clamp((1.0f - transmittance) * 0.22f,
            0.0f, 0.22f);
  const float3 lit =
      background * (1.0f - disk_absorption) +
      (1.0f -
       exp(-emission * 1.4f * uniforms.exposure));
  const float shadow_edge =
      shadow
          ? 1.0f -
                smoothstep(hole_radius * 0.90f,
                           hole_radius * 1.06f,
                           screen_radius)
          : 0.0f;
  return float4(mix(lit, float3(0.0f), shadow_edge),
                mask);
}

fragment float4 pet_fragment(PetVertexOutput input [[stage_in]],
                             texture2d<float> capture [[texture(0)]],
                             sampler capture_sampler [[sampler(0)]],
                             constant PetUniforms &uniforms [[buffer(0)]]) {
  const float aspect =
      uniforms.viewport_px.x / max(uniforms.viewport_px.y, 1.0f);
  const float2 p =
      (input.uv - uniforms.center_uv) * float2(aspect, 1.0f);
  const float panel_radius = length(p);
  const float4 approved =
      approved_black_hole(input.uv, capture, capture_sampler, uniforms);
  if (approved.a <= 0.0f &&
      uniforms.drop_phase == 0u &&
      uniforms.success_progress <= 0.0f &&
      uniforms.error_progress <= 0.0f &&
      uniforms.impact_level <= 0.0f &&
      uniforms.feed_strength <= 0.0f) {
    return float4(0.0f);
  }
  float3 color = approved.rgb;
  float alpha = approved.a;
  float3 premultiplied_color = color * alpha;

  if (uniforms.drop_phase == 3u &&
      uniforms.reduce_motion == 0u &&
      uniforms.absorption_progress > 0.0f) {
    const float3 before = color;
    color = absorption_jet_overlay(color, p, uniforms);
    const float3 addition = max(color - before, float3(0.0f));
    premultiplied_color += addition;
    alpha = max(alpha, max(addition.r,
                           max(addition.g, addition.b)));
  }

  {
    const float3 before = color;
    color = impact_afterglow_overlay(color, p, uniforms);
    const float3 addition = max(color - before, float3(0.0f));
    premultiplied_color += addition;
    alpha = max(alpha, max(addition.r,
                           max(addition.g, addition.b)));
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
    premultiplied_color += addition;
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
    premultiplied_color += addition;
    alpha = max(alpha,
                max(ripple, max(addition.r,
                                max(addition.g, addition.b))));
  }

  const float4 card = procedural_file_card(p, uniforms);
  premultiplied_color =
      card.rgb + premultiplied_color * (1.0f - card.a);
  alpha = card.a + alpha * (1.0f - card.a);

  premultiplied_color =
      clamp(premultiplied_color, float3(0.0f), float3(1.0f));
  alpha = clamp(alpha, 0.0f, 1.0f);
  return float4(premultiplied_color, alpha);
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
