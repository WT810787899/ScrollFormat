import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ToastHost } from "./components/Toast";
import { ContextMenuProvider } from "./components/ContextMenuProvider";
import { PromptHost } from "./components/PromptDialog";
import { isTauri } from "./lib/api";
import "./index.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <ContextMenuProvider>
      <App />
      <ToastHost />
      <PromptHost />
    </ContextMenuProvider>
  </React.StrictMode>,
);

// 启动页（index.html 内联样式）已经随 HTML 一起画好：
//   1) index.html 里的内联脚本会在首帧上报 frontend_ready，这里只是兜底再报一次
//   2) React 接管后撤掉启动页
requestAnimationFrame(() => {
  if (isTauri()) {
    import("@tauri-apps/api/core").then(({ invoke }) => invoke("frontend_ready").catch(() => {}));
  }
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      document.getElementById("boot")?.remove();
    }),
  );
});
