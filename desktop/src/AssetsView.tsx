import { useCallback, useEffect, useState } from "react";
import { api, pickFile } from "./api";
import type { Hint, ImageAssetView, ReadingView } from "./types";

interface Props {
  path: string;
  say: (text: string, bad?: boolean) => void;
}

/**
 * Images with words painted into them.
 *
 * The quietest failure in the whole project: a build where every string is translated, every
 * check passes, and the player still sees an English START button - because that word was never
 * a string, it was part of a picture.
 *
 * It shows them, says what about each one resembles a label, and records what a person decides -
 * after which the build reports every marked image that still ships its original artwork.
 *
 * Where the project knows the game's glyph sheet, the words can be read straight off the picture
 * by matching it against those same letters. A reading where every shape matched fills the box; a
 * reading with an unmatched shape in it is shown as unread rather than offered, and either way
 * nothing is saved until somebody presses the button.
 */
/** The Vietnamese wording for one of the core's reasons. */
function describe(hint: Hint): string {
  switch (hint.kind) {
    case "nameSuggests":
      return `tên file có chữ "${hint.word}"`;
    case "fewColours":
      return `${hint.colours} màu, chiếm ${hint.inkPercent}% ảnh — giống chữ hơn là giống tranh`;
    case "shapeOfALine":
      return `${hint.width}×${hint.height}, ${hint.bands} dải mực — đúng dáng một dòng chữ`;
  }
}

export function AssetsView({ path, say }: Props) {
  const [assets, setAssets] = useState<ImageAssetView[] | null>(null);
  const [onlySuspect, setOnlySuspect] = useState(true);
  const [busy, setBusy] = useState<string | null>(null);
  const [says, setSays] = useState<Record<string, string>>({});
  const [readings, setReadings] = useState<Record<string, ReadingView>>({});

  const load = useCallback(async () => {
    try {
      setAssets(await api.imageAssets(path));
    } catch (e) {
      say(String(e), true);
      setAssets([]);
    }
  }, [path, say]);

  useEffect(() => {
    void load();
  }, [load]);

  async function run<T>(tag: string, work: () => Promise<T>): Promise<T | null> {
    setBusy(tag);
    try {
      return await work();
    } catch (e) {
      say(String(e), true);
      return null;
    } finally {
      setBusy(null);
    }
  }

  if (!assets) {
    return (
      <div className="pad">
        <div className="card">
          <h3>Ảnh</h3>
          <div className="sub">Đang tải…</div>
        </div>
      </div>
    );
  }

  const marked = assets.filter((a) => a.marked);
  const shown = assets.filter((a) => !onlySuspect || a.hints.length > 0 || a.marked);

  return (
    <div className="pad">
      <div className="card">
        <h3>Ảnh có chữ vẽ sẵn</h3>
        <div className="sub">
          Chữ trong game không phải lúc nào cũng là chuỗi. Nút bấm, logo, banner thường là ảnh đã
          vẽ sẵn chữ lên — dịch chuỗi bao nhiêu cũng không đụng tới chúng. Một bản build có thể
          dịch xong hết, qua mọi kiểm tra, mà người chơi vẫn thấy nút "START" tiếng Anh.
        </div>
        <p style={{ color: "var(--text-faint)", fontSize: 12, margin: "0 0 12px" }}>
          Chữ trên nút bấm thường được vẽ bằng chính font của game, nên công cụ có thể{" "}
          <b>đối chiếu từng hình với từng chữ cái trong font đó</b>. Hình nào khớp thì đọc ra, hình
          nào không khớp thì báo là không đọc được — chứ không đoán bừa. Bạn nhìn lại rồi đánh dấu;
          ảnh đã đánh dấu mà chưa có bản thay thế sẽ được báo lại ở mỗi lần build.
        </p>
        <div className="row" style={{ gap: 10 }}>
          <label className="row" style={{ gap: 6, fontSize: 12.5, whiteSpace: "nowrap" }}>
            <input
              type="checkbox"
              checked={onlySuspect}
              onChange={(e) => setOnlySuspect(e.target.checked)}
            />
            Chỉ ảnh nghi có chữ
          </label>
          <button
            disabled={busy !== null}
            onClick={async () => {
              const read = await run("read", () => api.readTextAssets(path, []));
              if (!read) return;
              const found: Record<string, ReadingView> = {};
              const filled: Record<string, string> = {};
              for (const reading of read) {
                found[reading.entry] = reading;
                if (reading.complete) filled[reading.entry] = reading.text;
              }
              setReadings(found);
              setSays({ ...filled, ...says });
              const complete = read.filter((r) => r.complete).length;
              say(`Đọc được ${complete}/${read.length} ảnh bằng font của game`);
            }}
          >
            {busy === "read" ? "Đang đọc…" : "Đọc chữ bằng font game"}
          </button>
          <span style={{ color: "var(--text-faint)", fontSize: 12, whiteSpace: "nowrap" }}>
            {shown.length}/{assets.length} ảnh · {marked.length} đã đánh dấu
          </span>
        </div>
      </div>

      {shown.length === 0 && (
        <div className="card">
          <div className="sub" style={{ margin: 0 }}>
            {assets.length === 0
              ? "Game này không có ảnh PNG nào."
              : "Không ảnh nào có dáng của một dòng chữ. Bỏ dấu tick ở trên để xem tất cả."}
          </div>
        </div>
      )}

      {shown.map((asset) => (
        <div className="card" key={asset.entry}>
          <div className="row" style={{ alignItems: "flex-start", gap: 14 }}>
            <img
              src={asset.image}
              alt={asset.entry}
              style={{
                maxWidth: 220,
                maxHeight: 120,
                imageRendering: "pixelated",
                background: "#1b1f27",
                border: "1px solid var(--line)",
                borderRadius: 4,
              }}
            />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div className="row">
                <span style={{ fontFamily: "var(--mono)", fontSize: 12.5, flex: 1 }}>
                  {asset.entry}
                </span>
                {asset.marked && (
                  <span className={asset.replacement ? "pill ok" : "pill warn"}>
                    {asset.replacement ? "đã có bản thay" : "chưa có bản thay"}
                  </span>
                )}
              </div>
              <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 3 }}>
                {asset.width}×{asset.height}px · {asset.colours} màu
              </div>
              {asset.hints.map((hint) => (
                <div
                  key={hint.kind}
                  style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}
                >
                  · {describe(hint)}
                </div>
              ))}

              {readings[asset.entry] && (
                <div style={{ fontSize: 11.5, marginTop: 5 }}>
                  {readings[asset.entry].complete ? (
                    <span style={{ color: "var(--text-dim)" }}>
                      đọc được: <b>{readings[asset.entry].text}</b> (khớp thấp nhất{" "}
                      {readings[asset.entry].confidence.toFixed(2)})
                    </span>
                  ) : (
                    <span style={{ color: "var(--text-faint)" }}>
                      {readings[asset.entry].unread} hình không khớp chữ nào trong font — phải tự
                      đọc
                    </span>
                  )}
                </div>
              )}

              <div className="row" style={{ gap: 8, marginTop: 10, flexWrap: "wrap" }}>
                <input
                  placeholder="Ảnh này viết gì?"
                  value={says[asset.entry] ?? asset.says ?? ""}
                  onChange={(e) => setSays({ ...says, [asset.entry]: e.target.value })}
                  style={{ flex: 1, minWidth: 160 }}
                />
                <button
                  disabled={busy !== null}
                  onClick={async () => {
                    const updated = await run("mark", () =>
                      api.markTextAsset(
                        path,
                        asset.entry,
                        says[asset.entry] ?? asset.says ?? null,
                        asset.replacement,
                      ),
                    );
                    if (updated) {
                      setAssets(updated);
                      say(`Đã đánh dấu ${asset.entry}`);
                    }
                  }}
                >
                  {asset.marked ? "Lưu" : "Đánh dấu có chữ"}
                </button>
                {asset.marked && (
                  <>
                    <button
                      disabled={busy !== null}
                      onClick={async () => {
                        const chosen = await pickFile("Chọn ảnh đã vẽ lại", "PNG", ["png"]);
                        if (!chosen) return;
                        const updated = await run("replace", () =>
                          api.markTextAsset(
                            path,
                            asset.entry,
                            says[asset.entry] ?? asset.says ?? null,
                            chosen,
                          ),
                        );
                        if (updated) setAssets(updated);
                      }}
                    >
                      Chọn ảnh đã vẽ lại…
                    </button>
                    <button
                      disabled={busy !== null}
                      onClick={async () => {
                        const updated = await run("unmark", () =>
                          api.unmarkTextAsset(path, asset.entry),
                        );
                        if (updated) setAssets(updated);
                      }}
                    >
                      Bỏ đánh dấu
                    </button>
                  </>
                )}
              </div>
              {asset.replacement && (
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 7 }}>
                  Thay bằng {asset.replacement} — vẫn cần một luật ở tab Font để gắn nó vào bản
                  build.
                </div>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
  );
}
