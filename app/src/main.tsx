import { getCurrentWindow } from "@tauri-apps/api/window";
import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import { BubbleWindow } from "./features/bubble/BubbleWindow";
import { LensWindow } from "./features/lens/LensWindow";
import "./i18n";
import "./index.css";

/** 三个窗口共用同一个前端包：按窗口 label 决定渲染主界面、取景框还是悬浮按钮。 */
function currentLabel(): string {
  try {
    return getCurrentWindow().label;
  } catch {
    return "main"; // 浏览器里直接打开时没有 Tauri 环境
  }
}

const label = currentLabel();
if (label === "lens" || label === "bubble") document.documentElement.classList.add(label);

const root = { lens: <LensWindow />, bubble: <BubbleWindow /> }[label] ?? <App />;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{root}</React.StrictMode>,
);
