import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { PrintProjectSummary } from "../../lib/tauri";
import { History } from "./History";

const singlePlateProject: PrintProjectSummary = {
  project_id: "project-single",
  source_file_name: "月球灯.gcode.3mf",
  imported_at: "2026-07-30T04:00:00Z",
  plate_count: 1,
  total_estimated_seconds: 18300,
  cover_asset_id: null,
  cover_url: null,
  plates: [
    {
      plate_id: "plate-single",
      project_id: "project-single",
      plate_index: 1,
      display_name: "月球灯",
      thumbnail_asset_id: null,
      thumbnail_url: null,
      estimated_seconds: 18300,
      max_layer: 412,
      status: "pending_mapping",
      filaments: [],
    },
  ],
};

const threePlateProject: PrintProjectSummary = {
  project_id: "project-three",
  source_file_name: "机械龙套件.gcode.3mf",
  imported_at: "2026-07-29T09:15:00Z",
  plate_count: 3,
  total_estimated_seconds: 24600,
  cover_asset_id: "cover-three",
  cover_url: "asset://cover-three",
  plates: [
    {
      plate_id: "plate-one",
      project_id: "project-three",
      plate_index: 1,
      display_name: "龙首",
      thumbnail_asset_id: "thumb-one",
      thumbnail_url: "asset://thumb-one",
      estimated_seconds: 7200,
      max_layer: 208,
      status: "success",
      filaments: [],
    },
    {
      plate_id: "plate-two",
      project_id: "project-three",
      plate_index: 2,
      display_name: "躯干",
      thumbnail_asset_id: "thumb-two",
      thumbnail_url: "asset://thumb-two",
      estimated_seconds: 9000,
      max_layer: 265,
      status: "estimated",
      filaments: [],
    },
    {
      plate_id: "plate-three",
      project_id: "project-three",
      plate_index: 3,
      display_name: "尾部",
      thumbnail_asset_id: "thumb-three",
      thumbnail_url: "asset://thumb-three",
      estimated_seconds: 8400,
      max_layer: 244,
      status: "skipped",
      filaments: [],
    },
  ],
};

describe("History", () => {
  beforeEach(async () => setLocale("zh-CN"));

  it("renders each project name once with plate count, import time, and text status", () => {
    render(
      <History
        pending={[singlePlateProject]}
        history={[threePlateProject]}
        onOpenProject={() => undefined}
      />,
    );

    expect(screen.getAllByText("月球灯.gcode.3mf")).toHaveLength(1);
    expect(screen.getAllByText("机械龙套件.gcode.3mf")).toHaveLength(1);
    expect(screen.getByText("共 3 盘")).toBeVisible();
    expect(screen.getAllByText(/2026/)).toHaveLength(2);
    expect(screen.getByText("等待映射")).toBeVisible();
    expect(screen.getByText("已完成")).toBeVisible();
  });

  it("opens the selected project from a semantic card button", () => {
    const onOpenProject = vi.fn();
    render(
      <History
        pending={[singlePlateProject]}
        history={[]}
        onOpenProject={onOpenProject}
      />,
    );

    fireEvent.click(
      screen.getByRole("button", { name: /打开月球灯\.gcode\.3mf/ }),
    );

    expect(onOpenProject).toHaveBeenCalledWith("project-single");
  });

  it("keeps every project card keyboard focusable", async () => {
    const user = userEvent.setup();
    render(
      <History
        pending={[singlePlateProject]}
        history={[]}
        onOpenProject={() => undefined}
      />,
    );

    await user.tab();
    expect(screen.getByRole("button", { name: /打开月球灯\.gcode\.3mf/ })).toHaveFocus();
  });

  it("labels available covers and replaces failed media with the CYLUNE fallback", () => {
    render(
      <History
        pending={[]}
        history={[threePlateProject]}
        onOpenProject={() => undefined}
      />,
    );

    const cover = screen.getByRole("img", {
      name: "机械龙套件.gcode.3mf 的项目封面",
    });
    const card = cover.closest("article");
    fireEvent.error(cover);

    expect(
      within(card as HTMLElement).getByText("预览无法显示，已使用 CYLUNE 标记"),
    ).toBeVisible();
    expect(
      within(card as HTMLElement).getByRole("img", { name: "CYLUNE 图标" }),
    ).toBeVisible();
  });

  it("uses the same visible CYLUNE fallback when no cover exists", () => {
    render(
      <History
        pending={[singlePlateProject]}
        history={[]}
        onOpenProject={() => undefined}
      />,
    );

    const card = screen
      .getByRole("button", { name: /打开月球灯\.gcode\.3mf/ })
      .closest("article");
    expect(within(card as HTMLElement).getByText(
      "没有项目预览，已使用 CYLUNE 标记",
    )).toBeVisible();
    expect(
      within(card as HTMLElement).getByRole("img", { name: "CYLUNE 图标" }),
    ).toBeVisible();
  });

  it("renders distinct empty states for pending and settled history", () => {
    render(
      <History pending={[]} history={[]} onOpenProject={() => undefined} />,
    );

    expect(screen.getByText("没有待处理的打印项目")).toBeVisible();
    expect(screen.getByText("还没有已完成的打印项目")).toBeVisible();
  });
});
