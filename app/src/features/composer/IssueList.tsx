import { useTranslation } from "react-i18next";

import type { ValidationIssue } from "../../api/types";

export function IssueList({ issues, canCalibrate }: { issues: ValidationIssue[]; canCalibrate: boolean }) {
  const { t } = useTranslation();
  if (issues.length === 0) return null;
  return (
    <div className="rounded-md border border-amber-900/70 bg-amber-950/40 px-3 py-2 text-xs text-amber-200">
      <p className="font-medium">{t("issues.title")}</p>
      <ul className="mt-1 list-disc space-y-0.5 pl-4">
        {issues.map((issue, i) => (
          <li key={i}>
            {issue.kind === "missing_numbers"
              ? t("issues.missing_numbers", { numbers: issue.numbers.join("、") })
              : t(`issues.${issue.kind}`)}
          </li>
        ))}
      </ul>
      {canCalibrate && <p className="mt-1 text-amber-300/80">{t("issues.suggestCalibrate")}</p>}
    </div>
  );
}
