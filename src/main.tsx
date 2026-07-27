import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import { TrayDrop } from "./features/tray/TrayDrop";
import { api } from "./lib/tauri";
import { Theme } from "./theme/Theme";
import "./styles.css";

const isMenubar=new URLSearchParams(window.location.search).get("window")==="menubar";
if(isMenubar)document.documentElement.dataset.window="menubar";
const root=isMenubar?<Theme><TrayDrop onImport={(path)=>api.importPrintFile(path)} onOpenMain={()=>api.openMain?.()} onOpenJob={(jobId)=>api.openJobInMain?.(jobId)}/></Theme>:<App/>;
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    {root}
  </React.StrictMode>,
);
