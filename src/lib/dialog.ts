import { open } from "@tauri-apps/plugin-dialog";

type OpenDialog = (options: {
  multiple: false;
  directory: false;
  filters: Array<{ name: string; extensions: string[] }>;
}) => Promise<string | string[] | null>;

export async function pickSliced3mf(filterName: string, openDialog: OpenDialog = open): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: filterName, extensions: ["3mf"] }],
  });
  return typeof selected === "string" ? selected : null;
}
