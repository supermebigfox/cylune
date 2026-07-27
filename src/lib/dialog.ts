import { open } from "@tauri-apps/plugin-dialog";

type OpenDialog = (options: {
  multiple: false;
  directory: false;
  filters: Array<{ name: string; extensions: string[] }>;
}) => Promise<string | string[] | null>;

export async function pickSliced3mf(openDialog: OpenDialog = open): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: "Sliced 3MF", extensions: ["3mf"] }],
  });
  return typeof selected === "string" ? selected : null;
}
