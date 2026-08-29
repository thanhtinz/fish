import { useState } from "react";
import type { AnalystView, InspectionView, ScanPreview, SuggestionView } from "./types";

interface Props {
  analyst: AnalystView | null;
  suggestions: SuggestionView[];
  busy: string | null;
  onSave: (model: string, enabled: boolean) => void;
  onSaveKey: (key: string) => void;
  onPreview: () => Promise<ScanPreview | null>;
  onScan: () => void;
  onInspect: (entry: string) => Promise<InspectionView | null>;
}

/**
 * The analysis side: asking a model which files hold text, and what an unknown file is.
 *
 * Written to be read before it is used, like the engine card beside it. The two sentences that
 * matter are the ones about what leaves the machine, and they are on the page rather than in the
 * documentation: a scan sends names, a look at one file sends the start of that one file.
 */
export function AnalystCard(props: Props) {
  const { analyst, busy } = props;
  const [model, setModel] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const [preview, setPreview] = useState<ScanPreview | null>(null);
  const [entry, setEntry] = useState("");
  const [inspection, setInspection] = useState<InspectionView | null>(null);

  if (!analyst) {
    return (
      <div className="card">
        <h3>Claude phân tích</h3>
        <div className="sub">Đang tải…</div>
      </div>
    );
  }

  const currentModel = model ?? analyst.model;

  return (
    <div className="card">
      <h3>
        Claude phân tích{" "}
        {analyst.enabled ? (
          <span className="pill warn">đang bật — tên file sẽ được gửi khi anh bấm quét</span>
        ) : (
          <span className="pill ok">đang tắt — không có gì rời khỏi máy</span>
        )}
      </h3>
      <div className="sub">
        Việc máy làm được là đoán, không phải kết luận. Quét thì <b>chỉ gửi tên file, kích thước
        và định dạng máy đã nhận ra</b> — không gửi nội dung file nào. Chỉ khi anh bấm "hỏi về file
        này" mới có 2 KiB đầu của đúng file đó được gửi đi. Kết quả nằm ở mục riêng bên dưới và
        không thay đổi bất cứ điều gì phần lõi đã xác định.
      </div>

      <label>Model</label>
      <select value={currentModel} onChange={(e) => setModel(e.target.value)}>
        {analyst.models.map((m) => (
          <option key={m} value={m}>
            {m}
          </option>
        ))}
      </select>
      <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 5, lineHeight: 1.6 }}>
        claude-opus-5 đọc kỹ hơn; claude-haiku-4-5 rẻ hơn nhiều cho thư mục game lớn.
      </div>

      <label style={{ marginTop: 12 }}>Khoá API</label>
      <div className="row">
        <input
          type="password"
          placeholder={analyst.hasKey ? "đã lưu — nhập để thay" : "sk-ant-…"}
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
        <button disabled={busy !== null || key.trim() === ""} onClick={() => { props.onSaveKey(key); setKey(""); }}>
          Lưu khoá
        </button>
      </div>
      <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 5, lineHeight: 1.6 }}>
        Lưu ngoài thư mục dự án, chỉ chủ máy đọc được, và ghi theo địa chỉ dịch vụ — nên khoá này
        dùng chung với máy dịch <code>anthropic</code>, nhập một lần là đủ.
      </div>

      <div className="row" style={{ marginTop: 14 }}>
        <button
          disabled={busy !== null}
          onClick={() => props.onSave(currentModel, !analyst.enabled)}
        >
          {analyst.enabled ? "Tắt" : "Bật"}
        </button>
        <button
          disabled={busy !== null}
          onClick={async () => setPreview(await props.onPreview())}
        >
          Xem trước sẽ gửi gì
        </button>
        <button disabled={busy !== null || !analyst.enabled} onClick={props.onScan}>
          Quét
        </button>
      </div>

      {preview && (
        <div style={{ marginTop: 12 }}>
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 5 }}>
            {preview.paths.length} tên file sẽ được gửi tới {preview.model}
            {preview.tokens !== null
              ? ` · ${preview.tokens.toLocaleString("vi-VN")} token, do dịch vụ đếm`
              : ""}
          </div>
          {preview.trouble && (
            <div style={{ color: "var(--warn)", fontSize: 12, marginBottom: 5 }}>
              Chưa đếm được token: {preview.trouble}
            </div>
          )}
          <div
            className="src-text"
            style={{ whiteSpace: "pre-wrap", maxHeight: 240, overflow: "auto" }}
          >
            {preview.paths.join("\n")}
          </div>
        </div>
      )}

      <hr style={{ border: 0, borderTop: "1px solid var(--line)", margin: "16px 0" }} />

      <label>Hỏi về một file</label>
      <div className="row">
        <input
          placeholder="assets/data.bin"
          value={entry}
          onChange={(e) => setEntry(e.target.value)}
        />
        <button
          disabled={busy !== null || !analyst.enabled || entry.trim() === ""}
          onClick={async () => setInspection(await props.onInspect(entry.trim()))}
        >
          Gửi 2 KiB đầu
        </button>
      </div>
      {inspection && (
        <dl className="facts" style={{ marginTop: 10 }}>
          <dt>Định dạng</dt>
          <dd>{inspection.format}</dd>
          <dt>Chữ nằm ở</dt>
          <dd style={{ fontFamily: "inherit" }}>{inspection.whereTextIs}</dd>
          <dt>Địa chỉ hoá</dt>
          <dd style={{ fontFamily: "inherit" }}>{inspection.addressing}</dd>
          <dt>Có thể sai vì</dt>
          {/* Cuối cùng và không bao giờ bỏ: đây là phỏng đoán về một định dạng nhị phân. */}
          <dd style={{ fontFamily: "inherit", color: "var(--warn)" }}>{inspection.caveat}</dd>
        </dl>
      )}

      {props.suggestions.length > 0 && (
        <>
          <hr style={{ border: 0, borderTop: "1px solid var(--line)", margin: "16px 0" }} />
          {/* Tách hẳn khỏi mục "Đọc được" ở thẻ Loại gói: một phỏng đoán nằm cạnh một sự thật thì
              một tuần sau không ai phân biệt được nữa. */}
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 5 }}>
            Claude gợi ý — <b>phỏng đoán, không phải kết luận</b>, và không thay đổi gì phía trên:
          </div>
          <dl className="facts">
            {props.suggestions.map((s) => (
              <div key={s.path} style={{ display: "contents" }}>
                <dt>{s.path}</dt>
                <dd style={{ fontFamily: "inherit", color: "var(--text-faint)" }}>
                  {Math.round(s.confidence * 100)}% · {s.why}
                </dd>
              </div>
            ))}
          </dl>
          <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 7 }}>
            Do {props.suggestions[0].model} nói.
          </div>
        </>
      )}
    </div>
  );
}
