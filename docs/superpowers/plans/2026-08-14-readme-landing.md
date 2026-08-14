# CYLUNE GitHub README Landing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a product-first GitHub README that explains CYLUNE and gives visitors direct macOS and Windows v1.0 downloads.

**Architecture:** Add one root `README.md` that uses the existing tracked CYLUNE mark, keeps downloads above the product description, and links to the current GitHub Release assets. No application source, build configuration, installer, or local poster asset changes.

**Tech Stack:** GitHub Flavored Markdown, HTML alignment supported by GitHub, GitHub Releases

---

### Task 1: Publish the product README

**Files:**
- Create: `README.md`
- Reference: `src/assets/brand/cylune-mark.png`
- Reference: `docs/superpowers/specs/2026-08-14-readme-landing-design.md`

- [x] **Step 1: Verify the pre-change failure state**

Run:

```bash
test ! -e README.md
```

Expected: exit 0, confirming the default branch has no root README.

- [x] **Step 2: Create the README**

Create `README.md` with these ordered sections:

1. Centered CYLUNE mark, product name, and local-first 3D-print filament-management positioning.
2. A download table containing direct links to `CYLUNE.dmg`, `CYLUNE-Setup.exe`, and the complete `v1.0` Release page.
3. A concise product introduction explaining per-spool inventory, sliced-file data, and print records.
4. Core features covering the Bambu catalog, `.3mf` and `.gcode.3mf`, task settlement, retries, black-hole import, history, languages, and themes.
5. A three-step workflow: record spools, import or slice a file, settle usage.
6. Current scope and installation notes, including Apple Silicon, Apple notarization, Windows SmartScreen, local Bambu Studio slicing, and no automatic Bambu Handy or MakerWorld task sync.
7. Release, issue, and developer links, ending with `Developer: Robin Lyu`.

- [x] **Step 3: Validate local README structure**

Run:

```bash
test -f README.md
test -f src/assets/brand/cylune-mark.png
rg -q 'CYLUNE.dmg' README.md
rg -q 'CYLUNE-Setup.exe' README.md
rg -q 'Developer: Robin Lyu' README.md
git diff --check
```

Expected: every command exits 0 and `git diff --check` prints no errors.

- [x] **Step 4: Validate public download targets**

Run:

```bash
curl -L --fail --silent --show-error --output /dev/null --write-out '%{http_code}\n' 'https://github.com/supermebigfox/cylune/releases/download/v1.0/CYLUNE.dmg'
curl -L --fail --silent --show-error --output /dev/null --write-out '%{http_code}\n' 'https://github.com/supermebigfox/cylune/releases/download/v1.0/CYLUNE-Setup.exe'
```

Expected: each command prints `200`.

- [ ] **Step 5: Commit and publish**

Run:

```bash
git add README.md docs/superpowers/plans/2026-08-14-readme-landing.md
git commit -m "docs: add CYLUNE product README"
git push origin HEAD:codex/windows-port HEAD:main
```

Expected: the commit succeeds and both remote branches point to the new commit.

- [ ] **Step 6: Verify the GitHub-rendered README**

Run:

```bash
gh api repos/supermebigfox/cylune/readme --jq '.path'
curl -L --fail --silent --show-error 'https://github.com/supermebigfox/cylune' | rg -q 'CYLUNE for macOS'
curl -L --fail --silent --show-error 'https://github.com/supermebigfox/cylune' | rg -q 'CYLUNE for Windows'
```

Expected: GitHub reports `README.md` and both rendered download labels are present on the public repository page.
