# Print Success Celebration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Play one four-second, full-density fireworks and confetti celebration inside the CYLUNE app window only after a successful print settlement has been persisted.

**Architecture:** Add a presentation-only SuccessCelebration component at the root of the existing app shell. DesktopApp owns a monotonically increasing play token and increments it only after settleJob resolves for a success outcome; the component owns Canvas rendering, reduced-motion fallback, and cleanup without reading business data.

**Tech Stack:** React 18, TypeScript, Canvas 2D, CSS, Vitest, Testing Library

---

## File map

- Create src/components/SuccessCelebration.tsx for lifecycle, particles, Canvas rendering, reduced-motion fallback, and cleanup.
- Create src/components/SuccessCelebration.css for the window-confined, non-interactive overlay.
- Create src/components/SuccessCelebration.test.tsx for lifecycle, replay, reduced-motion, and cleanup coverage.
- Modify src/App.tsx to emit a one-shot play token after persisted success.
- Modify src/App.test.tsx to verify the real settlement boundary.

### Task 1: Build the isolated celebration component

**Files:**
- Create: src/components/SuccessCelebration.test.tsx
- Create: src/components/SuccessCelebration.tsx
- Create: src/components/SuccessCelebration.css

- [ ] **Step 1: Write the failing component tests**

Create SuccessCelebration.test.tsx with matchMedia, Canvas, requestAnimationFrame, and timer mocks. Cover the exact public contract:

~~~tsx
const { rerender } = render(<SuccessCelebration playId={0} />);
expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

rerender(<SuccessCelebration playId={1} />);
expect(screen.getByTestId("success-celebration")).toHaveAttribute("data-motion", "full");
expect(screen.getByTestId("success-celebration")).toHaveStyle({ pointerEvents: "none" });

act(() => vi.advanceTimersByTime(3999));
expect(screen.getByTestId("success-celebration")).toBeVisible();
act(() => vi.advanceTimersByTime(1));
expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

rerender(<SuccessCelebration playId={1} />);
expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();
rerender(<SuccessCelebration playId={2} />);
expect(screen.getByTestId("success-celebration")).toBeVisible();
~~~

Add a reduced-motion case that returns true only for (prefers-reduced-motion: reduce), expects data-motion="reduced", expects no canvas, advances 700 ms, and expects the overlay to be removed. Add an unmount case that verifies cancelAnimationFrame and clearTimeout are called.

- [ ] **Step 2: Run the component test to verify RED**

Run:

~~~bash
npm test -- --run src/components/SuccessCelebration.test.tsx
~~~

Expected: FAIL because ./SuccessCelebration does not exist.

- [ ] **Step 3: Implement the exact component contract**

Create SuccessCelebration.tsx with:

~~~tsx
export type SuccessCelebrationProps = { playId: number };

const FULL_DURATION_MS = 4000;
const REDUCED_DURATION_MS = 700;

export function SuccessCelebration({ playId }: SuccessCelebrationProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const previousPlayId = useRef(0);
  const [run, setRun] = useState<{ id: number; reduced: boolean } | null>(null);

  useEffect(() => {
    if (playId <= 0 || playId === previousPlayId.current) return;
    previousPlayId.current = playId;
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    setRun({ id: playId, reduced });
    const timeout = window.setTimeout(
      () => setRun((current) => current?.id === playId ? null : current),
      reduced ? REDUCED_DURATION_MS : FULL_DURATION_MS,
    );
    return () => window.clearTimeout(timeout);
  }, [playId]);

  useEffect(() => {
    if (!run || run.reduced) return;
    const canvas = canvasRef.current;
    const context = canvas?.getContext("2d");
    if (!canvas || !context) return;
    return startCelebration(canvas, context);
  }, [run]);

  if (!run) return null;
  return (
    <div
      className="success-celebration"
      data-testid="success-celebration"
      data-motion={run.reduced ? "reduced" : "full"}
      aria-hidden="true"
      style={{ pointerEvents: "none" }}
    >
      {run.reduced
        ? <div className="success-celebration__stars" />
        : <canvas ref={canvasRef} />}
    </div>
  );
}
~~~

In the same file define a Particle type with kind, position, velocity, gravity, drag, size, rotation, spin, life, decay, color, and startAt fields. Implement startCelebration with these exact rules:

- Read --coral, --yellow, --blue, and --mint from document.documentElement.
- Size Canvas from getBoundingClientRect and cap devicePixelRatio at 2.
- Create 170 confetti particles. Forty-five percent launch from alternating left/right lower edges with horizontal speed 2.1–5.7 and upward speed 4.8–9.6. The rest begin across the upper center with horizontal speed -1.65–1.65 and upward speed 1.4–4.4.
- Create seven radial bursts of 34 sparks. Place the first two at 20% and 80% width around 34% height, and distribute five more within 12–88% width and 16–58% height. Use startAt delays of 0, 80, 360, 620, 900, 1250, and 1600 ms.
- On each frame, activate particles whose startAt has elapsed, apply drag and gravity, update rotation, render confetti as rotating rectangles and sparks as circles, and decrease life.
- Stop requesting frames when no live or future particle remains.
- Listen for resize, rebuild Canvas dimensions, and remove the listener plus cancel the current frame in the returned cleanup function.

Create SuccessCelebration.css with the exact window boundary:

~~~css
.success-celebration {
  position: fixed;
  z-index: 1000;
  inset: 0;
  overflow: hidden;
  pointer-events: none;
  contain: strict;
}

.success-celebration canvas {
  display: block;
  width: 100%;
  height: 100%;
}

.success-celebration__stars {
  position: absolute;
  inset: 0;
  background:
    radial-gradient(circle at 24% 36%, var(--yellow) 0 3px, transparent 4px),
    radial-gradient(circle at 48% 23%, var(--mint) 0 4px, transparent 5px),
    radial-gradient(circle at 72% 38%, var(--coral) 0 3px, transparent 4px),
    radial-gradient(circle at 58% 62%, var(--blue) 0 3px, transparent 4px);
  animation: success-celebration-stars 700ms ease-out both;
}

@keyframes success-celebration-stars {
  0% { opacity: 0; transform: scale(.94); filter: brightness(1); }
  35% { opacity: 1; transform: scale(1.02); filter: brightness(1.3); }
  100% { opacity: 0; transform: scale(1); filter: brightness(1); }
}
~~~

- [ ] **Step 4: Run the component tests to verify GREEN**

Run:

~~~bash
npm test -- --run src/components/SuccessCelebration.test.tsx
~~~

Expected: every SuccessCelebration test passes.

- [ ] **Step 5: Commit the isolated component**

Stage only the three new celebration files and commit with:

~~~bash
git commit -m "feat: add print success celebration overlay"
~~~

### Task 2: Trigger only after persisted successful settlement

**Files:**
- Modify: src/App.test.tsx
- Modify: src/App.tsx

- [ ] **Step 1: Write failing integration tests**

Extend the existing multi-plate settlement scenario with a deferred settlement response:

Add SettlementResult to the existing type-only import from ./lib/tauri before using the deferred promise.

~~~tsx
let resolveSettlement!: (value: SettlementResult) => void;
const settleJob = vi.fn(() => new Promise<SettlementResult>((resolve) => {
  resolveSettlement = resolve;
}));

await user.click(screen.getByRole("button", { name: "确认扣减耗材" }));
expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

await act(async () => resolveSettlement({
  job_id: "job-2",
  outcome: { kind: "success" },
  settlement_version: 1,
  reversed: false,
  selected_layer: null,
  confidence: "exact",
  consumption: [],
}));
expect(await screen.findByTestId("success-celebration")).toBeVisible();
~~~

Add one API rejection scenario that expects no overlay. Add a table-driven successful-settlement scenario for failed, cancelled, and estimated outcomes that selects the corresponding radio and fills its required layer or progress value, then expects no overlay. Existing history-opening assertions must also continue to prove that reading a stored success does not create a new play event.

- [ ] **Step 2: Run App tests to verify RED**

Run:

~~~bash
npm test -- --run src/App.test.tsx
~~~

Expected: FAIL because DesktopApp does not render or trigger SuccessCelebration.

- [ ] **Step 3: Wire the play token at the persistence boundary**

Modify App.tsx exactly at the existing settlement action:

~~~tsx
import { SuccessCelebration } from "./components/SuccessCelebration";

const [successCelebrationId, setSuccessCelebrationId] = useState(0);

const result = await apiClient.settleJob(jobId, outcome);
// Preserve the existing plateResults update.
if (outcome.kind === "success") {
  setSuccessCelebrationId((current) => current + 1);
}
~~~

Render this as the last child of app-shell, after main:

~~~tsx
<SuccessCelebration playId={successCelebrationId} />
~~~

Do not derive the token from result, plate status, getSettlementResult, project refresh, or history loading. This makes API success the only source of the one-shot event.

- [ ] **Step 4: Run component and App tests to verify GREEN**

Run:

~~~bash
npm test -- --run src/components/SuccessCelebration.test.tsx src/App.test.tsx
~~~

Expected: all targeted tests pass.

- [ ] **Step 5: Commit the business trigger**

Stage only App.tsx and App.test.tsx and commit with:

~~~bash
git commit -m "feat: celebrate persisted successful prints"
~~~

### Task 3: Verify regression safety and approved window behavior

**Files:**
- Verify only; no production file should change.

- [ ] **Step 1: Run the complete frontend test suite**

Run:

~~~bash
npm test -- --run
~~~

Expected: all frontend tests pass, including slicing, multi-plate, retry, and color matching.

- [ ] **Step 2: Run the production frontend build**

Run:

~~~bash
npm run build
~~~

Expected: TypeScript and Vite complete without errors.

- [ ] **Step 3: Check formatting and worktree scope**

Run git diff --check and git status --short separately.

Expected: no whitespace errors. Existing slice/parser changes and the user-owned untracked result.json remain untouched. Never add, edit, delete, or stage result.json.

- [ ] **Step 4: Perform a local visual smoke test without packaging**

Run:

~~~bash
npm run tauri dev
~~~

Settle one mapped task as successful and verify that the animation fills the CYLUNE window, remains clipped when the app moves or resizes, accepts no pointer input, and disappears after about four seconds. Verify failed/cancelled settlement produces no celebration. Stop the development process afterward.

Do not run npm run release:mac. Packaging remains deferred until all requested work is complete.
