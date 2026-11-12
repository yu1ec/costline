import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { TrayPanel } from "./pages/TrayPanel";
import "./styles.css";

// The WebView's native context menu ships with a Reload entry; suppress it everywhere.
document.addEventListener(
  "contextmenu",
  (event) => event.preventDefault(),
  { capture: true },
);

// 托盘下拉面板和主面板共用同一份前端产物，用 query 参数区分视图。
const isTrayPanel =
  new URLSearchParams(window.location.search).get("view") === "tray";
if (isTrayPanel) {
  document.documentElement.dataset.window = "tray";
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {isTrayPanel ? <TrayPanel /> : <App />}
  </React.StrictMode>,
);
