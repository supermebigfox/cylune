# CYLUNE Launch Video Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a production-ready CYLUNE launch-video kit containing the locked 70-second script, shot-specific Seedance prompts, an exact user asset checklist, and a post-production workflow for a 4K Apple-keynote-inspired horizontal film.

**Architecture:** Keep the creative source of truth in focused Markdown files under `video/`. Real CYLUNE screen recordings remain authoritative for UI, text, data, and the black-hole interaction; Seedance 2.0 generates only abstract transitions and the final Logo transformation. A single README routes the user through capture, generation, editing, and QA without duplicating the detailed instructions.

**Tech Stack:** Seedance 2.0 reference-to-video, macOS screen recording, DaVinci Resolve or Final Cut Pro, optional After Effects, FFmpeg/ffprobe for validation, Markdown documentation.

---

## File Map

- Create: `video/README.md` — production order, tool selection, folder conventions, and final deliverables.
- Create: `video/script.md` — locked 70-second timeline, screen copy, motion, and sound cues.
- Create: `video/prompts.md` — shot-by-shot Seedance inputs, modes, prompts, negative constraints, and retry rules.
- Create: `video/assets.md` — exact recording and still-image checklist, naming convention, privacy checks, and capture settings.
- Create: `video/edit.md` — assembly timeline, typography, audio, color, export, and QA specifications.

### Task 1: Create the production guide and folder contract

**Files:**
- Create: `video/README.md`

- [ ] **Step 1: Write the production guide**

Document this fixed sequence:

```text
01 Capture real CYLUNE footage
02 Export clean UI stills and approved brand assets
03 Generate only shots 04 and 11 in Seedance 2.0
04 Assemble the 12-shot edit at 3840×2160 / 30fps
05 Add post-produced English typography and designed audio
06 Validate picture, spelling, data accuracy, privacy, and export files
```

Specify Seedance 2.0 reference-to-video as the primary model, with Runway/Kling as optional fallback only for the two abstract shots. State that no model may regenerate CYLUNE UI, text, data, or the black hole.

- [ ] **Step 2: Verify routing and prohibited uses**

Run:

```bash
rg -n "Seedance 2.0|shot|UI|black hole|3840|30fps" video/README.md
```

Expected: all six production stages, primary model, real-UI rule, resolution, and frame rate are present.

- [ ] **Step 3: Commit the guide**

```bash
git add video/README.md
git commit -m "docs: add CYLUNE video production guide"
```

### Task 2: Write the locked shooting script

**Files:**
- Create: `video/script.md`

- [ ] **Step 1: Write all 12 shots with exact timing**

The shot boundaries must be:

```text
01 00:00–00:04
02 00:04–00:08
03 00:08–00:14
04 00:14–00:18
05 00:18–00:24
06 00:24–00:31
07 00:31–00:38
08 00:38–00:46
09 00:46–00:53
10 00:53–01:01
11 01:01–01:06
12 01:06–01:10
```

For every shot, include picture, camera, product action, edit transition, on-screen copy, music, and sound-effect cues. Use only these approved text strings:

```text
Drop into focus.
Your print. Understood.
Every plate.
Every color.
Every gram.
All on your Mac.
Accounted for.
Every gram, in view.
Available now.
```

- [ ] **Step 2: Verify timing and copy**

Run:

```bash
rg -n "00:00|00:04|00:08|00:14|00:18|00:24|00:31|00:38|00:46|00:53|01:01|01:06|01:10" video/script.md
rg -n "Drop into focus|Your print\. Understood|Every plate|Every color|Every gram|All on your Mac|Accounted for|Every gram, in view|Available now" video/script.md
```

Expected: 12 contiguous shot ranges ending at 01:10 and all approved copy appears with no alternative wording.

- [ ] **Step 3: Commit the script**

```bash
git add video/script.md
git commit -m "docs: write CYLUNE launch film script"
```

### Task 3: Build the Seedance prompt pack

**Files:**
- Create: `video/prompts.md`

- [ ] **Step 1: Define the model and reference policy**

Record these production decisions:

```text
Primary model: Seedance 2.0
Primary mode: multimodal reference-to-video / 全能参考
Generated shots: 04 and 11 only
Shot 04 duration: 5 seconds, trim to 4 seconds
Shot 11 duration: 5 seconds
Aspect ratio: 16:9
Generation resolution: highest native option available; upscale after selection when needed
Generated audio: off for master production
Variations: 4 initial generations per shot, then 2 controlled retries for the selected direction
```

- [ ] **Step 2: Write prompts for shot 04**

Include an input map for the real black-hole exit frame, the ice-white poster reference, and the CYLUNE gradient palette. Provide one full Chinese prompt, one matching English prompt, global negative constraints, and two controlled retry prompts: reduce excessive energy and strengthen forward motion.

- [ ] **Step 3: Write prompts for shot 11**

Include an input map for the final celebration frame, transparent CYLUNE mark, and ice-white end-card reference. Require confetti to become fine cobalt-blue/electric-violet filaments that spiral clockwise and align toward the mark, while forbidding readable text and Logo redesign. Provide Chinese and English prompts plus retries for mark deformation and chaotic particles.

- [ ] **Step 4: Add fallback instructions**

State that if Logo consistency fails twice, the model generates only converging light filaments; the actual transparent Logo is revealed with a matte in the editor. If shot 04 cannot preserve the real black-hole geometry, use a hard light-wipe transition made in the editor rather than a generated replacement black hole.

- [ ] **Step 5: Validate the prompt pack**

Run:

```bash
rg -n "Seedance 2.0|全能参考|Shot 04|Shot 11|负面|Negative|Logo|UI|5 秒|16:9|4 initial" video/prompts.md
```

Expected: both shots have model settings, reference mappings, bilingual prompts, negative constraints, retry rules, and editor fallbacks.

- [ ] **Step 6: Commit the prompt pack**

```bash
git add video/prompts.md
git commit -m "docs: add Seedance prompt pack"
```

### Task 4: Create the capture-ready asset checklist

**Files:**
- Create: `video/assets.md`

- [ ] **Step 1: Define the folder and filename convention**

Use this structure without duplicating existing brand masters:

```text
video/assets/capture/blackhole/
video/assets/capture/ui/
video/assets/stills/
video/assets/generated/shot04/
video/assets/generated/shot11/
video/assets/audio/music/
video/assets/audio/sfx/
video/exports/review/
video/exports/master/
```

Reference existing brand files from `brand/` instead of copying them.

- [ ] **Step 2: Write the exact capture list**

For each desktop and UI clip, specify filename, 3840×2160/60fps capture, minimum handles, cursor visibility, intended shot, and acceptance check. Include the idle black hole, file approach and ingestion, slicing progress, multiplate result, filament library, AMS mapping, print history, task detail, and successful settlement celebration.

- [ ] **Step 3: Add demo-data and privacy requirements**

Require a display-cleared 3–5-color model named `CYLUNE Demo.3mf`, a multiplate version, 6–8 official Bambu spools, two same-color spools with different balances, a generic printer model name, consistent dates, no notifications, no personal paths, and no user account data.

- [ ] **Step 4: Validate checklist completeness**

Run:

```bash
rg -n "3840×2160|60fps|CYLUNE Demo|同色|AMS|隐私|通知|multiplate|success|blackhole" video/assets.md
```

Expected: capture specs, every product feature, demo-data rules, and privacy checks are present.

- [ ] **Step 5: Commit the asset checklist**

```bash
git add video/assets.md
git commit -m "docs: add CYLUNE video asset checklist"
```

### Task 5: Write the edit, sound, export, and QA workflow

**Files:**
- Create: `video/edit.md`

- [ ] **Step 1: Define the edit project**

Specify a 3840×2160, 30fps, 70-second Rec.709 timeline. Require 60fps screen recordings to be interpreted normally unless a deliberate slow-down is listed. Define the dark-to-ice-white-to-dark/brand visual arc and preserve real UI pixels at 100% scale whenever text must remain readable.

- [ ] **Step 2: Define typography and title animation**

Use Inter Display Medium, no more than four words per feature card, restrained tracking, and editor-rendered text only. Define fade/translate timing, safe margins, end-card hierarchy, and direct use of the existing CYLUNE wordmark.

- [ ] **Step 3: Define music and sound design**

Specify a 108 BPM instrumental cue, no vocals, no copyrighted Apple music, low-frequency gravity at the opening, suction and jet at shot 03, crystal UI ticks in shots 05–09, celebration transient in shot 10, and percussion removal after 01:05.

- [ ] **Step 4: Define exports and technical QA**

Require:

```text
Master: ProRes 422 HQ, 3840×2160, 30fps, 48kHz/24-bit stereo
Upload: H.265 Main10, 3840×2160, 30fps, 35–55 Mbps, AAC 48kHz/320kbps
Review: H.264, 1920×1080, 30fps, 12–20 Mbps
```

Include ffprobe checks for width, height, frame rate, duration, codecs, sample rate, and channel count; include visual checks for spelling, real data, Logo shape, AI flicker, edge artifacts, privacy, and window-only celebration.

- [ ] **Step 5: Verify and commit the workflow**

Run:

```bash
rg -n "3840×2160|30fps|Rec\.709|Inter Display|108 BPM|ProRes 422 HQ|Main10|ffprobe|Available now" video/edit.md
git add video/edit.md
git commit -m "docs: add CYLUNE launch video finishing workflow"
```

Expected: timeline, typography, audio, export, and QA requirements are all present and the commit succeeds.

### Task 6: Cross-document quality review

**Files:**
- Modify if needed: `video/README.md`
- Modify if needed: `video/script.md`
- Modify if needed: `video/prompts.md`
- Modify if needed: `video/assets.md`
- Modify if needed: `video/edit.md`

- [ ] **Step 1: Scan for placeholders and contradictions**

Run:

```bash
rg -n "TBD|TODO|待定|稍后|自行填写|30fps or|30 或 60|Coming Soon" video
```

Expected: no matches.

- [ ] **Step 2: Confirm all source-of-truth constraints**

Run:

```bash
rg -n "70|3840×2160|30fps|Seedance 2.0|真实|Available now" video/*.md
```

Expected: each relevant document agrees on duration, resolution, frame rate, model policy, real-product footage, and release status.

- [ ] **Step 3: Confirm repository cleanliness boundaries**

Run:

```bash
git status --short
```

Expected: only the intended `video/` documentation is changed by this plan; pre-existing slicing files and the user-owned untracked `result.json` remain untouched.

- [ ] **Step 4: Commit review fixes only if needed**

```bash
git add video/README.md video/script.md video/prompts.md video/assets.md video/edit.md
git commit -m "docs: finalize CYLUNE launch video kit"
```

Expected: create this commit only when the review required corrections; otherwise leave the previously focused commits unchanged.
