import { useEffect, useRef, useState } from "react";
import "./SuccessCelebration.css";

const FULL_DURATION_MS = 4000;
const REDUCED_DURATION_MS = 700;
const CONFETTI_COUNT = 170;
const BURST_COUNT = 7;
const SPARKS_PER_BURST = 34;
const BURST_DELAYS = [0, 80, 360, 620, 900, 1250, 1600];

type Particle = {
  kind: "confetti" | "spark";
  x: number;
  y: number;
  vx: number;
  vy: number;
  gravity: number;
  drag: number;
  size: number;
  rotation: number;
  spin: number;
  life: number;
  decay: number;
  color: string;
  startAt: number;
};

type CanvasSize = {
  width: number;
  height: number;
};

export type SuccessCelebrationProps = {
  playId: number;
};

function randomBetween(minimum: number, maximum: number) {
  return minimum + Math.random() * (maximum - minimum);
}

function themePalette() {
  const styles = getComputedStyle(document.documentElement);
  const colors = ["--coral", "--yellow", "--blue", "--mint"]
    .map((name) => styles.getPropertyValue(name).trim())
    .filter(Boolean);
  return colors.length ? colors : [styles.color];
}

function sizeCanvas(
  canvas: HTMLCanvasElement,
  context: CanvasRenderingContext2D,
): CanvasSize {
  const bounds = canvas.getBoundingClientRect();
  const ratio = Math.min(window.devicePixelRatio || 1, 2);
  const width = Math.max(1, bounds.width);
  const height = Math.max(1, bounds.height);
  canvas.width = Math.max(1, Math.floor(width * ratio));
  canvas.height = Math.max(1, Math.floor(height * ratio));
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  return { width, height };
}

function createConfetti(size: CanvasSize, colors: string[]): Particle[] {
  return Array.from({ length: CONFETTI_COUNT }, (_, index) => {
    const fromEdge = index < CONFETTI_COUNT * 0.45;
    const fromLeft = index % 2 === 0;
    return {
      kind: "confetti",
      x: fromEdge
        ? (fromLeft ? size.width * 0.08 : size.width * 0.92)
        : randomBetween(size.width * 0.22, size.width * 0.78),
      y: fromEdge
        ? randomBetween(size.height * 0.72, size.height * 0.9)
        : randomBetween(size.height * 0.04, size.height * 0.18),
      vx: fromEdge
        ? (fromLeft ? 1 : -1) * randomBetween(2.1, 5.7)
        : randomBetween(-1.65, 1.65),
      vy: fromEdge ? -randomBetween(4.8, 9.6) : -randomBetween(1.4, 4.4),
      gravity: randomBetween(0.055, 0.1),
      drag: randomBetween(0.988, 0.996),
      size: randomBetween(5, 10),
      rotation: randomBetween(0, Math.PI * 2),
      spin: randomBetween(-0.28, 0.28),
      life: 1,
      decay: randomBetween(0.00024, 0.00036),
      color: colors[index % colors.length],
      startAt: randomBetween(0, 900),
    };
  });
}

function burstPosition(index: number, size: CanvasSize) {
  if (index === 0) return { x: size.width * 0.2, y: size.height * 0.34 };
  if (index === 1) return { x: size.width * 0.8, y: size.height * 0.34 };
  return {
    x: randomBetween(size.width * 0.12, size.width * 0.88),
    y: randomBetween(size.height * 0.16, size.height * 0.58),
  };
}

function createSparks(size: CanvasSize, colors: string[]): Particle[] {
  return Array.from({ length: BURST_COUNT }, (_, burstIndex) => {
    const origin = burstPosition(burstIndex, size);
    return Array.from({ length: SPARKS_PER_BURST }, (_, sparkIndex) => {
      const angle = (Math.PI * 2 * sparkIndex) / SPARKS_PER_BURST
        + randomBetween(-0.08, 0.08);
      const speed = randomBetween(2.3, 5.9);
      return {
        kind: "spark" as const,
        x: origin.x,
        y: origin.y,
        vx: Math.cos(angle) * speed,
        vy: Math.sin(angle) * speed,
        gravity: randomBetween(0.018, 0.038),
        drag: randomBetween(0.966, 0.982),
        size: randomBetween(1.5, 3.2),
        rotation: 0,
        spin: 0,
        life: 1,
        decay: randomBetween(0.00072, 0.00105),
        color: colors[(burstIndex + sparkIndex) % colors.length],
        startAt: BURST_DELAYS[burstIndex],
      };
    });
  }).flat();
}

function drawParticle(
  context: CanvasRenderingContext2D,
  particle: Particle,
) {
  context.save();
  context.globalAlpha = Math.max(0, Math.min(1, particle.life));
  context.translate(particle.x, particle.y);
  context.fillStyle = particle.color;
  if (particle.kind === "confetti") {
    context.rotate(particle.rotation);
    context.fillRect(
      -particle.size / 2,
      -particle.size * 0.3,
      particle.size,
      particle.size * 0.6,
    );
  }
  else {
    context.shadowBlur = 9;
    context.shadowColor = particle.color;
    context.beginPath();
    context.arc(0, 0, particle.size, 0, Math.PI * 2);
    context.fill();
  }
  context.restore();
}

function startCelebration(
  canvas: HTMLCanvasElement,
  context: CanvasRenderingContext2D,
) {
  let size = sizeCanvas(canvas, context);
  const colors = themePalette();
  const particles = [
    ...createConfetti(size, colors),
    ...createSparks(size, colors),
  ];
  const startedAt = performance.now();
  let previousFrame = startedAt;
  let animationFrame = 0;

  const draw = (now: number) => {
    const elapsed = now - startedAt;
    const delta = Math.min(32, Math.max(0, now - previousFrame));
    const frameScale = delta / (1000 / 60);
    previousFrame = now;
    context.clearRect(0, 0, size.width, size.height);

    let hasWork = false;
    for (const particle of particles) {
      if (elapsed < particle.startAt) {
        hasWork = true;
        continue;
      }
      if (particle.life <= 0) continue;
      hasWork = true;
      particle.vx *= Math.pow(particle.drag, frameScale);
      particle.vy = particle.vy * Math.pow(particle.drag, frameScale)
        + particle.gravity * frameScale;
      particle.x += particle.vx * frameScale;
      particle.y += particle.vy * frameScale;
      particle.rotation += particle.spin * frameScale;
      particle.life -= particle.decay * delta;
      drawParticle(context, particle);
    }

    if (hasWork) animationFrame = window.requestAnimationFrame(draw);
  };

  const resize = () => {
    const previousSize = size;
    size = sizeCanvas(canvas, context);
    const scaleX = size.width / previousSize.width;
    const scaleY = size.height / previousSize.height;
    for (const particle of particles) {
      particle.x *= scaleX;
      particle.y *= scaleY;
    }
  };

  window.addEventListener("resize", resize);
  animationFrame = window.requestAnimationFrame(draw);
  return () => {
    window.removeEventListener("resize", resize);
    window.cancelAnimationFrame(animationFrame);
  };
}

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
  return <div
    className="success-celebration"
    data-testid="success-celebration"
    data-motion={run.reduced ? "reduced" : "full"}
    aria-hidden="true"
    style={{ pointerEvents: "none" }}
  >
    {run.reduced
      ? <div className="success-celebration__stars" />
      : <canvas ref={canvasRef} />}
  </div>;
}
