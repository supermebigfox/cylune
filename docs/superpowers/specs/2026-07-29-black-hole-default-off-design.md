# Black Hole Default-Off Design

**Status:** Approved for implementation
**Date:** 2026-07-29

## Goal

Make the desktop black hole an explicit, persistent on/off feature:

- A fresh installation starts with the black hole off.
- The first time the user turns it on, macOS requests Screen Recording permission.
- The selected on/off state survives app restarts.
- The existing black-hole appearance, shader, motion, file ingestion, and jet animation remain unchanged.

## Settings UI

Replace the user-facing rendering-mode selector with a master black-hole switch:

| Locale | Left option | Right option |
| --- | --- | --- |
| Simplified Chinese | 开启黑洞 | 关闭黑洞 |
| Traditional Chinese | 開啟黑洞 | 關閉黑洞 |
| English | Turn on | Turn off |

The settings group describes black-hole status, not rendering modes. The former
“真实扭曲 / 轻量模式” choice is no longer exposed to users.

The existing “隐藏黑洞 / 显示黑洞” control remains:

- It is enabled only while the master state is on.
- Hiding the black hole does not turn the feature off.
- While the master state is off, the control is disabled.

## State Semantics

The existing requested modes remain available internally to avoid a risky native
runtime migration:

- Requested `Real` means the master black-hole feature is on.
- Requested `Lite` means the master black-hole feature is off.
- `effective_mode = Lite` may still be used internally as a compatibility
  fallback when the user requested `Real` but live capture or Metal is
  unavailable. It is not a user-selectable “lightweight mode”.
- `visible` is subordinate to the master state and represents only the
  hide/show choice.

Effective visibility is therefore:

```text
requested mode is Real AND visible is true
```

## State Transitions

### Turn on

Selecting “开启黑洞” applies one atomic settings change:

```json
{ "mode": "real", "visible": true }
```

The app saves the state, starts the existing black-hole runtime, and requests
Screen Recording permission when macOS has not decided it yet.

### Turn off

Selecting “关闭黑洞” applies:

```json
{ "mode": "lite", "visible": false }
```

The app saves the state and stops/hides capture and rendering immediately. It
does not request Screen Recording permission.

### Hide and show

While enabled, “隐藏黑洞” and “显示黑洞” update only `visible`. The requested
mode remains `Real`, so restarting the app preserves the enabled-but-hidden
state.

## First Launch and Existing-User Compatibility

The persisted state loader uses these rules:

1. An explicit saved `visible` value always wins.
2. If `mode` is missing, it defaults to `Lite`.
3. If `visible` is missing, it is derived from the loaded mode:
   - `Real` -> `true`
   - `Lite` -> `false`

This produces the required behavior without disabling the feature for existing
users:

| Saved state | Loaded result |
| --- | --- |
| No black-hole keys (fresh install) | Off and hidden |
| `mode = Real`, no `visible` key (older enabled user) | On and visible |
| `mode = Real`, `visible = false` | On and hidden |
| `mode = Lite` | Off and hidden |

## Permission Behavior

- The app does not request Screen Recording permission merely because it starts.
- The first user action that turns the black hole on invokes the existing macOS
  permission request path.
- If macOS requires the app to restart before capture is available, the settings
  page shows the existing restart guidance.
- Once permission is granted, later launches do not repeatedly prompt.
- If permission is denied or capture is unavailable, the requested state stays
  on and the native compatibility fallback may run. User-facing copy describes
  the actual limitation without mentioning “轻量模式” or “lightweight mode”.

## Menu Bar Behavior

The menu-bar hide/show command follows the same master state:

- When the black hole is off, hide/show is disabled and cannot silently enable
  rendering.
- When it is on, the menu-bar command toggles only `visible`.
- Menu-bar state is initialized and refreshed using effective visibility, so it
  cannot display “visible” while the master feature is off.

## Error Handling

- If saving or applying a state change fails, the settings control rolls back to
  the last confirmed settings and reports the existing error message.
- Permission denial is not treated as a failure to save the user’s “on”
  preference.
- Turning off remains available even if capture initialization previously
  failed.

## Verification

Automated coverage must include:

- Fresh store defaults to `Lite` and `visible = false`.
- Existing `Real` state without a `visible` key remains visible.
- Explicit hidden state remains hidden.
- Turning on sends `Real + visible`, persists it, and uses the permission-request
  path.
- Turning off sends `Lite + hidden`, persists it, and performs no permission
  request.
- Hide/show is disabled while off and retained while on.
- Simplified Chinese, Traditional Chinese, and English show no user-selectable
  lightweight-mode wording.
- Menu-bar visibility remains consistent with the master state.
- Existing native, frontend, ingestion, shader, and packaging tests continue to
  pass.

## Out of Scope

- Any visual change to the approved black-hole effect.
- Any change to file import, rejection, ingestion, or jet animation.
- Removing the internal compatibility renderer or renaming its native enum.
