import { act, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SuccessCelebration } from "./SuccessCelebration";

function installMotionPreference(reduced: boolean) {
  vi.stubGlobal("matchMedia", vi.fn((query: string) => ({
    matches: query === "(prefers-reduced-motion: reduce)" && reduced,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })));
}

function fakeCanvasContext() {
  return {
    arc: vi.fn(),
    beginPath: vi.fn(),
    clearRect: vi.fn(),
    fill: vi.fn(),
    fillRect: vi.fn(),
    restore: vi.fn(),
    rotate: vi.fn(),
    save: vi.fn(),
    setTransform: vi.fn(),
    translate: vi.fn(),
    fillStyle: "",
    globalAlpha: 1,
  } as unknown as CanvasRenderingContext2D;
}

describe("SuccessCelebration", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    installMotionPreference(false);
    vi.spyOn(HTMLCanvasElement.prototype, "getContext").mockReturnValue(fakeCanvasContext());
    vi.stubGlobal("requestAnimationFrame", vi.fn(() => 7));
    vi.stubGlobal("cancelAnimationFrame", vi.fn());
  });

  afterEach(() => {
    vi.clearAllTimers();
    vi.useRealTimers();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("appears only for a new positive play token and removes itself after four seconds", () => {
    const { rerender } = render(<SuccessCelebration playId={0} />);
    expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

    rerender(<SuccessCelebration playId={1} />);
    expect(screen.getByTestId("success-celebration")).toHaveAttribute("data-motion", "full");
    expect(screen.getByTestId("success-celebration")).toHaveStyle({ pointerEvents: "none" });

    act(() => vi.advanceTimersByTime(3999));
    expect(screen.getByTestId("success-celebration")).toBeVisible();
    act(() => vi.advanceTimersByTime(1));
    expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();
  });

  it("replays for each new token but not for an unchanged token", () => {
    const { rerender } = render(<SuccessCelebration playId={1} />);
    act(() => vi.advanceTimersByTime(4000));
    expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

    rerender(<SuccessCelebration playId={1} />);
    expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();

    rerender(<SuccessCelebration playId={2} />);
    expect(screen.getByTestId("success-celebration")).toBeVisible();
  });

  it("uses a short static fallback when reduced motion is requested", () => {
    installMotionPreference(true);
    render(<SuccessCelebration playId={1} />);

    expect(screen.getByTestId("success-celebration")).toHaveAttribute("data-motion", "reduced");
    expect(document.querySelector(".success-celebration canvas")).not.toBeInTheDocument();
    expect(document.querySelector(".success-celebration__stars")).toBeInTheDocument();

    act(() => vi.advanceTimersByTime(700));
    expect(screen.queryByTestId("success-celebration")).not.toBeInTheDocument();
  });

  it("cancels animation and timeout work when it unmounts", () => {
    const clearTimeoutSpy = vi.spyOn(window, "clearTimeout");
    const { unmount } = render(<SuccessCelebration playId={1} />);

    unmount();

    expect(cancelAnimationFrame).toHaveBeenCalledWith(7);
    expect(clearTimeoutSpy).toHaveBeenCalled();
  });
});
