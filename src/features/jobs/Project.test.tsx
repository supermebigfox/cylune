import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type {
  ImportPreview,
  PrintProjectDetail,
  SettlementResult,
  Spool,
} from "../../lib/tauri";
import { formatDuration, Project } from "./Project";

const plateFilament = (tool: number, colorHex: string, grams: number) => ({
  profile: {
    tool,
    preset_id: "Bambu PLA Basic @BBL A1",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_hex: colorHex,
    diameter_mm: 1.75,
    density_g_cm3: 1.26,
  },
  total_grams: grams,
});

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
      filaments: [plateFilament(0, "#1C4EBB", 42.7)],
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
      filaments: [
        plateFilament(0, "#FE3D36", 10.3),
        plateFilament(1, "#1C4EBB", 8.1),
      ],
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
      filaments: [plateFilament(0, "#676977", 5.5)],
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
  reversed: false,
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
    const pendingPlate = screen.getByRole("button", { name: /打开第 1 盘/ });
    const settledPlate = screen.getByRole("button", { name: /打开第 2 盘/ });
    const skippedPlate = screen.getByRole("button", { name: /打开第 3 盘/ });
    expect(within(pendingPlate).getByRole("img", { name: "颜色 #1C4EBB" })).toBeVisible();
    expect(within(pendingPlate).getByText("预计共 42.7 克")).toBeVisible();
    expect(within(settledPlate).getByRole("img", { name: "颜色 #FE3D36" })).toBeVisible();
    expect(within(settledPlate).getByText("10.3 克")).toBeVisible();
    expect(within(settledPlate).getByText("8.1 克")).toBeVisible();
    expect(within(settledPlate).getByText("预计共 18.4 克")).toBeVisible();
    expect(within(skippedPlate).getByRole("img", { name: "颜色 #676977" })).toBeVisible();
    expect(within(skippedPlate).getByText("预计共 5.5 克")).toBeVisible();
  });

  it("keeps every plate card keyboard focusable", async () => {
    const user = userEvent.setup();
    render(
      <Project
        {...baseActions}
        project={project}
        selectedPlateId={null}
        onSelectPlate={() => undefined}
      />,
    );

    await user.tab();
    expect(screen.getByRole("button", { name: /打开第 1 盘/ })).toHaveFocus();
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
        preview={{ plateId: "plate-head", value: preview }}
        selectedPlateId="plate-head"
        onSelectPlate={onSelectPlate}
      />,
    );

    expect(screen.getAllByText("机械龙套件.gcode.3mf")).toHaveLength(1);
    expect(screen.getByText("Bambu PLA Basic @BBL A1")).toBeVisible();
    expect(screen.getAllByText("42.7 克")).toHaveLength(3);
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
        result={{ plateId: "plate-body", value: estimatedResult }}
        selectedPlateId="plate-body"
        onSelectPlate={() => undefined}
        onReverse={onReverse}
      />,
    );

    const detail = screen.getByRole("region", { name: "第 2 盘详情" });
    expect(within(detail).getByText("估算结果")).toBeVisible();
    expect(within(detail).getByText("已扣减 18.4 克")).toBeVisible();
    const deductions = within(detail).getByLabelText("实际扣减明细");
    expect(within(deductions).getByText("钴蓝 PLA")).toBeVisible();
    expect(within(deductions).getByText("结算时位于槽位 1")).toBeVisible();
    expect(within(deductions).getByText("18.4 克")).toBeVisible();
    fireEvent.click(
      within(detail).getByRole("button", { name: "撤销本次扣减" }),
    );
    expect(onReverse).toHaveBeenCalledWith("job-body");
  });

  it("shows the complete plate preview again in the selected plate detail", () => {
    const withThumbnail: PrintProjectDetail = {
      ...project,
      plates: project.plates.map((plate, index) => index === 0
        ? { ...plate, thumbnail_url: "asset://localhost/plate-head.png" }
        : plate),
    };
    render(
      <Project
        {...baseActions}
        project={withThumbnail}
        selectedPlateId="plate-head"
        onSelectPlate={() => undefined}
      />,
    );

    expect(screen.getAllByRole("img", { name: "第 1 盘预览" })).toHaveLength(2);
  });

  it("shows a reversed settlement as restored and removes the reversal action", () => {
    render(
      <Project
        {...baseActions}
        project={project}
        result={{
          plateId: "plate-body",
          value: { ...estimatedResult, reversed: true },
        }}
        selectedPlateId="plate-body"
        onSelectPlate={() => undefined}
      />,
    );

    const detail = screen.getByRole("region", { name: "第 2 盘详情" });
    expect(within(detail).getByText("扣减已撤销")).toBeVisible();
    expect(within(detail).getByText("已返还 18.4 克")).toBeVisible();
    expect(
      within(detail).queryByRole("button", { name: "撤销本次扣减" }),
    ).not.toBeInTheDocument();
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

  it("does not expose a stale pending preview after selection changes", () => {
    const onSettle = vi.fn();
    const { rerender } = render(
      <Project
        {...baseActions}
        onSettle={onSettle}
        project={project}
        preview={{ plateId: "plate-head", value: preview }}
        selectedPlateId="plate-head"
        onSelectPlate={() => undefined}
      />,
    );
    expect(screen.getByRole("group", { name: "这次打印的结果" })).toBeVisible();

    rerender(
      <Project
        {...baseActions}
        onSettle={onSettle}
        project={project}
        preview={{ plateId: "plate-head", value: preview }}
        selectedPlateId="plate-body"
        onSelectPlate={() => undefined}
      />,
    );

    expect(screen.queryByRole("group", { name: "这次打印的结果" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "确认扣减耗材" })).not.toBeInTheDocument();
    expect(screen.getByText("正在加载这盘的结算结果")).toBeVisible();
    expect(onSettle).not.toHaveBeenCalled();
  });

  it("does not expose a stale settled result or reversal after selection changes", () => {
    const onReverse = vi.fn();
    const { rerender } = render(
      <Project
        {...baseActions}
        onReverse={onReverse}
        project={project}
        result={{ plateId: "plate-body", value: estimatedResult }}
        selectedPlateId="plate-body"
        onSelectPlate={() => undefined}
      />,
    );
    expect(screen.getByRole("button", { name: "撤销本次扣减" })).toBeVisible();

    rerender(
      <Project
        {...baseActions}
        onReverse={onReverse}
        project={project}
        result={{ plateId: "plate-body", value: estimatedResult }}
        selectedPlateId="plate-head"
        onSelectPlate={() => undefined}
      />,
    );

    expect(screen.queryByRole("button", { name: "撤销本次扣减" })).not.toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "这次打印的结果" })).not.toBeInTheDocument();
    expect(screen.getByText("正在准备这盘的耗材映射")).toBeVisible();
    expect(onReverse).not.toHaveBeenCalled();
  });
});
