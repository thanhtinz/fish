import type { EmulatorSearch } from "./types";

interface Props {
  search: EmulatorSearch | null;
  busy: string | null;
  hasBuild: boolean;
  onFind: () => void;
  onUse: (path: string) => void;
  onPlay: () => void;
}

/**
 * Running the build in an emulator the machine already has.
 *
 * Chỉ đi tìm. Không tải gì về, không cài gì, và không gợi ý chỗ tải — nên câu trả lời **rỗng**
 * mới là câu phải viết cho tử tế: "không tìm thấy" thì không ai làm gì được, còn danh sách chỗ đã
 * dò thì làm được.
 */
export function EmulatorCard({ search, busy, hasBuild, onFind, onUse, onPlay }: Props) {
  return (
    <div className="card">
      <h3>Chạy thử trong giả lập</h3>
      <div className="sub">
        Kiểm tra tự động nói được bản build hợp lệ, nhưng không nói được chữ có bị cắt ở menu cửa
        hàng không. Cái đó phải nhìn. Công cụ <b>chỉ đi tìm</b> giả lập đã có sẵn trên máy —
        không tải, không cài, không gợi ý chỗ tải.
      </div>

      <div className="row" style={{ flexWrap: "wrap" }}>
        <button disabled={busy !== null} onClick={onFind}>
          {busy === "emulator" ? <span className="spin" /> : null} Tìm giả lập trên máy
        </button>
        {search?.configured && (
          <button
            className="primary"
            disabled={busy !== null || !hasBuild}
            title={hasBuild ? "" : "Build trước đã"}
            onClick={onPlay}
          >
            Chạy bản build
          </button>
        )}
      </div>

      {search?.configured && (
        <dl className="facts" style={{ marginTop: 12 }}>
          <dt>Đang dùng</dt>
          <dd>{search.configured}</dd>
        </dl>
      )}

      {search && search.found.length > 0 && (
        <div style={{ marginTop: 12 }}>
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 5 }}>
            Tìm thấy trên máy này:
          </div>
          {search.found.map((one) => (
            <div key={one.path} className="row" style={{ gap: 8, marginTop: 6 }}>
              <span style={{ flex: 1, minWidth: 0 }}>
                <b>{one.name}</b>
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, fontFamily: "var(--mono)" }}>
                  {one.path}
                </div>
                <div style={{ color: "var(--text-faint)", fontSize: 11.5 }}>{one.evidence}</div>
              </span>
              <button disabled={busy !== null} onClick={() => onUse(one.path)}>
                Dùng cái này
              </button>
            </div>
          ))}
        </div>
      )}

      {search && search.found.length === 0 && (
        <div style={{ marginTop: 12 }}>
          <div style={{ color: "var(--warn)", fontSize: 12.5, marginBottom: 8 }}>
            Không tìm thấy giả lập J2ME nào trên máy này.
            {!search.javaAvailable && (
              <>
                {" "}Máy cũng <b>không có java</b>, mà phần lớn giả lập J2ME là file <code>.jar</code>
                {" "}cần java để chạy.
              </>
            )}
          </div>
          {/* Danh sách chỗ đã dò, không phải lời xin lỗi: đó là thứ người ta hành động được. */}
          <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 4 }}>Đã dò ở:</div>
          <div
            className="src-text"
            style={{ whiteSpace: "pre-wrap", maxHeight: 160, overflow: "auto", fontSize: 11.5 }}
          >
            {search.searched.join("\n")}
          </div>
          <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 8, lineHeight: 1.6 }}>
            Cài một cái (FreeJ2ME, MicroEmulator, KEmulator…) vào một trong những chỗ trên rồi bấm
            tìm lại. Công cụ không tải giúp: nó sẽ là phần mềm của người khác, tải về máy anh, kèm
            giấy phép và chuyện xác minh bản tải mà dự án này không nhận.
          </div>
        </div>
      )}
    </div>
  );
}
