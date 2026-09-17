import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { IssueList } from "../composer/IssueList";
import { useLensStore } from "./useLensStore";

/** 停止输入多久后自动翻译。 */
const DEBOUNCE_MS = 600;

/** 我的回复：打中文，停一下自动出英文；可复制，也可在线校对。 */
export function ReplyBox() {
  const { t } = useTranslation();
  const reply = useLensStore((s) => s.reply);
  const translateReply = useLensStore((s) => s.translateReply);
  const calibrateReply = useLensStore((s) => s.calibrateReply);
  const [text, setText] = useState("");
  const [copied, setCopied] = useState(false);
  const shown = reply.calibrated ?? reply.translation;

  useEffect(() => {
    const timer = setTimeout(() => void translateReply(text), DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [text, translateReply]);

  const copy = async () => {
    if (!shown) return;
    await navigator.clipboard.writeText(shown.translation);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <footer className="space-y-2 border-t border-zinc-800 p-3">
      <textarea
        className="input h-16 resize-none text-sm"
        value={text}
        placeholder={t("lens.replyPlaceholder")}
        onChange={(e) => setText(e.target.value)}
      />
      {reply.busy === "translating" && <p className="text-xs text-zinc-400">{t("lens.translating")}</p>}
      {shown && <p className="text-sm leading-snug text-emerald-200">{shown.translation}</p>}
      {reply.calibrated?.notes.map((note, i) => (
        <p key={i} className="text-xs text-sky-300">
          {note}
        </p>
      ))}
      {shown && <IssueList issues={shown.issues} canCalibrate={!reply.calibrated} />}
      {reply.error && <ErrorNotice error={reply.error} />}
      {shown && (
        <div className="flex gap-3 text-xs">
          <button type="button" className="text-emerald-400 hover:underline" onClick={() => void copy()}>
            {copied ? t("lens.copied") : t("lens.copy")}
          </button>
          {!reply.calibrated && (
            <button
              type="button"
              className="text-sky-400 hover:underline disabled:opacity-50"
              disabled={reply.busy !== null}
              onClick={() => void calibrateReply()}
            >
              {reply.busy === "calibrating" ? t("lens.calibrating") : t("lens.calibrate")}
            </button>
          )}
        </div>
      )}
    </footer>
  );
}
