import { useState } from "react";
import type { EnginePreview, EngineView } from "./types";

interface Props {
  engine: EngineView | null;
  busy: string | null;
  onSave: (kind: string, endpoint: string, model: string | null, enabled: boolean) => void;
  onSaveKey: (key: string) => void;
  onPreview: (text: string) => Promise<EnginePreview | null>;
}

/**
 * The external engine's settings.
 *
 * Written to be read before it is used. Turning this on sends the game's text to somebody else's
 * computer, which is a decision, and the interface's job is to make it one rather than a checkbox
 * nobody read.
 */
export function EngineCard({ engine, busy, onSave, onSaveKey, onPreview }: Props) {
  const [kind, setKind] = useState<string | null>(null);
  const [endpoint, setEndpoint] = useState<string | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [key, setKey] = useState("");
  const [preview, setPreview] = useState<EnginePreview | null>(null);

  if (!engine) {
    return (
      <div className="card">
        <h3>Máy dịch ngoài</h3>
        <div className="sub">Đang tải…</div>
      </div>
    );
  }

  const currentKind = kind ?? engine.kind;
  const currentEndpoint = endpoint ?? engine.endpoint;
  const currentModel = model ?? engine.model ?? "";
  const chosen = engine.kinds.find((k) => k.id === currentKind);

  function pickKind(id: string) {
    setKind(id);
    // Switching family without switching endpoint would point the new request shape at the old
    // service, which fails in a way that reads like an outage.
    const preset = engine!.kinds.find((k) => k.id === id);
    if (preset) setEndpoint(preset.defaultEndpoint);
  }

  return (
    <div className="card">
      <h3>
        Máy dịch ngoài{" "}
        {engine.enabled ? (
          <span className="pill bad">đang bật — chữ trong game sẽ được gửi đi</span>
        ) : (
          <span className="pill ok">đang tắt — không có gì rời khỏi máy</span>
        )}
      </h3>
      <div className="sub">
        Từ điển tra thuật ngữ; máy dịch viết được cả câu. Bật lên là chữ trong game của anh được
        gửi tới dịch vụ bên dưới, tính phí theo lượng chữ. Không có dịch vụ nào cài sẵn — anh tự
        chọn và tự nhập khoá.
      </div>

      <div className="row" style={{ marginBottom: 10 }}>
        <label style={{ width: 96, color: "var(--text-faint)", fontSize: 12 }}>Loại</label>
        <select value={currentKind} onChange={(e) => pickKind(e.target.value)}>
          {engine.kinds.map((k) => (
            <option key={k.id} value={k.id}>
              {k.id}
            </option>
          ))}
        </select>
      </div>

      {chosen && !chosen.takesInstructions && (
        <div className="banner note" style={{ marginBottom: 10 }}>
          <span>
            Loại này không nhận được chỉ dẫn bằng lời, nên thuật ngữ và xưng hô chỉ được{" "}
            <b>kiểm tra lúc trả về</b> chứ không gửi đi kèm. Kết quả sẽ cần sửa nhiều hơn.
          </span>
        </div>
      )}

      <div className="row" style={{ marginBottom: 10 }}>
        <label style={{ width: 96, color: "var(--text-faint)", fontSize: 12 }}>Endpoint</label>
        <input value={currentEndpoint} onChange={(e) => setEndpoint(e.target.value)} />
      </div>

      {chosen?.takesInstructions && (
        <div className="row" style={{ marginBottom: 10 }}>
          <label style={{ width: 96, color: "var(--text-faint)", fontSize: 12 }}>Model</label>
          <input value={currentModel} onChange={(e) => setModel(e.target.value)} />
        </div>
      )}

      <div className="row" style={{ marginBottom: 10 }}>
        <label style={{ width: 96, color: "var(--text-faint)", fontSize: 12 }}>Khoá API</label>
        <input
          type="password"
          placeholder={engine.hasKey ? "đã lưu — nhập để thay" : "chưa lưu"}
          value={key}
          onChange={(e) => setKey(e.target.value)}
        />
        <button
          className="small"
          disabled={busy !== null || key.trim() === ""}
          onClick={() => {
            onSaveKey(key);
            setKey("");
          }}
        >
          Lưu khoá
        </button>
      </div>
      <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginBottom: 12, lineHeight: 1.6 }}>
        Khoá lưu ngoài thư mục dự án, ở file chỉ chủ máy đọc được. Dự án là thư mục người ta commit
        và gửi cho người dịch — khoá nằm trong đó là lộ ngay lần đầu ai đó làm vậy.
      </div>

      <div className="row" style={{ flexWrap: "wrap" }}>
        <button
          disabled={busy !== null}
          onClick={() => onSave(currentKind, currentEndpoint, currentModel || null, engine.enabled)}
        >
          Lưu thiết lập
        </button>
        <button
          className={engine.enabled ? "" : "primary"}
          disabled={busy !== null || !engine.hasKey}
          title={engine.hasKey ? "" : "Cần lưu khoá API trước"}
          onClick={() =>
            onSave(currentKind, currentEndpoint, currentModel || null, !engine.enabled)
          }
        >
          {engine.enabled ? "Tắt" : "Bật máy dịch"}
        </button>
        <button
          disabled={busy !== null}
          onClick={async () => setPreview(await onPreview("Guild: %d members"))}
        >
          Xem thử request
        </button>
      </div>

      {preview && (
        <div className="block" style={{ marginTop: 14 }}>
          <h4>Đúng cái sẽ được gửi đi</h4>
          <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginBottom: 8 }}>
            POST {preview.url}
          </div>
          <div className="src-text" style={{ whiteSpace: "pre-wrap", maxHeight: 240, overflow: "auto" }}>
            {preview.instructions}
          </div>
          <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 8, lineHeight: 1.6 }}>
            Thuật ngữ đã chốt và luật xưng hô của dự án được gửi kèm, rồi kiểm lại lúc trả về — đó
            là chỗ biến bản dịch máy đúng ngữ pháp thành bản dịch hợp game.
          </div>
        </div>
      )}
    </div>
  );
}
