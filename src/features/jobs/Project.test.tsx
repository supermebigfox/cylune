import { fireEvent, render, screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type {
  ImportPreview,
  PrintProjectDetail,
  SettlementResult,
  Spool,
} from "../../lib/tauri";
import { formatDuration, Project } from "./Project";

const project: PrintProjectDetail = {
  project_id: "project-dragon",
  source_hash: "hash-dragon",
  source_file_name: "机械龙套件.gcode.3mf",
  source_path: "/prints/机械龙套件.gcode.3mf",
  imported_at: "2026-07-30T04:00:00Z",
  plate_count: 3,
  total_estimated_seconds: 30900,
  cover_asset_id: null,
  cover_url: null,
  plates: [
    {
      plate_id: "plate-head",
      project_id: "project-dragon",
      plate_index: 1,
      display_name: "龙首",
      thumbnail_asset_id: null,
      thumbnail_url: null,
      estimated_seconds: 18300,
      max_layer: 208,
      status: "pending_mapping",
    },
    {
      plate_id: "plate-body",
      project_id: "project-dragon",
      plate_index: 2,
      display_name: "躯干",
      thumbnail_asset_id: null,
      thumbnail_url: null,
      estimated_seconds: 7200,
      max_layer: 265,
      status: "estimated",
    },
    {
      plate_id: "plate-tail",
      project_id: "project-dragon",
      plate_index: 3,
      display_name: "尾部",
      thumbnail_asset_id: null,
      thumbnail_url: null,
      estimated_seconds: 5400,
      max_layer: 144,
      status: "skipped",
    },
  ],
};

const spool: Spool = {
  spool_id: "spool-blue",
  display_name: "钴蓝 PLA",
  preset_id: "Bambu PLA Basic @BBL A1",
  preset_base: null,
  catalog_id: null,
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_name: "钴蓝",
  color_code: "10602",
  color_hex: "#1C4EBB",
  color_hexes: ["#1C4EBB"],
  remaining_grams: 721.3,
  status: "available",
};

const preview: ImportPreview = {
  job_id: "job-head",
  source_hash: "hash-dragon",
  source_file_name: "机械龙套件.gcode.3mf",
  max_layer: 208,
  state: "new",
  filaments: [
    {
      tool: 0,
      profile: {
        tool: 0,
        preset_id: "Bambu PLA Basic @BBL A1",
        brand: "Bambu Lab",
        material: "PLA",
        series: "Basic",
        color_hex: "#1C4EBB",
        diameter_mm: 1.75,
        density_g_cm3: 1.26,
      },
      total_grams: 42.7,
      candidate_spool_ids: ["spool-blue"],
      suggested_spool_id: "spool-blue",
      confidence: "exact",
    },
  ],
};

const estimatedResult: SettlementResult = {
  job_id: "job-body",
  outcome: { kind: "estimated", progress_percent: 63 },
  settlement_version: 1,
  selected_layer: null,
  confidence: "estimated",
  consumption: [
    {
      spool_id: "spool-blue",
      grams: 18.4,
      confidence: "estimated",
      slot_number: 1,
    },
  ],
};

const baseActions = {
  spools: [spool],
  onConfirmMapping: async () => undefined,
  onSettle: async () => undefined,
  onConfirmNewPrint: async () => undefined,
  onReverse: async () => undefined,
};

describe("formatDuration", () => {
  it("formats an exact five-hour five-minute value in every supported locale", () => {
    expect(formatDuration(18300, "zh-CN")).toBe("5 小时 5 分钟");
    expect(formatDuration(18300, "zh-TW")).toBe("5 小時 5 分鐘");
    expect(formatDuration(18300, "en")).toBe("5 hr 5 min");
  });
});

describe("Project", () => {
  beforeEach(async () => setLocale("zh-CN"));

  it("shows one project heading, total progress, and one button per plate", () => {
    render(
      <Project
        {...baseActions}
        project={project}
        selectedPlateId={null}
        onSelectPlate={() => undefined}
      />,
    );

    expect(screen.getAllByText("机械龙套件.gcode.3mf")).toHaveLength(1);
    expect(screen.getByText("2 / 3 盘已处理")).toBeVisible();
    expect(screen.getByRole("progressbar", { name: "项目总进度" })).toHaveAttribute(
      "value",
      "2",
    );
    expect(screen.getAllByRole("button", { name: /打开第 \d 盘/ })).toHaveLength(3);
    expect(screen.getByText("5 小时 5 分钟")).toBeVisible();
    expect(screen.getByText("208 层")).toBeVisible();
  });

  it("selects a plate and embeds the existing Job workspace without repeating the project name", () => {
    const onSelectPlate = vi.fn();
    const { rerender } = render(
      <Project
        {...baseActions}
        project={project}
        selectedPlateId={null}
        onSelectPlate={onSelectPlate}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /打开第 1 盘/ }));
    expect(onSelectPlate).toHaveBeenCalledWith("plate-head");

    rerender(
      <Project
        {...baseActions}
        project={project}
        preview={preview}
        selectedPlateId="plate-head"
        onSelectPlate={onSelectPlate}
      />,
    );

    expect(screen.getAllByText("机械龙套件.gcode.3mf")).toHaveLength(1);
    expect(screen.getByText("Bambu PLA Basic @BBL A1")).toBeVisible();
    expect(screen.getAllByText("42.7 克")).toHaveLength(2);
    expect(screen.getByText("#1C4EBB")).toBeVisible();
    expect(
      screen.getByRole("group", { name: "这次打印的结果" }),
    ).toBeVisible();
  });

  it("shows an estimated settled result and keeps reversal available", () => {
    const onReverse = vi.fn();
    render(
      <Project
        {...baseActions}
        project={project}
        result={estimatedResult}
        selectedPlateId="plate-body"
        onSelectPlate={() => undefined}
        onReverse={onReverse}
      />,
    );

    const detail = screen.getByRole("region", { name: "第 2 盘详情" });
    expect(within(detail).getByText("估算结果")).toBeVisible();
    expect(within(detail).getByText("已扣减 18.4 克")).toBeVisible();
    fireEvent.click(
      within(detail).getByRole("button", { name: "撤销本次扣减" }),
    );
    expect(onReverse).toHaveBeenCalledWith("job-body");
  });

  it("states that a skipped plate has no deduction and exposes no deduction control", () => {
    render(
      <Project
        {...baseActions}
        project={project}
        selectedPlateId="plate-tail"
        onSelectPlate={() => undefined}
      />,
    );

    const detail = screen.getByRole("region", { name: "第 3 盘详情" });
    expect(within(detail).getByText("已跳过，没有扣减耗材")).toBeVisible();
    expect(
      within(detail).queryByRole("button", { name: /扣减|撤销/ }),
    ).not.toBeInTheDocument();
  });

  it("shows an actionable empty detail when a pending plate preview is unavailable", () => {
    render(
      <Project
        {...baseActions}
        project={project}
        selectedPlateId="plate-head"
        onSelectPlate={() => undefined}
      />,
    );

    expect(screen.getByText("正在准备这盘的耗材映射")).toBeVisible();
  });
});
