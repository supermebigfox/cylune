import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { ImportPreview, Spool } from "../../lib/tauri";
import { Job } from "./Job";

const spools: Spool[] = [
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
    remaining_grams: 488.2,
    status: "available",
  },
];

const preview: ImportPreview = {
  job_id: "job-mask",
  source_hash: "hash-mask",
  source_file_name: "萨莫面具-布莱克.gcode.3mf",
  max_layer: 186,
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
        color_hex: "#252733",
        diameter_mm: 1.75,
        density_g_cm3: 1.26,
      },
      total_grams: 42.7,
      candidate_spool_ids: ["spool-black-a", "spool-black-b"],
      suggested_spool_id: null,
      confidence: "needs_confirmation",
    },
  ],
};

describe("Job", () => {
  beforeEach(async () => setLocale("zh-CN"));

  it("requires one exact spool id when identical candidates exist", async () => {
    const onConfirmMapping = vi.fn();
    render(
      <Job
        preview={preview}
        spools={spools}
        onConfirmMapping={onConfirmMapping}
        onSettle={async () => undefined}
        onConfirmNewPrint={async () => undefined}
        onReverse={async () => undefined}
      />,
    );

    expect(screen.getByText("萨莫面具-布莱克.gcode.3mf")).toBeVisible();
    expect(screen.getByText("Bambu PLA Basic @BBL A1")).toBeVisible();
    expect(screen.getByText("42.7 克")).toBeVisible();
    expect(screen.getByText("发现 2 卷同款耗材，请选择实际使用的一卷")).toBeVisible();
    const mappingGroup = screen.getByText("发现 2 卷同款耗材，请选择实际使用的一卷").closest("fieldset");
    expect(within(mappingGroup as HTMLElement).getAllByRole("radio")).toHaveLength(2);

    fireEvent.click(screen.getByLabelText("黑色 PLA #B，488.2 克"));
    fireEvent.click(screen.getByRole("button", { name: "确认耗材映射" }));
    await waitFor(() => expect(onConfirmMapping).toHaveBeenCalledWith("job-mask", [
      { tool: 0, spool_id: "spool-black-b" },
    ]));
  });

  it("converts a user-visible stopped layer to the zero-based backend value", () => {
    const onSettle = vi.fn();
    render(
      <Job
        preview={preview}
        spools={spools}
        initialMappings={{ 0: "spool-black-a" }}
        onConfirmMapping={async () => undefined}
        onSettle={onSettle}
        onConfirmNewPrint={async () => undefined}
        onReverse={async () => undefined}
      />,
    );

    fireEvent.click(screen.getByRole("radio", { name: "打印中途失败" }));
    fireEvent.change(screen.getByLabelText("最后完成的层数"), {
      target: { value: "37" },
    });
    fireEvent.click(screen.getByRole("button", { name: "确认扣减耗材" }));

    expect(onSettle).toHaveBeenCalledWith("job-mask", {
      kind: "failed",
      stop_layer: 36,
    });
  });

  it("marks percentage settlement as estimated and exposes all confirmation paths", () => {
    const onSettle = vi.fn();
    const onConfirmNewPrint = vi.fn();
    const onReverse = vi.fn();
    render(
      <Job
        preview={{ ...preview, state: "new_print_confirmation_required" }}
        spools={spools}
        initialMappings={{ 0: "spool-black-a" }}
        settled
        onConfirmMapping={async () => undefined}
        onSettle={onSettle}
        onConfirmNewPrint={onConfirmNewPrint}
        onReverse={onReverse}
      />,
    );

    fireEvent.click(screen.getByRole("radio", { name: "按打印进度估算" }));
    expect(screen.getByText("估算")).toBeVisible();
    fireEvent.change(screen.getByLabelText("大约完成百分比"), {
      target: { value: "43" },
    });
    fireEvent.click(screen.getByRole("button", { name: "确认扣减耗材" }));
    expect(onSettle).toHaveBeenCalledWith("job-mask", {
      kind: "estimated",
      progress_percent: 43,
    });

    fireEvent.click(screen.getByRole("button", { name: "确认这是一次新打印" }));
    expect(onConfirmNewPrint).toHaveBeenCalledWith("hash-mask");
    fireEvent.click(screen.getByRole("button", { name: "撤销本次扣减" }));
    expect(onReverse).toHaveBeenCalledWith("job-mask");
  });
});
