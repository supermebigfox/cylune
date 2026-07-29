import type { CSSProperties } from "react";

export function Swatch({
  colors,
  size = "small",
}: {
  colors: string[];
  size?: "small" | "large";
}) {
  const safe = colors.length ? colors : ["#888888"];
  const background =
    safe.length === 1
      ? safe[0]
      : `linear-gradient(135deg, ${safe
          .map(
            (color, index) =>
              `${color} ${(index / (safe.length - 1)) * 100}%`,
          )
          .join(", ")})`;

  return (
    <i
      data-testid="swatch"
      aria-hidden="true"
      className={`swatch swatch-${size}`}
      style={{ "--swatch": safe[0], background } as CSSProperties}
    />
  );
}
