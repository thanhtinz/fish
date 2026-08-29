import { useCallback, useEffect, useState } from "react";
import { api, pickFolder } from "./api";
import type {
  CompositionView,
  FontLookupView,
  FontScan,
  FontView as Font,
  RuleView,
  SheetCandidateView,
} from "./types";

interface Props {
  path: string;
  say: (text: string, bad?: boolean) => void;
}

/**
 * The game's font.
 *
 * A J2ME game usually draws text from a strip of pixels holding the letters it was written for,
 * and a game from China, Japan or Korea holds ASCII and nothing else. Vietnamese needs 134 more.
 * So a finished, correct translation still renders as blank boxes unless somebody deals with the
 * font - which is what this tab is for.
 *
 * Everything here is offered rather than decided. Which image is the font, which grid it uses and
 * which typeface the marks come from are all judgements about one game, and getting any of them
 * wrong produces artwork that looks almost right, which is worse than an obvious failure.
 */
export function FontView({ path, say }: Props) {
  const [font, setFont] = useState<Font | null>(null);
  const [candidates, setCandidates] = useState<SheetCandidateView[] | null>(null);
  const [scan, setScan] = useState<FontScan | null>(null);
  const [composed, setComposed] = useState<CompositionView | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [sample, setSample] = useState("Cá đã cắn câu\nBạn nhận được 5 vàng\nThoát trò chơi");
  const [order, setOrder] = useState("");
  const [rules, setRules] = useState<RuleView[]>([]);
  const [lookups, setLookups] = useState<FontLookupView[]>([]);
  const [busy, setBusy] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setFont(await api.fontStatus(path));
      setRules(await api.rules(path));
    } catch (e) {
      say(String(e), true);
    }
  }, [path, say]);

  useEffect(() => {
    setCandidates(null);
    setScan(null);
    setComposed(null);
    setPreview(null);
    void load();
  }, [load]);

  // Where the game looks like it writes down its sheet's shape. Loaded on its own because it is
  // evidence about the game rather than about anything the person just did.
  useEffect(() => {
    let alive = true;
    api
      .fontLookupCandidates(path)
      .then((found) => {
        if (alive) setLookups(found);
      })
      .catch(() => {
        if (alive) setLookups([]);
      });
  }, [path]);

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

  if (!font) {
    return (
      <div className="pad">
        <div className="card">
          <h3>Font</h3>
          <div className="sub">Đang tải…</div>
        </div>
      </div>
    );
  }

  const share = font.required > 0 ? font.covered / font.required : 0;
  const complete = font.covered >= font.required;

  return (
    <div className="pad">
      <div className="card">
        <h3>Font của game</h3>
        <div className="sub">
          Game J2ME thường vẽ chữ từ một ảnh chứa sẵn các chữ cái, và ảnh đó chỉ có những chữ mà
          game gốc cần. Tiếng Việt cần thêm 134 chữ có dấu — thiếu chúng thì bản dịch đúng vẫn hiện
          ra ô trống.
        </div>

        {!font.declared ? (
          <p style={{ color: "var(--warn)", fontSize: 12.5, margin: "0 0 12px" }}>
            Chưa khai báo font. Chưa khai báo <b>không</b> có nghĩa là font đủ dùng — nghĩa là chưa
            ai kiểm tra.
          </p>
        ) : font.problem ? (
          <p style={{ color: "var(--bad)", fontSize: 12.5, margin: "0 0 12px" }}>{font.problem}</p>
        ) : (
          <>
            <div className="row" style={{ marginBottom: 6 }}>
              <span className={complete ? "pill ok" : "pill warn"}>
                {font.covered}/{font.required} chữ Việt
              </span>
              <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
                {font.deviceFont
                  ? "Game dùng font của máy"
                  : `${font.entry} · ${font.grid?.columns}×${font.grid?.rows} ô ${font.grid?.cellWidth}×${font.grid?.cellHeight}px`}
              </span>
            </div>
            <div className="bar" style={{ marginBottom: 12 }}>
              <i style={{ width: `${Math.round(share * 100)}%` }} />
            </div>
            {!complete && (
              <dl className="facts" style={{ marginBottom: 12 }}>
                <dt>Thiếu</dt>
                <dd style={{ fontSize: 13 }}>{font.missing}</dd>
                <dt>Ghép được</dt>
                <dd>
                  {font.composable} chữ có thể dựng từ chính các chữ sẵn có trong ảnh font của game
                </dd>
              </dl>
            )}
          </>
        )}

        <div className="wrap">
          <button
            disabled={busy !== null}
            onClick={async () => {
              const found = await run("candidates", () => api.fontCandidates(path));
              if (found) {
                setCandidates(found);
                if (found.length === 0) say("Không tìm thấy ảnh PNG nào trong gói game");
              }
            }}
          >
            {busy === "candidates" ? <span className="spin" /> : null} Tìm ảnh font trong game
          </button>
          <button
            disabled={busy !== null}
            onClick={async () => {
              const f = await run("device", () => api.setDeviceFont(path));
              if (f) {
                setFont(f);
                say("Đã ghi nhận: game dùng font của máy");
              }
            }}
          >
            Game dùng font của máy
          </button>
          {font.declared && (
            <button
              disabled={busy !== null}
              onClick={async () => {
                const f = await run("clear", () => api.clearFont(path));
                if (f) setFont(f);
              }}
            >
              Xoá khai báo
            </button>
          )}
        </div>
      </div>

      {candidates && candidates.length > 0 && (
        <div className="card">
          <h3>Ảnh nào là font?</h3>
          <div className="sub">
            Xếp theo mức giống một bảng chữ: ít màu, ít mực, và các chữ nằm gọn trong lưới. Máy chỉ
            gợi ý — chọn sai lưới thì mọi chữ lệch một pixel và trông như lỗi hiển thị, nên người
            nhìn ảnh mới là người quyết.
          </div>
          <label className="row" style={{ marginBottom: 12, gap: 8 }}>
            <span style={{ color: "var(--text-faint)", fontSize: 12 }}>Thứ tự chữ</span>
            <input
              value={order}
              placeholder="để trống = ASCII in được (mặc định của hầu hết game)"
              onChange={(e) => setOrder(e.target.value)}
              style={{ flex: 1 }}
            />
          </label>

          <div style={{ display: "grid", gap: 14 }}>
            {candidates.map((c) => (
              <div key={c.entry} style={{ borderTop: "1px solid var(--line-soft)", paddingTop: 12 }}>
                <div className="row" style={{ alignItems: "flex-start", gap: 14 }}>
                  <img
                    src={c.image}
                    alt={c.entry}
                    style={{
                      maxWidth: 260,
                      maxHeight: 130,
                      imageRendering: "pixelated",
                      background: "#1b1f27",
                      border: "1px solid var(--line)",
                      borderRadius: 4,
                    }}
                  />
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ fontFamily: "var(--mono)", fontSize: 12.5 }}>{c.entry}</div>
                    <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 3 }}>
                      {c.width}×{c.height}px · {c.colours} màu · {Math.round(c.inkShare * 100)}% có
                      mực
                    </div>
                    <div className="wrap" style={{ marginTop: 9 }}>
                      {c.grids.length === 0 && (
                        <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
                          Không có lưới nào chia đều được ảnh này
                        </span>
                      )}
                      {c.grids.slice(0, 6).map((g) => (
                        <button
                          key={`${g.columns}x${g.rows}`}
                          disabled={busy !== null}
                          title={`${g.capacity} ô · khớp lưới ${Math.round(g.fit * 100)}%`}
                          onClick={async () => {
                            const f = await run("sheet", () =>
                              api.setFontSheet(path, c.entry, g, order.trim() || null),
                            );
                            if (f) {
                              setFont(f);
                              setCandidates(null);
                              setComposed(null);
                              setPreview(null);
                              say(`Đã chọn ${c.entry}`);
                            }
                          }}
                        >
                          {g.columns}×{g.rows} · ô {g.cellWidth}×{g.cellHeight} ·{" "}
                          {Math.round(g.fit * 100)}%
                        </button>
                      ))}
                    </div>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {font.declared && !font.deviceFont && !font.problem && (
        <>
          <div className="card">
            <h3>Kiểu dấu</h3>
            <div className="sub">
              Dấu thanh có thể tự vẽ từ pixel, hoặc mượn hình dấu từ một bộ font trên máy bạn. Font
              không bị chép vào dự án — chỉ nhớ đường dẫn, nên vẫn gửi dự án cho người khác được.
            </div>

            <div className="wrap" style={{ marginBottom: 12 }}>
              <button
                disabled={busy !== null}
                onClick={async () => {
                  const dir =
                    font.markLibrary ?? (await pickFolder("Chọn thư mục chứa font (.ttf/.otf)"));
                  if (!dir) return;
                  const s = await run("scan", () => api.scanFontLibrary(path, dir, 40));
                  if (s) {
                    setScan(s);
                    await load();
                    if (s.fonts.length === 0) say("Không font nào trong thư mục dùng được", true);
                  }
                }}
              >
                {busy === "scan" ? <span className="spin" /> : null}{" "}
                {font.markLibrary ? "Đo lại thư mục font" : "Chọn thư mục font…"}
              </button>
              {font.markLibrary && (
                <button
                  disabled={busy !== null}
                  onClick={async () => {
                    const dir = await pickFolder("Chọn thư mục chứa font (.ttf/.otf)");
                    if (!dir) return;
                    const s = await run("scan", () => api.scanFontLibrary(path, dir, 40));
                    if (s) {
                      setScan(s);
                      await load();
                    }
                  }}
                >
                  Đổi thư mục…
                </button>
              )}
              {font.marksFrom && (
                <button
                  disabled={busy !== null}
                  onClick={async () => {
                    const f = await run("marks", () => api.setMarksFont(path, null));
                    if (f) {
                      setFont(f);
                      setPreview(null);
                      say("Quay lại dấu tự vẽ");
                    }
                  }}
                >
                  Dùng dấu tự vẽ
                </button>
              )}
            </div>

            <dl className="facts" style={{ marginBottom: scan ? 12 : 0 }}>
              <dt>Thư mục</dt>
              <dd>{font.markLibrary ?? "chưa chọn — đang dùng dấu tự vẽ"}</dd>
              <dt>Đang dùng</dt>
              <dd>{font.marksFrom ?? "dấu tự vẽ"}</dd>
            </dl>

            {scan && (
              <>
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginBottom: 8 }}>
                  {scan.found} font trong thư mục · {scan.covering} font đủ 134 chữ Việt ·{" "}
                  {scan.measured} font đã đo thật trên ảnh font của game. Số phần trăm là số dấu
                  font đó thực sự cấp được ở đúng cỡ chữ này — không phải font đẹp hơn là cấp được
                  nhiều hơn.
                </div>
                <div style={{ display: "grid", gap: 2, maxHeight: 260, overflow: "auto" }}>
                  {scan.fonts.map((f) => (
                    <div
                      key={f.path}
                      className="row"
                      style={{
                        padding: "5px 8px",
                        borderRadius: 5,
                        background: f.chosen ? "var(--accent-soft)" : "transparent",
                      }}
                    >
                      <span style={{ flex: 1, minWidth: 0, fontSize: 12.5 }}>{f.name}</span>
                      <span
                        style={{
                          color: "var(--text-faint)",
                          fontFamily: "var(--mono)",
                          fontSize: 11.5,
                        }}
                      >
                        {f.fromTypeface}/{f.composed} · {Math.round(f.share * 100)}%
                      </span>
                      <button
                        disabled={busy !== null || f.chosen}
                        onClick={async () => {
                          const updated = await run("marks", () => api.setMarksFont(path, f.path));
                          if (updated) {
                            setFont(updated);
                            setScan({
                              ...scan,
                              fonts: scan.fonts.map((x) => ({ ...x, chosen: x.path === f.path })),
                            });
                            setPreview(null);
                            say(`Dấu lấy từ ${f.name}`);
                          }
                        }}
                      >
                        {f.chosen ? "Đang dùng" : "Chọn"}
                      </button>
                    </div>
                  ))}
                </div>
              </>
            )}
          </div>

          <div className="card">
            <h3>Xem thử</h3>
            <div className="sub">
              Xem ở đúng cỡ chữ mà game hiển thị. Dấu nào dễ đọc hơn thì con số không trả lời được
              — nên nó được đặt trước mắt người.
            </div>
            <textarea
              value={sample}
              rows={3}
              onChange={(e) => setSample(e.target.value)}
              style={{ width: "100%", marginBottom: 10 }}
            />
            <div className="wrap" style={{ marginBottom: preview ? 12 : 0 }}>
              <button
                disabled={busy !== null}
                onClick={async () => {
                  const image = await run("preview", () => api.fontPreview(path, sample, 4));
                  if (image) setPreview(image);
                }}
              >
                {busy === "preview" ? <span className="spin" /> : null} Xem thử
              </button>
            </div>
            {preview && (
              <img
                src={preview}
                alt="Xem thử font"
                style={{
                  maxWidth: "100%",
                  imageRendering: "pixelated",
                  background: "#1b1f27",
                  border: "1px solid var(--line)",
                  borderRadius: 4,
                }}
              />
            )}
          </div>

          <div className="card">
            <h3>Tạo ảnh font mới</h3>
            <div className="sub">
              Ghi ảnh font đã bổ sung chữ Việt vào thư mục fonts/ của dự án. Việc này{" "}
              <b>chưa gắn font vào game</b>: bắt game dùng bảng chữ mới đòi hỏi sửa cách game tra
              cứu chữ, mà cách đó khác nhau ở từng game.
            </div>
            <div className="wrap">
              <button
                className="primary"
                disabled={busy !== null}
                onClick={async () => {
                  const result = await run("compose", () => api.composeFont(path));
                  if (result) {
                    setComposed(result);
                    say(`Đã thêm ${[...result.added].length} chữ`);
                  }
                }}
              >
                {busy === "compose" ? <span className="spin" /> : null} Tạo ảnh font
              </button>
            </div>
            {composed && (
              <>
                <dl className="facts" style={{ margin: "14px 0" }}>
                  <dt>File</dt>
                  <dd>{composed.path}</dd>
                  <dt>Đã thêm</dt>
                  <dd style={{ fontSize: 13 }}>{composed.added}</dd>
                  <dt>Nguồn dấu</dt>
                  <dd>
                    {composed.typeface
                      ? `${composed.typeface} (${composed.fromTypeface} dấu), còn lại tự vẽ`
                      : "tự vẽ toàn bộ"}
                  </dd>
                  {composed.skipped.length > 0 && (
                    <>
                      <dt>Bỏ qua</dt>
                      <dd>
                        {composed.skipped.map((g) => (
                          <div key={g.reason} style={{ marginBottom: 6 }}>
                            <span style={{ fontSize: 13 }}>{g.letters}</span>
                            <span style={{ color: "var(--text-faint)", fontFamily: "inherit" }}>
                              {" "}
                              — {g.reason}
                            </span>
                          </div>
                        ))}
                        <div style={{ color: "var(--text-faint)", fontFamily: "inherit" }}>
                          Bỏ còn hơn vẽ ra một chữ mà người chơi đọc nhầm sang chữ khác.
                        </div>
                      </dd>
                    </>
                  )}
                </dl>
                <img
                  src={composed.image}
                  alt="Ảnh font đã bổ sung"
                  style={{
                    maxWidth: "100%",
                    imageRendering: "pixelated",
                    background: "#1b1f27",
                    border: "1px solid var(--line)",
                    borderRadius: 4,
                  }}
                />
              </>
            )}
          </div>

          <div className="card">
            <h3>Gắn vào game</h3>
            <div className="sub">
              Đây là phần <b>riêng của từng game</b>: đổi ảnh font thì game nào cũng như nhau,
              nhưng dạy game biết bảng chữ đã cao thêm và ô mới chứa chữ gì thì mỗi game một kiểu.
              Công cụ viết sẵn phần giống nhau, và <i>đi tìm</i> phần còn lại — nên luật viết ra
              luôn ở trạng thái <b>tắt</b> cho tới khi bạn đọc lại.
            </div>

            {lookups.length > 0 && (
              <div style={{ marginBottom: 12 }}>
                <div style={{ color: "var(--text-dim)", fontSize: 12, marginBottom: 4 }}>
                  Những chỗ này trông giống nơi game tự ghi lại hình dạng bảng font — là thứ{" "}
                  <i>tìm được</i>, không phải thứ đã kiểm chứng:
                </div>
                {lookups.slice(0, 12).map((lookup) => (
                  <div
                    key={`${lookup.class}-${lookup.what}-${lookup.value}`}
                    style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 2 }}
                  >
                    · <span style={{ fontFamily: "var(--mono)" }}>{lookup.class}</span> giữ{" "}
                    {lookup.value.length > 24 ? `${lookup.value.slice(0, 24)}…` : lookup.value} ({lookup.what})
                  </div>
                ))}
              </div>
            )}
            <div className="wrap" style={{ marginBottom: rules.length > 0 ? 14 : 0 }}>
              <button
                disabled={busy !== null}
                onClick={async () => {
                  const updated = await run("rule", () => api.writeFontInstallRule(path));
                  if (updated) {
                    setRules(updated);
                    say("Đã viết luật install-font — đang tắt");
                  }
                }}
              >
                {busy === "rule" ? <span className="spin" /> : null} Viết luật cài font
              </button>
            </div>

            {rules.map((r) => (
              <div
                key={r.id}
                style={{ borderTop: "1px solid var(--line-soft)", padding: "11px 0 4px" }}
              >
                <div className="row">
                  <span style={{ fontFamily: "var(--mono)", fontSize: 12.5, flex: 1 }}>{r.id}</span>
                  <span className={r.ready ? "pill ok" : r.enabled ? "pill bad" : "pill"}>
                    {r.ready ? "đang chạy" : r.enabled ? "bật, không khớp game" : "tắt"}
                  </span>
                  <button
                    disabled={busy !== null}
                    onClick={async () => {
                      const updated = await run("toggle", () =>
                        api.setRuleEnabled(path, r.id, !r.enabled),
                      );
                      if (updated) {
                        setRules(updated);
                        await load();
                      }
                    }}
                  >
                    {r.enabled ? "Tắt" : "Bật"}
                  </button>
                  <button
                    disabled={busy !== null}
                    onClick={async () => {
                      const updated = await run("remove", () => api.removeRule(path, r.id));
                      if (updated) {
                        setRules(updated);
                        await load();
                      }
                    }}
                  >
                    Xoá
                  </button>
                </div>
                {r.description && (
                  <div style={{ color: "var(--text-faint)", fontSize: 12, marginTop: 5 }}>
                    {r.description}
                  </div>
                )}
                {r.effects.map((e) => (
                  <div key={e} style={{ fontSize: 12, marginTop: 4 }}>
                    → {e}
                  </div>
                ))}
                {r.unmet.map((u) => (
                  <div key={u} style={{ color: "var(--bad)", fontSize: 12, marginTop: 4 }}>
                    ✕ {u}
                  </div>
                ))}
                {r.effects.length === 0 && r.unmet.length === 0 && (
                  <div style={{ color: "var(--text-faint)", fontSize: 12, marginTop: 4 }}>
                    Không khớp gì trong game này — luật sẽ không làm gì cả.
                  </div>
                )}
              </div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}
