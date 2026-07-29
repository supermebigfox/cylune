import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { SlotView, Spool } from "../../lib/tauri";
import { Home } from "./Home";

const spools: Spool[] = [
  {
    spool_id: "spool-black-a",
    display_name: "黑色 PLA #A",
    preset_id: "Bambu PLA Basic @BBL A1",
    preset_base: null,
    catalog_id: null,
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: null,
    color_code: null,
    color_hex: "#252733",
    color_hexes: ["#252733"],
    remaining_grams: 612.4,
    status: "assigned",
  },
  {
    spool_id: "spool-black-b",
    display_name: "黑色 PLA #B",
    preset_id: "Bambu PLA Basic @BBL A1",
    preset_base: null,
    catalog_id: null,
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: null,
    color_code: null,
    color_hex: "#252733",
    color_hexes: ["#252733"],
    remaining_grams: 88.7,
    status: "available",
  },
];

const slots: SlotView[] = [
  { slot_number: 1, spool_id: "spool-black-a", spool: spools[0] },
  { slot_number: 2, spool_id: null, spool: null },
  { slot_number: 3, spool_id: null, spool: null },
  { slot_number: 4, spool_id: null, spool: null },
];

const multicolorSpool: Spool = {
  ...spools[0],
  spool_id: "spool-multicolor",
  display_name: "双色 PLA",
  color_hex: "#8EC9E9",
  color_hexes: ["#8EC9E9", "#E7C1D5"],
};

describe("Home", () => {
  beforeEach(async () => setLocale("zh-CN"));

  it("always renders the four physical AMS Lite slots", () => {
    render(
      <Home
        slots={slots}
        spools={spools}
        pendingJobs={2}
        onImport={() => undefined}
      />,
    );

    expect(screen.getAllByTestId("ams-slot")).toHaveLength(4);
    expect(screen.getByText("黑色 PLA #A")).toBeVisible();
    expect(screen.getAllByText("此槽位为空")).toHaveLength(3);
  });

  it("summarizes inventory separately from slots and starts explicit import", () => {
    const onImport = vi.fn();
    render(
      <Home
        slots={slots}
        spools={spools}
        pendingJobs={2}
        onImport={onImport}
      />,
    );

    expect(screen.getByText("耗材库 2 卷")).toBeVisible();
    expect(screen.getByText("低库存 1 卷")).toBeVisible();
    expect(screen.getByText("待处理 2 个")).toBeVisible();
    fireEvent.click(screen.getByRole("button", { name: "导入切片文件" }));
    expect(onImport).toHaveBeenCalledTimes(1);
  });

  it("shows truthful grams without inventing a percentage from a 1000 gram capacity", () => {
    render(<Home slots={slots} spools={spools} pendingJobs={0} onImport={() => undefined} />);

    expect(screen.getByText("612.4 克")).toBeVisible();
    expect(screen.queryByText("61%")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("剩余 612.4 克")).not.toBeInTheDocument();
  });

  it("shows every color of a multicolor spool mounted in a slot", () => {
    render(
      <Home
        slots={[
          { slot_number: 1, spool_id: multicolorSpool.spool_id, spool: multicolorSpool },
          ...slots.slice(1),
        ]}
        spools={[multicolorSpool]}
        pendingJobs={0}
        onImport={() => undefined}
      />,
    );

    expect(screen.getByTestId("swatch")).toHaveStyle({
      background:
        "linear-gradient(135deg, #8EC9E9 0%, #E7C1D5 100%)",
    });
  });

  it("falls back to the primary color when a mounted spool has no color list", () => {
    const legacySpool: Spool = {
      ...multicolorSpool,
      spool_id: "spool-legacy",
      color_hex: "#F2A65A",
      color_hexes: [],
    };

    render(
      <Home
        slots={[
          { slot_number: 1, spool_id: legacySpool.spool_id, spool: legacySpool },
          ...slots.slice(1),
        ]}
        spools={[legacySpool]}
        pendingJobs={0}
        onImport={() => undefined}
      />,
    );

    expect(screen.getByTestId("swatch")).toHaveStyle({
      background: "#F2A65A",
    });
  });
});
