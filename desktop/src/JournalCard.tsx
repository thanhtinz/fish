import { useState } from "react";
import type { JournalView } from "./types";

interface Props {
  entries: JournalView[];
  busy: string | null;
  onNote: (text: string) => void;
}

/** How each kind of entry reads at a glance. */
const LABELS: Record<string, string> = {
  import: "nhập",
  extract: "trích",
  build: "build",
  rule: "luật",
  patch: "áp vá",
  target: "ngôn ngữ",
  note: "ghi chú",
};

/**
 * What was done to this project, in the order it happened.
 *
 * Việt hoá một game không phải việc một buổi. Ba tuần sau quay lại, thư mục dự án nói rất rõ mọi
 * thứ đang *ở đâu* và không nói gì về chuyện đã đi tới đó thế nào, hay vì sao dừng lại.
 */
export function JournalCard({ entries, busy, onNote }: Props) {
  const [text, setText] = useState("");

  return (
    <div className="card">
      <h3>Nhật ký công việc</h3>
      <div className="sub">
        Các mốc tự ghi lại: nhập game, trích chữ, build số mấy và <b>đạt hay không</b>, bật luật
        nào, áp vá lúc nào. File <code>journal.jsonl</code> ở gốc dự án, chỉ ghi thêm — đọc và
        diff được như mọi thứ khác trong dự án.
      </div>

      <div className="row" style={{ marginBottom: 12 }}>
        <input
          placeholder="đang dở ở đâu? vì sao dừng?"
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && text.trim() !== "") {
              onNote(text.trim());
              setText("");
            }
          }}
        />
        <button
          disabled={busy !== null || text.trim() === ""}
          onClick={() => {
            onNote(text.trim());
            setText("");
          }}
        >
          Ghi lại
        </button>
      </div>
      <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginBottom: 12, lineHeight: 1.6 }}>
        Đây là thứ duy nhất không mốc nào tự suy ra được: <i>vì sao</i> anh dừng. Không file nào
        trong dự án nói được điều đó.
      </div>

      {entries.length === 0 ? (
        <div style={{ color: "var(--text-faint)", fontSize: 12.5 }}>Chưa có gì trong nhật ký.</div>
      ) : (
        <div style={{ display: "grid", gap: 6 }}>
          {entries.map((entry, i) => (
            <div key={`${entry.at}-${i}`} className="row" style={{ gap: 8, alignItems: "baseline" }}>
              <span
                style={{
                  fontFamily: "var(--mono)",
                  fontSize: 11,
                  color: "var(--text-faint)",
                  whiteSpace: "nowrap",
                }}
              >
                {entry.at.replace("T", " ").replace("Z", "")}
              </span>
              <span className={entry.kind === "note" ? "pill" : "pill ok"}>
                {LABELS[entry.kind] ?? entry.kind}
              </span>
              {entry.language && <span className="pill">{entry.language}</span>}
              <span style={{ flex: 1, minWidth: 0, fontSize: 12.5 }}>{entry.detail}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
