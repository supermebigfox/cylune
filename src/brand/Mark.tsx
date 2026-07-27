import markUrl from "../assets/brand/filament-mark.svg";

type MarkProps = {
  className?: string;
  label?: string;
  size?: number;
};

export function Mark({ className, label, size = 24 }: MarkProps) {
  const accessibility = label
    ? { alt: label }
    : { alt: "", "aria-hidden": true as const };

  return (
    <img
      {...accessibility}
      className={className}
      draggable={false}
      height={size}
      src={markUrl}
      width={size}
    />
  );
}
