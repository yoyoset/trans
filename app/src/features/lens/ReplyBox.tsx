import { memo, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";

import { ErrorNotice } from "../../shared/ui/ErrorNotice";
import { IssueList } from "../composer/IssueList";
import { useLensStore, type LensRect } from "./useLensStore";

/** 停止输入多久后自动翻译。 */
const DEBOUNCE_MS = 600;

/** 我的回复：打中文，停一下自动出英文；可复制，也可在线校对。 */
export const ReplyBox = memo(function ReplyBox({
  captureRect,
}: {
  captureRect: () => Promise<LensRect | null>;
}) {
  const { t } = useTranslation();
  const reply = useLensStore((s) => s.reply);
  const translateReply = useLensStore((s) => s.translateReply);
  const calibrateReply = useLensStore((s) => s.calibrateReply);
  const sendReply = useLensStore((s) => s.sendReply);
  const [sent, setSent] = useState<"copied" | "pasted" | null>(null);
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

  /** 送回聊天窗口：粘贴到取景区中心所在的窗口，游戏场景后端会自动降级为只复制。 */
  const send = async (pressEnter: boolean) => {
    const rect = await captureRect();
    const center: [number, number] | null = rect
      ? [
          rect.x + Math.round(rect.width / 2),
          rect.y + Math.round(rect.height / 2),
        ]
      : null;
    const pasted = await sendReply(center, pressEnter);
    setSent(pasted ? "pasted" : "copied");
    setTimeout(() => setSent(null), 1500);
  };

  return (
    <footer className="space-y-2 border-t border-zinc-800 p-3">
      <textarea
        className="input h-16 resize-none text-sm"
        value={text}
        placeholder={t("lens.replyPlaceholder")}
        onChange={(e) => setText(e.target.value)}
      />
      {reply.busy === "translating" && (
        <p className="text-xs text-zinc-400">{t("lens.translating")}</p>
      )}
      {shown && (
        <p className="text-sm leading-snug text-emerald-200">
          {shown.translation}
        </p>
      )}
      {reply.calibrated?.notes.map((note, i) => (
        <p key={i} className="text-xs text-sky-300">
          {note}
        </p>
      ))}
      {shown && (
        <IssueList issues={shown.issues} canCalibrate={!reply.calibrated} />
      )}
      {reply.error && <ErrorNotice error={reply.error} />}
      {shown && (
        <div className="flex gap-3 text-xs">
          <button
            type="button"
            className="text-emerald-400 hover:underline"
            onClick={() => void copy()}
          >
            {copied ? t("lens.copied") : t("lens.copy")}
          </button>
          <button
            type="button"
            className="text-emerald-400 hover:underline"
            title={t("lens.pasteHint")}
            onClick={() => void send(false)}
          >
            {sent === "pasted"
              ? t("lens.pasted")
              : sent === "copied"
                ? t("lens.copiedOnly")
                : t("lens.paste")}
          </button>
          <button
            type="button"
            className="text-emerald-400 hover:underline"
            title={t("lens.pasteEnterHint")}
            onClick={() => void send(true)}
          >
            {t("lens.pasteEnter")}
          </button>
          {!reply.calibrated && (
            <button
              type="button"
              className="text-sky-400 hover:underline disabled:opacity-50"
              disabled={reply.busy !== null}
              onClick={() => void calibrateReply()}
            >
              {reply.busy === "calibrating"
                ? t("lens.calibrating")
                : t("lens.calibrate")}
            </button>
          )}
        </div>
      )}
    </footer>
  );
});
