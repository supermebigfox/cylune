# Windows black-hole parity QA

This gate compares the Windows D3D11/HLSL renderer with the approved macOS
recording from native reference commit `d640d92`. It is a perceptual gate, not
permission to tune either renderer. A mismatch fails the run; do not change the
sealed Mac implementation to make a Windows recording pass.

## Fixed capture setup

- Record both platforms at 3840×2160 and 60 FPS using the same configured
  black-hole size, visual style, brightness, and desktop scale.
- Use the same source asset or reproducible desktop motion, place the black-hole
  center at the same normalized coordinates, and begin each clip from idle.
- Capture at least 5 seconds before interaction and through the complete active
  animation. Save the OS/build, GPU, monitor topology, scale, style, size, FPS,
  source asset, and capture timestamps beside each pair.
- Compare synchronized clips at full resolution and at 0.25× speed. Keep the
  approved macOS clip and the Windows candidate together so the comparison can
  be repeated.

### Pinned evidence protocol

- Use `docs/qa-assets/windows-parity-checkerboard.html` at browser zoom 100%,
  full-screen 3840×2160. It is the moving-checkerboard source of record: 64 px
  cells inside a 128 px repeat, translating right at 96 px/s and down at
  48 px/s. Record its Git blob hash with each clip.
- Use `brand/poster-final-4k.png` as the static color/detail source and record
  its Git blob hash. The white-browser scene is a blank local HTML page with
  black 32 px system text in 96 px rows; scroll downward at 480 px/s using an
  automated input tool whose name/version/script is saved with the evidence.
- The Explorer scene uses a clean folder containing exactly four copies of the
  same fixture named `01.gcode.3mf`, `02.3mf`, `03.gcode`, and `04.txt`, laid
  out as large icons in one row. The approved Mac Finder scene uses the same
  bytes, names, order, icon size, and window bounds. Save fixture SHA-256 values;
  do not commit user print files.
- Interaction path: begin with the black-hole center at normalized
  `(0.25, 0.50)`; capture 5.0 s idle, hover 2.0 s, then drag linearly to
  `(0.75, 0.50)` over 4.0 s. For ingest/eject/jet clips, wait 5.0 s idle and
  release the fixture at the normalized center. Record input automation and
  timestamps beside the clip.
- Capture losslessly with FFmpeg `ffv1` level 3 in Matroska at 3840×2160/60;
  save the exact `ffmpeg -version`, capture-device enumeration and command.
  Do not compare H.264/HEVC proxy files. A proxy may accompany, but never
  replace, the `.mkv` source. Record the monitor color profile and whether HDR
  is disabled; both platforms must use the same SDR condition.

## Capture matrix

| Scene | Required action | Evidence to inspect |
| --- | --- | --- |
| Dark navy | Hold idle, then hover for 2 seconds | Transparent outer field, moving center light, clockwise disk/flow, unchanged diameter |
| White browser | Scroll continuously behind idle and hover states | Live interior sampling, no fixed circular lens edge, no delayed or frozen interior |
| Moving checkerboard | Translate and animate the 4K checkerboard behind the pet | Per-frame distortion continuity and absence of rectangular capture boundaries |
| Explorer icons | Move and select icons behind and around the pet | Correct live icon distortion, no detached or free-floating sampled artifact |
| Multi-monitor drag | Drag across each monitor, including mixed scale/rotation and cross-adapter boundaries | Correct normalized center, uninterrupted current-monitor sampling, no stale frame |
| Ingest | Drop a supported file and record through the 0.74 s swallow interval | Raw ingest uniform timing, clockwise acceleration/pull, dynamic center light, unchanged diameter |
| Eject | Drop a rejected/unsupported file and record through 0.74 s swallow plus 0.62 s eject | Eject begins only after swallow progress reaches 1.0; outward flow remains centered and live |
| Success jet | Complete a supported ingest and record the 0.50 s jet | Jet timing, alignment, fade, live background, clockwise flow, and dynamic center light |

Windows currently has no macOS-style file-icon spiral overlay. Task 9 does not
add a Direct2D/icon layer or change the acknowledgement/generation protocol.
Record that absence in the run notes as a known platform difference; judge the
shared black-hole uniforms, interaction timing, and renderer output described
above.

## Rejection checklist

Reject the Windows run if any clip shows:

- a fixed circular lens edge;
- a black rectangular boundary;
- a frozen sampled frame or delayed interior;
- a free-floating capture artifact;
- counter-clockwise motion;
- a diameter change during hover or an active animation;
- a center light that freezes in hover, ingest, eject, or jet;
- timing different from 0.74 s swallow, 0.62 s eject, or 0.50 s success jet;
- stale/current-monitor mismatch during a multi-monitor drag; or
- any changed output from the sealed Mac build.

## Automated gates and sign-off

Before accepting recordings, run:

```text
npm run check:mac-seal
npm test -- --run
node scripts/rust.mjs cargo test --manifest-path src-tauri/Cargo.toml
```

The reviewer records PASS/FAIL for every matrix row, links both matching clips,
and signs with date, Windows build/GPU, Mac build/GPU, size/style/FPS, and the
`d640d92` seal result. Any FAIL blocks parity sign-off.
