import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { beforeEach, expect, it, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { Spool } from "../../lib/tauri";
import { Spools } from "./Spools";

const identicalBlackSpools: Spool[] = [
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

const mixedSpools: Spool[] = [
  ...identicalBlackSpools,
  {
    spool_id: "spool-red",
    display_name: "红色 PLA",
    preset_id: "Bambu PLA Matte @BBL A1",
    preset_base: null,
    catalog_id: null,
    brand: "Bambu Lab",
    material: "PLA",
    series: "Matte",
    color_name: null,
    color_code: null,
    color_hex: "#E54A42",
    color_hexes: ["#E54A42"],
    remaining_grams: 401.2,
    status: "available",
  },
];

const officialWhite: Spool = {
  spool_id: "spool-jade-white",
  display_name: "玉石白 · PLA Basic",
  preset_id: "Bambu PLA Basic @base",
  preset_base: "Bambu PLA Basic @base",
  catalog_id: "bambu:GFA00:10100",
  brand: "Bambu Lab",
  material: "PLA",
  series: "Basic",
  color_name: "持久化白色",
  color_code: "10100",
  color_hex: "#FFFFFF",
  color_hexes: ["#FFFFFF"],
  remaining_grams: 500,
  status: "available",
};

const dualColorSpool: Spool = {
  ...officialWhite,
  spool_id: "spool-dual",
  display_name: "工作室双色卷",
  catalog_id: null,
  color_name: "海盐双色",
  color_code: null,
  color_hex: "#63D8E2",
  color_hexes: ["#63D8E2", "#FF8BA0"],
};

const codeOnlySpool: Spool = {
  ...officialWhite,
  spool_id: "spool-code-only",
  display_name: "仅色号旧卷",
  catalog_id: null,
  color_name: null,
  color_code: "STUDIO-42",
};

beforeEach(async () => setLocale("zh-CN"));

function renderSpools(
  overrides: Partial<ComponentProps<typeof Spools>> = {},
) {
  const props: ComponentProps<typeof Spools> = {
    spools: [],
    slotBySpool: {},
    onCreate: vi.fn().mockResolvedValue({ ok: true }),
    onCalibrate: vi.fn(),
    onArchive: vi.fn(),
    onMount: vi.fn(),
    onUnmount: vi.fn(),
    onMove: vi.fn(),
    ...overrides,
  };
  return { ...render(<Spools {...props} />), props };
}

async function openAndChooseJadeWhite(
  user: ReturnType<typeof userEvent.setup>,
) {
  await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));
  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
}

it("opens the catalog dialog instead of the inline color form", async () => {
  const user = userEvent.setup();
  const { container } = renderSpools();

  await user.click(screen.getByRole("button", { name: "添加一卷耗材" }));

  const dialog = screen.getByRole("dialog", { name: "添加一卷耗材" });
  expect(dialog).toBeVisible();
  expect(dialog.closest(".modal-backdrop")?.parentElement).toBe(document.body);
  expect(container.querySelector(".inline-form")).toBeNull();
  expect(container.querySelector('input[type="color"]')).toBeNull();
});

it.each(["close button", "Escape"])(
  "restores focus to the add button after closing with %s",
  async (method) => {
    const user = userEvent.setup();
    renderSpools();
    const opener = screen.getByRole("button", { name: "添加一卷耗材" });

    await user.click(opener);
    expect(screen.getByRole("button", { name: "关闭" })).toHaveFocus();

    if (method === "Escape") {
      await user.keyboard("{Escape}");
    } else {
      await user.click(screen.getByRole("button", { name: "关闭" }));
    }

    await waitFor(() =>
      expect(screen.queryByRole("dialog")).not.toBeInTheDocument(),
    );
    expect(opener).toHaveFocus();
  },
);

it("creates identical official colors as separate rolls", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue({ ok: true });
  renderSpools({ spools: [officialWhite], onCreate });

  await openAndChooseJadeWhite(user);
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      display_name: "玉石白 · PLA Basic #2",
      catalog_id: "bambu:GFA00:10100",
    }),
  );
});

it("resolves official names for the current locale and keeps metadata searchable", async () => {
  const user = userEvent.setup();
  renderSpools({ spools: [officialWhite, dualColorSpool, codeOnlySpool] });

  expect(screen.getByText("玉石白")).toBeVisible();
  expect(screen.getByText("10100")).toBeVisible();
  expect(screen.getByText("海盐双色")).toBeVisible();
  expect(screen.getByText("STUDIO-42")).toBeVisible();

  await act(async () => setLocale("en"));
  expect(await screen.findByText("Jade White")).toBeVisible();
  expect(screen.queryByText("持久化白色")).not.toBeInTheDocument();

  await user.type(screen.getByLabelText("Search spools"), "持久化白色");
  expect(screen.getByText("玉石白 · PLA Basic")).toBeVisible();
  expect(screen.queryByText("工作室双色卷")).not.toBeInTheDocument();

  await user.clear(screen.getByLabelText("Search spools"));
  await user.type(screen.getByLabelText("Search spools"), "STUDIO-42");
  expect(screen.getByText("仅色号旧卷")).toBeVisible();
  expect(screen.queryByText("工作室双色卷")).not.toBeInTheDocument();
  expect(screen.queryByText("玉石白 · PLA Basic")).not.toBeInTheDocument();
});

it("renders every spool color through the shared Swatch component", () => {
  renderSpools({ spools: [dualColorSpool] });

  const swatch = screen.getByTestId("swatch");
  expect(swatch).toHaveStyle({
    background:
      "linear-gradient(135deg, #63D8E2 0%, #FF8BA0 100%)",
  });
});

it("keeps same-color spools as independently actionable entities", async () => {
  const onCalibrate = vi.fn();
  render(
    <Spools
      spools={identicalBlackSpools}
      slotBySpool={{ "spool-black-a": 1 }}
      onCreate={async () => ({ ok: true })}
      onCalibrate={onCalibrate}
      onArchive={async () => undefined}
      onMount={async () => undefined}
      onUnmount={async () => undefined}
      onMove={async () => undefined}
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
      onCreate={async () => ({ ok: true })}
      onCalibrate={async () => undefined}
      onArchive={async () => undefined}
      onMount={onMount}
      onUnmount={async () => undefined}
      onMove={async () => undefined}
    />,
  );

  fireEvent.change(screen.getByLabelText("颜色筛选"), { target: { value: "#252733" } });
  expect(screen.getByText("黑色 PLA #A")).toBeVisible();
  expect(screen.getByText("黑色 PLA #B")).toBeVisible();
  expect(screen.queryByText("红色 PLA")).not.toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "装入 AMS" }));
  fireEvent.change(screen.getByLabelText("AMS 槽位"), { target: { value: "4" } });
  fireEvent.click(screen.getByRole("button", { name: "确认装入" }));

  await waitFor(() => expect(onMount).toHaveBeenCalledWith("spool-black-b", 4));
});

it("moves a mounted spool to a chosen slot and can unmount it", async () => {
  const onMove = vi.fn(async () => undefined);
  const onUnmount = vi.fn(async () => undefined);
  render(
    <Spools
      spools={identicalBlackSpools}
      slotBySpool={{ "spool-black-a": 1 }}
      onCreate={async () => ({ ok: true })}
      onCalibrate={async () => undefined}
      onArchive={async () => undefined}
      onMount={async () => undefined}
      onUnmount={onUnmount}
      onMove={onMove}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "移动到其他槽位" }));
  fireEvent.change(screen.getByLabelText("AMS 槽位"), { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "确认移动" }));
  await waitFor(() => expect(onMove).toHaveBeenCalledWith("spool-black-a", 3));

  fireEvent.click(screen.getByRole("button", { name: "从 AMS 拆下" }));
  expect(onUnmount).toHaveBeenCalledWith(1);
});
