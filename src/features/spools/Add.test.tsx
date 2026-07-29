import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, expect, test, vi } from "vitest";
import { setLocale } from "../../i18n";
import type { Spool, SpoolStatus } from "../../lib/tauri";
import { Add } from "./Add";

beforeEach(async () => setLocale("zh-CN"));

function jadeWhiteSpool(
  spoolId: string,
  status: SpoolStatus,
): Spool {
  return {
    spool_id: spoolId,
    display_name: `existing ${spoolId}`,
    preset_id: "Bambu PLA Basic",
    preset_base: "Bambu PLA Basic",
    catalog_id: "bambu:GFA00:10100",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: "玉石白",
    color_code: "10100",
    color_hex: "#FFFFFF",
    color_hexes: ["#FFFFFF"],
    remaining_grams: 500,
    status,
  };
}

async function selectJadeWhite(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
}

test("selects material, series, and an official Chinese color", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue(true);
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={onCreate}
    />,
  );

  await selectJadeWhite(user);
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      catalog_id: "bambu:GFA00:10100",
      color_name: "玉石白",
      color_code: "10100",
      color_hex: "#FFFFFF",
      color_hexes: ["#FFFFFF"],
      preset_id: "Bambu PLA Basic @base",
      preset_base: "Bambu PLA Basic @base",
    }),
  );
});

test("has no operating-system color input", () => {
  const { container } = render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  expect(container.querySelector('input[type="color"]')).toBeNull();
});

test("changing material clears downstream choices", async () => {
  const user = userEvent.setup();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  await selectJadeWhite(user);
  await user.type(screen.getByLabelText("搜索颜色"), "10100");
  await user.click(screen.getByRole("button", { name: "PETG" }));

  const seriesChoices = within(
    screen.getByRole("region", { name: "选择系列" }),
  ).getAllByRole("button");
  for (const choice of seriesChoices) {
    expect(choice).toHaveAttribute("aria-pressed", "false");
  }
  expect(screen.queryByText("已选颜色")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("搜索颜色")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

test("changing series clears the selected color and search query", async () => {
  const user = userEvent.setup();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  await selectJadeWhite(user);
  await user.type(screen.getByLabelText("搜索颜色"), "10100");
  await user.click(screen.getByRole("button", { name: "Matte" }));

  expect(screen.getByRole("button", { name: "Matte" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.queryByText("已选颜色")).not.toBeInTheDocument();
  expect(screen.getByLabelText("搜索颜色")).toHaveValue("");
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

test("searching colors removes non-matching official colors", async () => {
  const user = userEvent.setup();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  expect(
    screen.getByRole("button", { name: /玉石白.*10100/ }),
  ).toBeVisible();
  expect(screen.getByRole("button", { name: /黑色.*10101/ })).toBeVisible();

  await user.type(screen.getByLabelText("搜索颜色"), "10100");

  expect(
    screen.getByRole("button", { name: /玉石白.*10100/ }),
  ).toBeVisible();
  expect(
    screen.queryByRole("button", { name: /黑色.*10101/ }),
  ).not.toBeInTheDocument();
});

test("closes from the keyboard without saving", async () => {
  const user = userEvent.setup();
  const onClose = vi.fn();
  const onCreate = vi.fn();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={onClose}
      onCreate={onCreate}
    />,
  );

  await user.keyboard("{Escape}");

  expect(onClose).toHaveBeenCalledOnce();
  expect(onCreate).not.toHaveBeenCalled();
});

test("exposes a labelled modal and focuses its close button when opened", async () => {
  const { rerender } = render(
    <Add
      open={false}
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  rerender(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  const dialog = screen.getByRole("dialog", { name: "添加一卷耗材" });
  expect(dialog).toHaveAttribute("aria-modal", "true");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "关闭" })).toHaveFocus(),
  );
});

test("uses exact catalog fields and numbers duplicate active spools", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue(true);
  render(
    <Add
      open
      spools={[
        jadeWhiteSpool("one", "available"),
        jadeWhiteSpool("two", "empty"),
        jadeWhiteSpool("archived", "archived"),
      ]}
      busy={false}
      onClose={() => undefined}
      onCreate={onCreate}
    />,
  );

  await selectJadeWhite(user);
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onCreate).toHaveBeenCalledWith({
    display_name: "玉石白 · PLA Basic #3",
    preset_id: "Bambu PLA Basic @base",
    preset_base: "Bambu PLA Basic @base",
    catalog_id: "bambu:GFA00:10100",
    brand: "Bambu Lab",
    material: "PLA",
    series: "Basic",
    color_name: "玉石白",
    color_code: "10100",
    color_hex: "#FFFFFF",
    color_hexes: ["#FFFFFF"],
    remaining_grams: 1000,
  });
});

test("uses a trimmed custom name and a positive custom weight", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn().mockResolvedValue(true);
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={onCreate}
    />,
  );

  await selectJadeWhite(user);
  await user.type(screen.getByLabelText("自定义名称"), "  工作室白色  ");
  await user.clear(screen.getByLabelText("当前剩余量"));
  await user.type(screen.getByLabelText("当前剩余量"), "750.5");
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      display_name: "工作室白色",
      remaining_grams: 750.5,
    }),
  );
});

test.each(["", "0", "-1"])(
  "disables saving for an invalid weight of %j",
  async (weight) => {
    const user = userEvent.setup();
    render(
      <Add
        open
        spools={[]}
        busy={false}
        onClose={() => undefined}
        onCreate={vi.fn()}
      />,
    );

    await selectJadeWhite(user);
    await user.clear(screen.getByLabelText("当前剩余量"));
    if (weight) {
      await user.type(screen.getByLabelText("当前剩余量"), weight);
    }

    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  },
);

test("keeps every field open when creation fails", async () => {
  const user = userEvent.setup();
  const onClose = vi.fn();
  const onCreate = vi.fn().mockResolvedValue(false);
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={onClose}
      onCreate={onCreate}
    />,
  );

  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.type(screen.getByLabelText("搜索颜色"), "10100");
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
  await user.type(screen.getByLabelText("自定义名称"), "失败后保留");
  await user.clear(screen.getByLabelText("当前剩余量"));
  await user.type(screen.getByLabelText("当前剩余量"), "900");
  await user.click(screen.getByRole("button", { name: "保存" }));

  expect(onClose).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(screen.getByLabelText("搜索颜色")).toHaveValue("10100");
  expect(screen.getByLabelText("自定义名称")).toHaveValue("失败后保留");
  expect(screen.getByLabelText("当前剩余量")).toHaveValue(900);
  expect(screen.getByRole("button", { name: "保存" })).toBeEnabled();
});

test("successful creation clears transient fields but retains material and series", async () => {
  const user = userEvent.setup();
  const onClose = vi.fn();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={onClose}
      onCreate={vi.fn().mockResolvedValue(true)}
    />,
  );

  await user.click(screen.getByRole("button", { name: "PLA" }));
  await user.click(screen.getByRole("button", { name: "Basic" }));
  await user.type(screen.getByLabelText("搜索颜色"), "10100");
  await user.click(screen.getByRole("button", { name: /玉石白.*10100/ }));
  await user.type(screen.getByLabelText("自定义名称"), "临时名字");
  await user.clear(screen.getByLabelText("当前剩余量"));
  await user.type(screen.getByLabelText("当前剩余量"), "900");
  await user.click(screen.getByRole("button", { name: "保存" }));

  await waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(screen.getByRole("button", { name: "PLA" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.getByRole("button", { name: "Basic" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.getByLabelText("搜索颜色")).toHaveValue("");
  expect(screen.getByLabelText("自定义名称")).toHaveValue("");
  expect(screen.getByLabelText("当前剩余量")).toHaveValue(1000);
  expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
});

test("renders a Swatch in color tiles and in the selected summary", async () => {
  const user = userEvent.setup();
  render(
    <Add
      open
      spools={[]}
      busy={false}
      onClose={() => undefined}
      onCreate={vi.fn()}
    />,
  );

  await selectJadeWhite(user);

  expect(
    within(screen.getByRole("button", { name: /玉石白.*10100/ })).getByTestId(
      "swatch",
    ),
  ).toBeVisible();
  expect(
    within(screen.getByText("已选颜色").parentElement as HTMLElement).getByTestId(
      "swatch",
    ),
  ).toBeVisible();
});
