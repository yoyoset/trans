import { useEffect } from "react";
import { useTranslation } from "react-i18next";

import { Button } from "../../shared/ui/Button";
import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { Section } from "../../shared/ui/Section";
import { useCaptureStore } from "./useCaptureStore";

/** 主窗口里的入口：说明用法并打开取景框。 */
export function CapturePanel() {
  const { t } = useTranslation();
  const capture = useCaptureStore();
  const load = capture.load;

  useEffect(() => {
    void load();
  }, [load]);

  if (!capture.supported) {
    return <p className="text-zinc-400">{t("capture.unsupported")}</p>;
  }

  return (
    <Section title={t("capture.title")}>
      <p className="text-sm text-zinc-300">{t("capture.howTo")}</p>
      <p className="text-xs text-amber-300">{t("capture.clickWarning")}</p>
      <div className="flex flex-wrap gap-2">
        <Button variant="primary" onClick={() => void capture.showLens()} disabled={capture.busy}>
          {t("capture.openLens")}
        </Button>
        <Button onClick={() => void capture.hideLens()} disabled={capture.busy}>
          {t("capture.closeLens")}
        </Button>
      </div>
      {capture.error && <ErrorNotice error={capture.error} />}
    </Section>
  );
}
