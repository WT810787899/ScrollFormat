import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { ToastHost } from "./components/Toast";
import { ContextMenuProvider } from "./components/ContextMenuProvider";
import { PromptHost } from "./components/PromptDialog";
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
