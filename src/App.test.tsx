import { render, screen } from "@testing-library/react";
import { App } from "./App";

it("shows the local prototype shell", () => {
  render(<App />);
  expect(
    screen.getByRole("img", { name: "拓竹耗材管家图标" }),
  ).toBeVisible();
  expect(screen.getByRole("heading", { name: "拓竹耗材管家" })).toBeVisible();
  expect(screen.getByText("本地模式")).toBeVisible();
});
