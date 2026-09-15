import { useTranslation } from "react-i18next";

import type { AppError } from "../../api/client";

/** 错误分两层：摘要直接显示，技术细节折叠。 */
export function ErrorNotice({ error }: { error: AppError }) {
  const { t } = useTranslation();
  return (
    <div className="rounded-md border border-rose-900 bg-rose-950/50 px-3 py-2 text-rose-200">
      <p>{error.message}</p>
      {error.detail && (
        <details className="mt-1 text-xs text-rose-300/80">
          <summary className="cursor-pointer select-none">{t("composer.showDetail")}</summary>
          <pre className="mt-1 whitespace-pre-wrap break-all font-mono">{error.detail}</pre>
        </details>
      )}
    </div>
  );
}
