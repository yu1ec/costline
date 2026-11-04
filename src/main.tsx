import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles.css";

// The WebView's native context menu ships with a Reload entry; suppress it everywhere.
document.addEventListener(
  "contextmenu",
  (event) => event.preventDefault(),
  { capture: true },
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
