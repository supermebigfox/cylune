// Direct numerical port of the sealed Metal black-hole shader.
struct Params {
  float2 resolution;
  float time;
  float size;
  float brightness;
  float speed;
  uint style;
  uint pendingCount;
  float2 center;
  float ingestProgress;
  float ejectProgress;
  float pullGain;
  float successJetProgress;
  uint desktopRotation;
  float padding1;
};

struct VertexOutput {
  float4 position : SV_POSITION;
  float2 uv : TEXCOORD0;
};

struct Preset {
  float diskTemp;
  float diskIncl;
  float diskRoll;
  float diskInner;
  float diskOuter;
  float diskOpacity;
  float dopplerMix;
  float diskBeam;
  float diskGain;
  float diskContrast;
  float diskWind;
  float diskSpeed;
  float starGain;
  float exposure;
};

cbuffer BlackHoleParams : register(b0) { Params P; }
Texture2D<float4> desktop : register(t0);
SamplerState linearSampler : register(s0);

Preset presetForStyle(uint style) {
  Preset p = {8500.0, 1.45, 0.15, 3.0, 9.0, 0.65, 1.0,
              3.0,    1.0,  0.9,  5.0, 3.6, 0.0,  1.0};
  switch (style) {
    case 1: {
      Preset value = {4500.0, 1.52, 0.10, 2.2, 7.0, 0.85, 0.35,
                      2.0,    1.4,  0.5,  5.0, 3.6, 0.0,  1.2};
      p = value;
      break;
    }
    case 2: {
      Preset value = {15000.0, 1.30, 0.15, 3.0, 14.0, 0.35, 1.0,
                      4.0,     1.2,  1.3,  8.0, 3.6,  0.0,  0.8};
      p = value;
      break;
    }
    case 3: {
      Preset value = {6500.0, 0.30, 0.0, 3.0, 10.0, 0.50, 0.8,
                      2.5,    1.0,  1.1, 5.0, 3.6,  0.0, 1.0};
      p = value;
      break;
    }
    case 4: {
      Preset value = {3800.0, 0.55, -0.30, 2.2, 6.0, 0.45, 0.9,
                      3.5,    1.6,  0.4,   3.0, 2.5, 0.0,  1.1};
      p = value;
      break;
    }
    case 5: {
      Preset value = {18000.0, 1.05, 0.55, 3.0, 16.0, 0.30, 1.0,
                      5.0,     1.0,  1.5,  9.0, 6.0,  0.0,  0.75};
      p = value;
      break;
    }
    case 6: {
      Preset value = {5500.0, 1.50, 0.35, 1.8, 8.0, 0.90, 0.6,
                      2.5,    2.2,  1.6,  7.0, 5.0, 0.0,  1.4};
      p = value;
      break;
    }
    case 7: {
      Preset value = {8500.0, 1.45, 0.15, 3.0, 9.0, 0.0, 1.0,
                      3.0,    0.0,  0.9,  5.0, 3.6, 0.6, 1.0};
      p = value;
      break;
    }
    case 8: {
      Preset value = {7000.0, 1.45, 0.15, 3.5, 7.0, 0.40, 0.5,
                      2.0,    0.5,  0.3,  3.0, 1.5, 0.0,  0.7};
      p = value;
      break;
    }
    default:
      break;
  }
  return p;
}

VertexOutput vs_main(uint vertexId : SV_VertexID) {
  const float2 positions[3] = {float2(-1.0, -1.0), float2(-1.0, 3.0),
                               float2(3.0, -1.0)};
  VertexOutput output;
  output.position = float4(positions[vertexId], 0.0, 1.0);
  output.uv = float2(positions[vertexId].x * 0.5 + 0.5,
                     0.5 - positions[vertexId].y * 0.5);
  return output;
}

float2 mirrorUV(float2 u) { return 1.0 - abs(1.0 - fmod(u, 2.0)); }

float2 wallpaperUV(float2 u, float2 resolution) {
  uint textureWidth = 0;
  uint textureHeight = 0;
  desktop.GetDimensions(textureWidth, textureHeight);
  float screenAspect = resolution.x / resolution.y;
  float textureAspect = (P.desktopRotation == 90u || P.desktopRotation == 270u)
      ? (float) textureHeight / (float) max(textureWidth, 1u)
      : (float) textureWidth / (float) max(textureHeight, 1u);
  if (textureAspect > screenAspect) {
    u.x = 0.5 + (u.x - 0.5) * (screenAspect / textureAspect);
  } else {
    u.y = 0.5 + (u.y - 0.5) * (textureAspect / screenAspect);
  }
  return clamp(u, 0.0, 1.0);
}

float2 desktopUV(float2 u) {
  if (P.desktopRotation == 90u) return float2(u.y, 1.0 - u.x);
  if (P.desktopRotation == 180u) return float2(1.0 - u.x, 1.0 - u.y);
  if (P.desktopRotation == 270u) return float2(1.0 - u.y, u.x);
  return u;
}

float2 rot(float2 p, float a) {
  float c = cos(a);
  float s = sin(a);
  return float2(c * p.x - s * p.y, s * p.x + c * p.y);
}

float hash21(float2 p) {
  p = frac(p * float2(234.34, 435.345));
  p += dot(p, p + 34.23);
  return frac(p.x * p.y);
}

float noise(float2 p) {
  float2 i = floor(p);
  float2 f = frac(p);
  f = f * f * (3.0 - 2.0 * f);
  return lerp(lerp(hash21(i), hash21(i + float2(1.0, 0.0)), f.x),
              lerp(hash21(i + float2(0.0, 1.0)), hash21(i + 1.0), f.x),
              f.y);
}

float inflowContour(float angle, float normalizedRadius, float t) {
  float logarithmicRadius = log(max(normalizedRadius, 1.02));
  float primary = pow(0.5 + 0.5 * cos(angle * 3.0 -
                                      logarithmicRadius * 7.2 - t * 3.35),
                      7.0);
  float secondary = pow(0.5 + 0.5 * cos(angle * 5.0 -
                                        logarithmicRadius * 11.0 - t * 4.60),
                        10.0);
  float organic = noise(float2(angle * 1.35 - t * 0.31,
                               logarithmicRadius * 4.20 + t * 1.16));
  return clamp(primary * 0.78 + secondary * 0.34 + organic * 0.13, 0.0,
               1.0);
}

float2 inwardAccretionFlow(float2 p, float plen, float rh, float t,
                           float activityGain, float flowDirection) {
  float safeRadius = max(rh, 0.0001);
  float normalizedRadius = plen / safeRadius;
  float coreGuard = smoothstep(1.03, 1.32, normalizedRadius);
  float2 radial = plen > 0.0001 ? p / plen : float2(1.0, 0.0);
  // Preserve the sealed Metal flow direction in every active state.
  float2 clockwiseTangent = float2(-radial.y, radial.x);
  float angle = atan2(p.y, p.x);
  float radialFade = 1.0 - smoothstep(3.45, 5.0, normalizedRadius);
  float contour = inflowContour(angle, normalizedRadius, t);
  float fullSurfaceEnvelope = coreGuard * radialFade;
  float stream = noise(float2(angle * 1.65 - t * 0.96,
                              normalizedRadius * 2.10 + t * 2.62));
  float filament = noise(float2(angle * 4.60 - t * 1.52,
                                normalizedRadius * 3.70 + t * 3.48));
  float spiralInflow = contour;
  float flowGain = 1.50 * activityGain;
  float radialPull = safeRadius * fullSurfaceEnvelope * flowGain *
                     (1.38 + 1.30 * spiralInflow + 0.82 * stream);
  float rotationalPull = safeRadius * fullSurfaceEnvelope * flowGain *
                         (1.02 + 0.92 * spiralInflow + 0.52 * filament);
  return (radial * radialPull + clockwiseTangent * rotationalPull) *
         flowDirection;
}

float3 blackbody(float temperature) {
  float t = clamp(temperature, 1500.0, 40000.0) / 100.0;
  float r = t <= 66.0
                ? 1.0
                : clamp(1.292936 * pow(t - 60.0, -0.1332047), 0.0, 1.0);
  float g = t <= 66.0
                ? clamp(0.3900816 * log(t) - 0.6318414, 0.0, 1.0)
                : clamp(1.1298909 * pow(t - 60.0, -0.0755148), 0.0, 1.0);
  float b = t >= 66.0
                ? 1.0
                : (t <= 19.0
                       ? 0.0
                       : clamp(0.5432068 * log(t - 10.0) - 1.196254, 0.0,
                               1.0));
  return float3(r, g, b);
}

float3 diskTintForStyle(uint style, float heat) {
  float h = clamp(heat, 0.0, 1.0);
  if (style == 1) {
    return lerp(float3(0.46, 0.012, 0.002), float3(1.00, 0.25, 0.012), h);
  }
  return lerp(float3(0.025, 0.012, 0.34), float3(0.40, 0.025, 0.92), h);
}

float4 successJet(float2 p, float rh, float t, float progress) {
  float burst = sin(3.14159265 * clamp(progress, 0.0, 1.0));
  float2 axis = normalize(float2(-0.12, 1.0));
  float2 tangent = float2(axis.y, -axis.x);
  float axial = dot(p, axis) / max(rh, 0.0001);
  float lateral = abs(dot(p, tangent)) / max(rh, 0.0001);
  float reach = 0.85 + 4.25 * smoothstep(0.0, 0.58, progress);
  float coneWidth = 0.055 + 0.055 * abs(axial);
  float core = exp(-pow(lateral / max(coneWidth, 0.001), 2.0) * 3.4);
  float lobe = smoothstep(0.55, 0.88, abs(axial)) *
               (1.0 - smoothstep(reach - 0.45, reach, abs(axial)));
  float particles =
      0.58 + 0.42 * noise(float2(lateral * 34.0 - t * 5.2,
                                 abs(axial) * 7.0 - t * 9.0));
  float alpha = core * lobe * particles * burst;
  float whiteCore =
      exp(-pow(lateral / max(coneWidth * 0.38, 0.001), 2.0) * 4.0);
  float3 color = lerp(float3(0.04, 0.32, 1.0),
                      float3(0.72, 0.94, 1.0), whiteCore);
  return float4(color * alpha, alpha);
}

float4 ps_main(VertexOutput input) : SV_TARGET {
  Preset S = presetForStyle(P.style);
  float ingest = clamp(P.ingestProgress, 0.0, 1.0);
  float eject = clamp(P.ejectProgress, 0.0, 1.0);
  float ingestPulse = sin(3.14159265 * ingest);
  float ejectPulse = sin(3.14159265 * eject);
  float activityGain =
      max(P.pullGain, 1.0) + 1.70 * ingestPulse + 1.10 * ejectPulse;
  float flowDirection = 1.0 - 2.15 * ejectPulse;
  float2 uv = input.uv;
  float2 res = P.resolution;
  float aspect = res.x / res.y;
  float t = P.time * P.speed + ingest * 5.50 - eject * 4.80;
  float rh = 0.125 * P.size;
  float2 center = clamp(P.center, float2(0.0, 0.0), float2(1.0, 1.0));
  float2 p = (uv - center) * float2(aspect, 1.0);
  float plen = length(p);
  float window = exp(-pow(plen / (7.0 * rh), 2.0));
  float2 spacetimeFlow =
      inwardAccretionFlow(p, plen, rh, t, activityGain, flowDirection);
  float normalizedRadius = plen / max(rh, 0.0001);
  float warpedRadius = length(p + spacetimeFlow * 0.24) / max(rh, 0.0001);
  float mask = 1.0 - smoothstep(3.10, 5.0, warpedRadius);
  float4 jet = successJet(p, rh, t, P.successJetProgress);
  if (mask < 0.002 && jet.a < 0.002) return float4(0.0, 0.0, 0.0, 0.0);

  const float B = 2.5980762;
  const float Z0 = 14.0;
  float W = B / max(rh, 0.0001);
  float2 pr = rot(float2(p.x, -p.y), S.diskRoll) * W;
  float b = length(pr);
  if (b > S.diskOuter + 3.0) {
    float deflection = (2.0 / (W * W)) / max(plen, 0.0001) *
                       (13.0 / window * window) * window;
    float2 sampleUv = mirrorUV(
        center + (p + spacetimeFlow - normalize(p) * deflection) /
                     float2(aspect, 1.0));
    return float4(desktop.Sample(linearSampler,
                                 desktopUV(wallpaperUV(sampleUv, res))).rgb +
                      jet.rgb,
                  max(mask, jet.a));
  }

  float3 x = float3(pr, Z0);
  float3 v = float3(0.0, 0.0, -1.0);
  float3 previous = x;
  float h2 = dot(pr, pr);
  float3 n = float3(0.0, sin(S.diskIncl), cos(S.diskIncl));
  float previousPlane = dot(x, n);
  float3 emission = float3(0.0, 0.0, 0.0);
  float transmittance = 1.0;
  bool captured = false;
  [loop]
  for (uint i = 0; i < 40; ++i) {
    float r2 = dot(x, x);
    if (r2 < 1.0) {
      captured = true;
      break;
    }
    if (x.z < -Z0 && v.z < 0.0) break;
    float r = sqrt(r2);
    float dt = clamp(0.16 * r, 0.03, 1.5);
    float3 acceleration = -1.5 * h2 * x / (r2 * r2 * r);
    v += acceleration * 0.5 * dt;
    x += v * dt;
    r2 = dot(x, x);
    r = sqrt(r2);
    acceleration = -1.5 * h2 * x / (r2 * r2 * r);
    v += acceleration * 0.5 * dt;
    float plane = dot(x, n);
    if (plane * previousPlane < 0.0 && transmittance > 0.02) {
      float f = previousPlane / (previousPlane - plane);
      float3 hit = lerp(previous, x, f);
      float rc = length(hit);
      if (rc > S.diskInner && rc < S.diskOuter) {
        float phi = atan2(
            dot(hit, float3(0.0, cos(S.diskIncl), -sin(S.diskIncl))),
            hit.x);
        float grain = noise(float2(rc * 2.8 + phi * S.diskWind * 0.12,
                                   phi * 3.0 - t * S.diskSpeed * 0.55));
        float contrastMix = clamp(S.diskContrast * 0.5, 0.0, 1.0);
        float streak = lerp(1.0,
                            0.25 + 1.9 *
                                       pow(grain, 1.0 + S.diskContrast),
                            contrastMix);
        float band =
            smoothstep(S.diskInner, S.diskInner + 0.45, rc) *
            (1.0 - smoothstep(max(S.diskInner + 0.5, S.diskOuter - 2.4),
                              S.diskOuter, rc));
        float beta = clamp(rsqrt(max(2.0 * (rc - 1.0), 0.2)), 0.0, 0.99);
        float gPhysics =
            sqrt(max(1.0 - 1.5 / rc, 0.02)) /
            max(1.0 + beta * dot(normalize(cross(n, hit)), normalize(v)),
                0.05);
        float g = lerp(1.0, gPhysics, S.dopplerMix);
        float temperature =
            pow(S.diskInner / rc, 0.75) *
            pow(max(1.0 - sqrt(S.diskInner / rc), 0.0), 0.25) / 0.488;
        float diskLuminousFlow =
            0.24 + 2.82 * pow(0.5 + 0.5 *
                                       cos(phi * 2.0 -
                                           t * S.diskSpeed * 1.82 + rc * 0.92),
                                   10.0);
        float density = band * streak;
        emission +=
            transmittance * blackbody(S.diskTemp * temperature * g) *
            (4.8 * S.diskGain * density * diskLuminousFlow * temperature *
             temperature * pow(g, S.diskBeam));
        transmittance *=
            1.0 - clamp(S.diskOpacity * density, 0.0, 0.95);
      }
    }
    previousPlane = plane;
    previous = x;
  }

  float2 flowingUv =
      center + (p + spacetimeFlow) / float2(aspect, 1.0);
  float3 background =
      desktop.Sample(linearSampler, desktopUV(wallpaperUV(flowingUv, res))).rgb;
  bool shadow = captured && plen < rh * 1.06;
  float2 starUv = uv;
  if (!shadow && !captured) {
    float3 direction = normalize(v);
    if (direction.z < -0.05) {
      float q = (-13.0 - x.z) / direction.z;
      float2 sky = rot((x + direction * q).xy, -S.diskRoll) / W;
      float2 sampleUv = mirrorUV(
          center +
          (p + spacetimeFlow + (float2(sky.x, -sky.y) - p) * window) /
              float2(aspect, 1.0));
      starUv = sampleUv;
      background =
          desktop.Sample(linearSampler, desktopUV(wallpaperUV(sampleUv, res))).rgb;
    }
  }
  if (S.starGain > 0.0) {
    float star = pow(hash21(floor(starUv * res / 5.0)), 32.0) * S.starGain;
    background += float3(0.55, 0.72, 1.0) * star;
  }
  float diskAbsorption = clamp((1.0 - transmittance) * 0.22, 0.0, 0.22);
  float3 physicalDisk =
      (1.0 - exp(-emission * 1.4 * S.exposure)) * P.brightness;
  float diskPeak = max(physicalDisk.r, max(physicalDisk.g, physicalDisk.b));
  float heat = clamp(length(emission) * 0.055, 0.0, 1.0);
  // The center light remains time-varying during hover, ingest, eject, and jet.
  float luminousBreath = 0.82 + 0.18 * sin(t * 2.05);
  float3 diskLight = diskTintForStyle(P.style, heat) * diskPeak *
                     luminousBreath * (1.0 + 0.45 * ingestPulse);
  float diskOcclusion = clamp(diskAbsorption + diskPeak * 0.82, 0.0, 0.92);
  float3 lit = background * (1.0 - diskOcclusion) + diskLight;
  float shadowEdge =
      shadow ? 1.0 - smoothstep(rh * 0.90, rh * 1.06, plen) : 0.0;
  float3 finalColor = lerp(lit, float3(0.0, 0.0, 0.0), shadowEdge) + jet.rgb;
  return float4(finalColor, max(mask, jet.a));
}
