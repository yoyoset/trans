import { useState } from "react";
import { useTranslation } from "react-i18next";

const STORAGE_PREFIX = "trans.hint.";

/** 读"这条提示是否已经读过"。存储不可用时当作没读过：多显示一次，总比让用户不知道行为变了好。 */
function alreadySeen(id: string): boolean {
  try {
    return localStorage.getItem(STORAGE_PREFIX + id) === "seen";
  } catch {
    return false;
  }
}

/**
 * 只提示一次的说明条，用于"行为和用户预期不同"的地方（例如关窗口不退出而是收成悬浮按钮）。
 * 点「知道了」后不再出现。
 */
export function OneTimeHint({ id, children }: { id: string; children: React.ReactNode }) {
  const { t } = useTranslation();
  const [dismissed, setDismissed] = useState(() => alreadySeen(id));
  if (dismissed) return null;

  const dismiss = () => {
    setDismissed(true);
    try {
      localStorage.setItem(STORAGE_PREFIX + id, "seen");
    } catch {
      // 记不住就下次再提示一遍，不影响使用。
    }
  };

  return (
    <div className="flex items-start gap-2 rounded-md bg-sky-950/60 px-3 py-1.5 text-xs text-sky-200">
      <p className="flex-1">{children}</p>
      <button type="button" className="shrink-0 text-sky-300 hover:underline" onClick={dismiss}>
        {t("app.gotIt")}
      </button>
    </div>
  );
}
