# CYLUNE Black Hole Desktop Integration Design

Date: 2026-07-28

## Goal

Replace CYLUNE's current black-hole visual implementation with the exact visual renderer from `tiyda/blackhole-desktop`, pinned to commit `03e74a5cf2522748993aca679cdc6027c7b19697`.

The accepted upstream appearance is the visual specification. CYLUNE adds only:

1. manual black-hole dragging;
2. live size adjustment from the main app;
3. supported print-file import when a file is dropped into the black-hole core.

No additional particle system, recoloring, visual restyling, automatic wandering, or trash behavior will be added.

## Source and License

The renderer is based on:

- `tiyda/blackhole-desktop`, MIT License, copyright 2026 tiyda;
- its declared upstream `s0xDk/ghostty-blackhole`, MIT License, copyright 2026 s13k.

CYLUNE will retain the required copyright and MIT license notices in `THIRD_PARTY_NOTICES.md` and in the packaged application.

## Visual Fidelity

The following upstream behavior must be preserved:

- Metal ray-traced gravitational lensing;
- accretion disk, photon ring, brightness, presets, and time animation;
- full-display transparent rendering pane;
- live application-background capture in always-on-top mode;
- multi-display and all-Spaces window behavior;
- transparent pixels outside the visual effect.

The upstream shader and preset values are copied without artistic changes. The only shader-interface change is replacing the upstream sine-driven center with a CPU-provided `center` uniform.

## Native Architecture

### Rendering panes

CYLUNE creates one mouse-transparent, borderless visual pane for every active display. Each pane covers the display's full `NSScreen.frame` and renders the accepted upstream Metal effect.

All panes share one global black-hole center in macOS screen coordinates. Each pane converts that point into its own normalized `center` uniform. This allows the effect to reach physical display edges and remain continuous at display seams without moving or clamping the visual panes.

The visual panes never accept mouse or file-drop events.

### Screen background

Each visual pane captures its corresponding full display and excludes CYLUNE's own windows. Moving the black hole changes only the GPU center uniform; it does not crop, restart, or reposition screen capture.

This removes the current delayed-crop behavior and prevents a stale rectangular capture from following the pointer.

### Interaction target

A separate transparent circular hit-target panel follows the global black-hole center.

It is the only native window that:

- accepts click-drag gestures used to move the black hole;
- registers for Finder file URLs;
- performs the final circular drop hit test.

Dragging the black hole over desktop files cannot import them because moving the hit target is not a Finder drag session. Only an external file drag entering the target and being released inside the core starts an import.

## Manual Dragging

Pressing and dragging inside the black-hole core updates the global center immediately. The GPU receives the new center every display frame.

Normal dragging is not clamped to `visibleFrame`. The center may reach the menu bar, Dock, display edges, and display seams. If a display is disconnected and the saved center no longer belongs to any display, CYLUNE recovers it to the primary display.

The last center is persisted and restored on the next launch. Automatic sine-wave wandering from the upstream project is disabled.

## Size Adjustment

The existing CYLUNE main-app black-hole size control remains the source of truth.

The selected size is converted to a physical pixel radius and passed to every display pane. Changing the setting updates the renderer and hit-target panel live. The effect retains the upstream proportions at every size.

The supported range remains the range exposed by CYLUNE settings. The largest value must be substantially larger than the former maximum and may occupy roughly 40% of the shorter display dimension, as previously requested.

The selected value is persisted and restored.

## File Import

The upstream recycle-bin path is not copied.

The existing CYLUNE import pipeline remains responsible for:

- accepting `.3mf`, `.gcode.3mf`, and `.gcode`;
- rejecting directories, unsupported extensions, and invalid paths;
- parsing the file and creating the print record;
- showing import success or failure in the main app;
- preserving the source file.

Dropping a valid file into the core calls the existing import pipeline. The source file is never moved to Trash or deleted.

## Settings and Presets

The upstream nine visual presets may be exposed through the existing black-hole style setting, but their shader constants remain unchanged.

Frame-rate selection, light/dark UI mode, languages, and the rest of CYLUNE settings continue to behave independently. They must not alter the accepted visual other than the selected upstream preset, size, and frame rate.

## Error Handling

- Without screen-recording permission, the renderer uses the current desktop wallpaper, matching upstream behavior.
- If live display capture fails, the black hole remains visible with the wallpaper fallback.
- If Metal is unavailable, CYLUNE reports that the full-effect mode is unavailable and keeps the non-effect fallback selectable.
- Import failure leaves the source file untouched and reports the existing parser error.

## Verification

The integration is accepted only when all of the following pass:

1. Side-by-side screenshots and a ten-second recording match the upstream renderer at the same preset, size, brightness, and speed.
2. The effect continues animating while stationary; moving it does not create a trailing capture rectangle or one-frame black border.
3. The core can be dragged to every physical edge and corner without snapping back.
4. Crossing between displays does not jump, duplicate, or change scale.
5. Moving the black hole across desktop files never imports them.
6. Dropping a supported print file inside the core imports it exactly once.
7. Dropping outside the core or dropping an unsupported file imports nothing.
8. Size changes from the main app update immediately and persist after restart.
9. The source print file remains in its original location after import.
10. Existing frontend, Rust, native lifecycle, parser, and import tests remain green.

