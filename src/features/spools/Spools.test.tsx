import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { Spool } from "../../lib/tauri";
import { Spools } from "./Spools";

const identicalBlackSpools: Spool[] = [
  {
    spool_id: "spool-black-a",
    display_name: "黑色 PLA #A",
    preset_id: "Bambu PLA Basic @BBL A1",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_hex: "#252733",
    remaining_grams: 612.4,
    status: "assigned",
  },
  {
    spool_id: "spool-black-b",
    display_name: "黑色 PLA #B",
    preset_id: "Bambu PLA Basic @BBL A1",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_hex: "#252733",
    remaining_grams: 88.7,
    status: "available",
  },
];

const mixedSpools: Spool[] = [
  ...identicalBlackSpools,
  {
    spool_id: "spool-red",
    display_name: "红色 PLA",
    preset_id: "Bambu PLA Matte @BBL A1",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Matte",
    color_hex: "#E54A42",
    remaining_grams: 401.2,
    status: "available",
  },
];

beforeEach(async () => setLocale("zh-CN"));

it("keeps same-color spools as independently actionable entities", async () => {
  const onCalibrate = vi.fn();
  render(
    <Spools
      spools={identicalBlackSpools}
      slotBySpool={{ "spool-black-a": 1 }}
      onCreate={async () => undefined}
      onCalibrate={onCalibrate}
      onArchive={async () => undefined}
      onMount={async () => undefined}
    />,
  );

  expect(screen.getByText("黑色 PLA #A")).toBeVisible();
  expect(screen.getByText("黑色 PLA #B")).toBeVisible();
  expect(screen.getByText("AMS 1")).toBeVisible();
  expect(screen.getByText("耗材库")).toBeVisible();
  expect(screen.getByText("612.4 克")).toBeVisible();
  expect(screen.getByText("88.7 克")).toBeVisible();

  fireEvent.click(
    screen.getAllByRole("button", { name: "校准重量" })[1],
  );
  const grams = screen.getByLabelText("新的剩余重量");
  fireEvent.change(grams, { target: { value: "76.2" } });
  fireEvent.click(screen.getByRole("button", { name: "保存校准" }));
  await waitFor(() => expect(onCalibrate).toHaveBeenCalledWith("spool-black-b", 76.2));
});

it("filters explicitly by color and mounts an available spool into a chosen slot", async () => {
  const onMount = vi.fn();
  render(
    <Spools
      spools={mixedSpools}
      slotBySpool={{ "spool-black-a": 1 }}
      onCreate={async () => undefined}
      onCalibrate={async () => undefined}
      onArchive={async () => undefined}
      onMount={onMount}
    />,
  );

  fireEvent.change(screen.getByLabelText("颜色筛选"), { target: { value: "#252733" } });
  expect(screen.getByText("黑色 PLA #A")).toBeVisible();
  expect(screen.getByText("黑色 PLA #B")).toBeVisible();
  expect(screen.queryByText("红色 PLA")).not.toBeInTheDocument();

  fireEvent.click(screen.getAllByRole("button", { name: "装入 AMS" })[1]);
  fireEvent.change(screen.getByLabelText("AMS 槽位"), { target: { value: "4" } });
  fireEvent.click(screen.getByRole("button", { name: "确认装入" }));

  await waitFor(() => expect(onMount).toHaveBeenCalledWith("spool-black-b", 4));
});
