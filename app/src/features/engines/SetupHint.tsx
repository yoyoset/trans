import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { useEngineStatus } from "./useEngineStatus";

/**
 * 首次使用时本地引擎是空的，翻译会一直失败而界面只写着"未配置"。
 * 这里把"要做什么"和入口一起给出来。
 */
export function SetupHint({ onOpenSettings }: { onOpenSettings: () => void }) {
  const { t } = useTranslation();
  const local = useEngineStatus((s) => s.status?.local);
  if (local?.state !== "not_configured") return null;

  return (
    <div className="flex items-center gap-3 rounded-md bg-amber-950/60 px-3 py-1.5 text-xs text-amber-200">
      <p className="flex-1">{t("engine.setupNeeded")}</p>
      <Button variant="primary" onClick={onOpenSettings}>
        {t("engine.goSettings")}
      </Button>
    </div>
  );
}
