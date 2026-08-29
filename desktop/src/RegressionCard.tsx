import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { RegressionView } from "./types";

interface Props {
  path: string;
  language: string;
  say: (text: string, bad?: boolean) => void;
}

/**
 * What changed in the picture since the last drawing somebody accepted.
 *
 * Not an emulator: nothing here runs the game, and nothing here knows what a menu looks like. What
 * it does know is what the text will look like, in the game's own letters at the game's own size -
 * and comparing that against the drawing accepted last time catches the failure a text report
 * cannot show. Six lines were edited and sixty changed: a font was recomposed, a glyph order
 * edited, a rule installed a sheet whose letters sit a pixel lower.
 */
export function RegressionCard({ path, language, say }: Props) {
  const [state, setState] = useState<RegressionView | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      setState(await api.visualRegression(path, language, null));
    } catch {
      setState(null);
    }
  }, [path, language]);

  useEffect(() => {
    void load();
  }, [load]);

  if (!state) return null;

  return (
    <div className="card">
      <h3>So với bản vẽ đã duyệt</h3>
      <div className="sub">
        Công cụ vẽ mọi bản dịch đã duyệt bằng đúng font và đúng cỡ chữ của game. So bản vẽ lần này
        với bản đã duyệt lần trước sẽ thấy thứ mà báo cáo chữ không thấy được: sửa sáu dòng mà ảnh
        đổi sáu chục chỗ nghĩa là có thứ khác đã dịch chuyển.
      </div>

      {!state.compared ? (
        <div style={{ color: "var(--text-faint)", fontSize: 12, marginBottom: 10 }}>
          Chưa có bản vẽ nào được duyệt để so. Xem bản vẽ hiện tại, thấy đúng thì bấm duyệt.
        </div>
      ) : state.identical ? (
        <div className="row" style={{ gap: 8, marginBottom: 10 }}>
          <span className="pill ok">không đổi</span>
          <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
            giống hệt bản đã duyệt
          </span>
        </div>
      ) : (
        <div style={{ marginBottom: 10 }}>
          <div className="row" style={{ gap: 8 }}>
            <span className="pill warn">
              {state.changed} điểm ảnh đổi ({(state.share * 100).toFixed(2)}%)
            </span>
            {state.resized && <span className="pill warn">ảnh đổi kích thước</span>}
          </div>
          {state.bands.length > 0 && (
            <div style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 5 }}>
              đổi ở {state.bands.length} chỗ · hàng {state.bands.slice(0, 8).join(", ")}
              {state.bands.length > 8 ? "…" : ""}
            </div>
          )}
        </div>
      )}

      {state.picture && (
        <img
          src={state.picture}
          alt="bản vẽ, chỗ đổi được đánh dấu"
          style={{
            maxWidth: "100%",
            imageRendering: "pixelated",
            background: "#12151a",
            border: "1px solid var(--line)",
            borderRadius: 4,
            marginBottom: 10,
          }}
        />
      )}

      <div className="row">
        <button
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              const done = await api.acceptBaseline(path, language, null);
              say(
                done
                  ? "Đã duyệt bản vẽ hiện tại làm mốc so sánh"
                  : "Chưa có gì để vẽ: cần khai báo font và có ít nhất một bản dịch đã duyệt",
                !done,
              );
              await load();
            } catch (e) {
              say(String(e), true);
            } finally {
              setBusy(false);
            }
          }}
        >
          Duyệt bản vẽ hiện tại làm mốc
        </button>
        <button disabled={busy} onClick={() => void load()}>
          So lại
        </button>
      </div>
    </div>
  );
}
