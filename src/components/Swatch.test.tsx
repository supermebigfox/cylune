import { render, screen } from "@testing-library/react";
import { Swatch } from "./Swatch";

test("renders a solid color without a gradient", () => {
  render(<Swatch colors={["#FFFFFF"]} />);

  expect(screen.getByTestId("swatch")).toHaveStyle({ background: "#FFFFFF" });
});

test("keeps every gradient color in order", () => {
  render(<Swatch colors={["#8EC9E9", "#E7C1D5"]} />);

  expect(screen.getByTestId("swatch").getAttribute("style")).toContain(
    "linear-gradient(135deg, #8EC9E9 0%, #E7C1D5 100%)",
  );
});
