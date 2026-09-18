import { Menu } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import clsx from "clsx";
import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";

import {
  getCaptureStatus,
  onCaptureStatus,
  quitApp,
  showMainWindow,
  toggleLens,
} from "../../api/capture";

/** 按下后移动超过这么多像素算拖动，否则算点击。 */
const DRAG_THRESHOLD_PX = 4;

/**
 * 主窗口关掉后留在桌面上的小圆图标：点一下开关取景框，拖动换位置，右键打开主窗口或退出。
 * 识别进行中边框变绿，不用打开取景框也能知道后台还在读。
 *
 * 窗口比图标大（Windows 不让无边框窗口低于约 135 px 宽），多出来的部分由壳层用圆形裁掉，
 * 所以这里把图标画在正中间，并且整页背景必须透明。
 */
export function BubbleWindow() {
  const { t } = useTranslation();
  const [capturing, setCapturing] = useState(false);
  const press = useRef<{ x: number; y: number; dragging: boolean } | null>(
    null,
  );

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    getCaptureStatus()
      .then((status) => !disposed && setCapturing(status.running != null))
      .catch((error) => console.error("capture_status failed", error));
    onCaptureStatus((status) => setCapturing(status.running != null))
      .then((fn) => (disposed ? fn() : (unlisten = fn)))
      .catch((error) =>
        console.error("capture status subscription failed", error),
      );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const showMenu = async () => {
    const menu = await Menu.new({
      items: [
        {
          id: "main",
          text: t("bubble.openMain"),
          action: () => void showMainWindow(),
        },
        { id: "quit", text: t("bubble.quit"), action: () => void quitApp() },
      ],
    });
    await menu.popup();
  };

  return (
    // 外层铺满窗口、完全透明：窗口比图标大，多出来的部分被壳层裁掉，这里不能画任何东西。
    <div className="flex h-screen w-screen items-center justify-center bg-transparent">
      <button
        type="button"
        title={
          capturing
            ? `${t("bubble.capturing")} · ${t("bubble.hint")}`
            : t("bubble.hint")
        }
        className={clsx(
          "flex h-11 w-11 cursor-pointer select-none items-center justify-center",
          "rounded-full border-2 bg-zinc-900/95 hover:bg-zinc-800",
          capturing ? "border-emerald-400" : "border-sky-400/80",
        )}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          press.current = { x: e.screenX, y: e.screenY, dragging: false };
        }}
        onPointerMove={(e) => {
          const start = press.current;
          if (!start || start.dragging) return;
          if (
            Math.hypot(e.screenX - start.x, e.screenY - start.y) >
            DRAG_THRESHOLD_PX
          ) {
            start.dragging = true;
            void getCurrentWindow().startDragging();
          }
        }}
        onPointerUp={() => {
          const start = press.current;
          press.current = null;
          if (start && !start.dragging) {
            toggleLens().catch((error) =>
              console.error("toggle_lens failed", error),
            );
          }
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          showMenu().catch((error) => console.error("menu failed", error));
        }}
      >
        <img src="/bubble-icon.png" alt="" className="h-6 w-6" draggable={false} />
      </button>
    </div>
  );
}
