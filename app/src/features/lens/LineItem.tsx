import { memo } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { IssueList } from "../composer/IssueList";
import { useLensStore, type Line } from "./useLensStore";

/**
 * 识别到的一句对方消息：原文 → 点「翻译」出中文 → 不满意再点「在线校对」。
 * memo：store 只替换被修改的那一句，其余句子引用不变，翻译一句时不必重渲染整个列表。
 */
export const LineItem = memo(function LineItem({ line }: { line: Line }) {
  const { t } = useTranslation();
  const translateLine = useLensStore((s) => s.translateLine);
  const calibrateLine = useLensStore((s) => s.calibrateLine);
  const shown = line.calibrated ?? line.translation;

  return (
    <article className="space-y-1.5 rounded-md border border-zinc-800 bg-zinc-900 p-2.5">
      <p className="text-sm leading-snug text-zinc-200">
        {line.sender && <span className="mr-1 text-xs text-sky-300">{line.sender}:</span>}
        {line.text}
      </p>

      {shown && <p className="text-sm leading-snug text-emerald-200">{shown.translation}</p>}
      {line.calibrated?.notes.map((note, i) => (
        <p key={i} className="text-xs text-sky-300">
          {note}
        </p>
      ))}
      {shown && <IssueList issues={shown.issues} canCalibrate={!line.calibrated} />}
      {line.error && <ErrorNotice error={line.error} />}

      <div className="flex gap-3 text-xs">
        {!line.translation && (
          <button
            type="button"
            className="text-emerald-400 hover:underline disabled:opacity-50"
            disabled={line.busy !== null}
            onClick={() => void translateLine(line.id)}
          >
            {line.busy === "translating" ? t("lens.translating") : t("lens.translate")}
          </button>
        )}
        {line.translation && !line.calibrated && (
          <button
            type="button"
            className="text-sky-400 hover:underline disabled:opacity-50"
            disabled={line.busy !== null}
            onClick={() => void calibrateLine(line.id)}
          >
            {line.busy === "calibrating" ? t("lens.calibrating") : t("lens.calibrate")}
          </button>
        )}
      </div>
    </article>
  );
});
