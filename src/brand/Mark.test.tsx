import { render, screen } from "@testing-library/react";
import { expect, it } from "vitest";
import { Mark } from "./Mark";

it("is decorative by default at a crisp small icon size", () => {
  const { container } = render(<Mark size={18} />);
  const image = container.querySelector("img");

  expect(image).toHaveAttribute("alt", "");
  expect(image).toHaveAttribute("aria-hidden", "true");
  expect(image).toHaveAttribute("width", "18");
  expect(image).toHaveAttribute("height", "18");
});

it("becomes an accessible image when given a label", () => {
  render(<Mark size={32} label="Filament manager" />);

  expect(screen.getByRole("img", { name: "Filament manager" })).toHaveAttribute(
    "width",
    "32",
  );
});
