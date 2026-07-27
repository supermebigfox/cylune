import { open, save } from "@tauri-apps/plugin-dialog";

type OpenDialog = (options: {
  multiple: false;
  directory: false;
  filters: Array<{ name: string; extensions: string[] }>;
}) => Promise<string | string[] | null>;
type BackupOpenDialog = (options: {
  multiple: false;
  directory: false;
  filters: Array<{ name: string; extensions: string[] }>;
}) => Promise<string | string[] | null>;
type SaveDialog = (options: {
  defaultPath: string;
  filters: Array<{ name: string; extensions: string[] }>;
}) => Promise<string | null>;

export async function pickSliced3mf(filterName: string, openDialog: OpenDialog = open): Promise<string | null> {
  const selected = await openDialog({
    multiple: false,
    directory: false,
    filters: [{ name: filterName, extensions: ["3mf"] }],
  });
  return typeof selected === "string" ? selected : null;
}

export async function pickWatchFolder(): Promise<string | null> {
  const selected=await open({multiple:false,directory:true});
  return typeof selected === "string" ? selected : null;
}

export async function pickBackupToImport(filterName: string, openDialog: BackupOpenDialog = open): Promise<string | null> {
  const selected=await openDialog({multiple:false,directory:false,filters:[{name:filterName,extensions:["json"]}]});
  return typeof selected === "string" ? selected : null;
}

export async function pickBackupDestination(filterName: string, defaultPath: string, saveDialog: SaveDialog = save): Promise<string | null> {
  return await saveDialog({defaultPath,filters:[{name:filterName,extensions:["json"]}]});
}
