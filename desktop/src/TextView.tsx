import { useEffect, useMemo, useState } from "react";
import type { GlossView, NodeView } from "./types";

interface Props {
  nodes: NodeView[];
  onSetTranslation: (nodeId: string, target: string) => void;
  onGloss: (nodeId: string) => Promise<GlossView | null>;
  onEngine: ((nodeId: string) => Promise<GlossView | null>) | null;
  onExport: () => void;
  onImport: () => void;
}

type StatusFilter = "all" | "untranslated" | "translated" | "suggested" | "issues";

/**
 * The translation table.
 *
 * Non-translatable nodes are hidden by default rather than removed: a translator should be able
 * to see that `/img/hud.png` was found and deliberately left alone, otherwise "where did that
 * string go?" has no answer.
 */
export function TextView({
  nodes,
  onSetTranslation,
  onGloss,
  onEngine,
  onExport,
  onImport,
}: Props) {
  const [query, setQuery] = useState("");
  const [context, setContext] = useState("all");
  const [status, setStatus] = useState<StatusFilter>("all");
  const [showTechnical, setShowTechnical] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [draft, setDraft] = useState<string>("");
  const [gloss, setGloss] = useState<GlossView | null>(null);
  const [glossing, setGlossing] = useState(false);
  const [engine, setEngine] = useState<GlossView | null>(null);
  const [asking, setAsking] = useState(false);

  const contexts = useMemo(
    () => Array.from(new Set(nodes.map((n) => n.context))).sort(),
    [nodes],
  );

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return nodes.filter((n) => {
      if (!showTechnical && !n.translatable) return false;
      if (context !== "all" && n.context !== context) return false;
      if (status === "untranslated" && n.target) return false;
      if (status === "translated" && !n.target) return false;
      if (status === "suggested" && (!n.candidate || n.target)) return false;
      if (status === "issues" && n.issues.length === 0) return false;
      if (q && !n.source.toLowerCase().includes(q) && !(n.target ?? "").toLowerCase().includes(q))
        return false;
      return true;
    });
  }, [nodes, query, context, status, showTechnical]);

  const current = nodes.find((n) => n.id === selected) ?? null;

  // The offline engine runs per row, on selection, rather than over the whole table: it is a
  // starting point a translator asks for, not something to fill a column with.
  useEffect(() => {
    let cancelled = false;
    setGloss(null);
    // The engine result is cleared with the selection: it was about the previous string, and
    // leaving it on screen beside a new one is how a translation lands on the wrong row.
    setEngine(null);
    if (!selected) return;
    setGlossing(true);
    onGloss(selected)
      .then((g) => {
        if (!cancelled) setGloss(g);
      })
      .catch(() => {
        if (!cancelled) setGloss(null);
      })
      .finally(() => {
        if (!cancelled) setGlossing(false);
      });
    return () => {
      cancelled = true;
    };
  }, [selected, onGloss]);

  function select(node: NodeView) {
    setSelected(node.id);
    setDraft(node.target ?? "");
  }

  function commit() {
    if (current && draft !== (current.target ?? "")) {
      onSetTranslation(current.id, draft);
    }
  }

  return (
    <div className="text-view">
      <div className="text-left">
        <div className="filters">
          <input
            type="search"
            placeholder="Tìm trong nguồn hoặc bản dịch…"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <select value={context} onChange={(e) => setContext(e.target.value)}>
            <option value="all">Mọi ngữ cảnh</option>
            {contexts.map((c) => (
              <option key={c} value={c}>
                {c}
              </option>
            ))}
          </select>
          <select value={status} onChange={(e) => setStatus(e.target.value as StatusFilter)}>
            <option value="all">Mọi trạng thái</option>
            <option value="untranslated">Chưa dịch</option>
            <option value="translated">Đã dịch</option>
            <option value="suggested">Có gợi ý</option>
            <option value="issues">Có vấn đề</option>
          </select>
          <label className="row" style={{ whiteSpace: "nowrap", cursor: "pointer" }}>
            <input
              type="checkbox"
              style={{ width: "auto" }}
              checked={showTechnical}
              onChange={(e) => setShowTechnical(e.target.checked)}
            />
            <span style={{ fontSize: 12, color: "var(--text-dim)" }}>Hiện chuỗi kỹ thuật</span>
          </label>
          <span className="count">
            {shown.length}/{nodes.length}
          </span>
          <button className="small ghost" onClick={onExport} title="Xuất ra CSV cho người dịch">
            Xuất CSV…
          </button>
          <button className="small ghost" onClick={onImport} title="Nhập lại CSV người dịch gửi về">
            Nhập CSV…
          </button>
        </div>

        <div className="rows">
          {shown.map((n) => (
            <div
              key={n.id}
              className={n.id === selected ? "trow on" : "trow"}
              onClick={() => select(n)}
            >
              <div className="src">{n.source}</div>
              <div
                className={
                  n.target ? "tgt" : n.candidate ? "tgt cand" : "tgt empty"
                }
              >
                {n.target ?? (n.candidate ? n.candidate.target : "chưa dịch")}
              </div>
              <div className="meta">
                <span className={`pill ${n.context}`}>{n.context}</span>
                {n.issues.length > 0 && (
                  <span className={n.issues.some((i) => i.blocking) ? "pill bad" : "pill warn"}>
                    {n.issues.length} vấn đề
                  </span>
                )}
                {!n.target && n.candidate && (
                  <span className="pill">gợi ý</span>
                )}
              </div>
            </div>
          ))}
          {shown.length === 0 && (
            <div style={{ padding: 30, textAlign: "center", color: "var(--text-faint)" }}>
              Không có chuỗi nào khớp bộ lọc.
            </div>
          )}
        </div>
      </div>

      <div className="detail">
        {!current ? (
          <div style={{ color: "var(--text-faint)", fontSize: 12.5, lineHeight: 1.7 }}>
            Chọn một dòng để xem gốc, vị trí, gợi ý và các cảnh báo chất lượng.
          </div>
        ) : (
          <>
            <div className="block">
              <h4>Nguyên bản</h4>
              <div className="src-text">{current.source}</div>
              {current.sourceEncoding && (
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 6 }}>
                  Bảng mã gốc: {current.sourceEncoding}
                </div>
              )}
            </div>

            <div className="block">
              <h4>Vị trí</h4>
              <dl className="facts">
                <dt>{current.location.kind === "class" ? "Lớp" : "Tài nguyên"}</dt>
                <dd>{current.location.file}</dd>
                <dt>Tại</dt>
                <dd>{current.location.detail}</dd>
              </dl>
            </div>

            {current.sourceWidth !== undefined && (
              <div className="block">
                <h4>Bề ngang khi vẽ</h4>
                <div className="row" style={{ gap: 8 }}>
                  <span style={{ fontFamily: "var(--mono)", fontSize: 12.5 }}>
                    {current.sourceWidth}px
                  </span>
                  <span style={{ color: "var(--text-faint)" }}>→</span>
                  <span
                    style={{
                      fontFamily: "var(--mono)",
                      fontSize: 12.5,
                      color:
                        current.targetWidth !== undefined &&
                        current.targetWidth > current.sourceWidth * 1.5
                          ? "var(--warn)"
                          : undefined,
                    }}
                  >
                    {current.targetWidth !== undefined ? `${current.targetWidth}px` : "—"}
                  </span>
                </div>
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 5 }}>
                  Đo bằng chính chữ của game. Số ký tự không nói lên điều này: chữ hoa rộng gấp mấy
                  lần chữ i, còn dấu tiếng Việt thì gần như không tốn thêm bề ngang.
                </div>
              </div>
            )}

            {current.placeholders.length > 0 && (
              <div className="block">
                <h4>Placeholder phải giữ nguyên</h4>
                <div className="wrap">
                  {current.placeholders.map((p) => (
                    <span className="pill format" key={p}>
                      {p}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {current.candidate && (
              <div className="block">
                <h4>Gợi ý</h4>
                <div className="cand-box">
                  <div className="row">
                    <span className="pill">
                      {current.candidate.origin === "glossary"
                        ? "thuật ngữ"
                        : current.candidate.origin === "memory"
                          ? "bộ nhớ — khớp chính xác"
                          : `bộ nhớ — gần đúng ${current.candidate.score?.toFixed(2)}`}
                    </span>
                    {current.candidate.autoApprovable ? (
                      <span className="pill ok">duyệt tự động được</span>
                    ) : (
                      <span className="pill warn">cần người duyệt</span>
                    )}
                  </div>
                  <div className="t">{current.candidate.target}</div>
                  <button
                    className="small"
                    onClick={() => setDraft(current.candidate!.target)}
                  >
                    Dùng gợi ý này
                  </button>
                </div>
                {!current.candidate.autoApprovable && (
                  <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 8, lineHeight: 1.6 }}>
                    Khớp gần đúng không bao giờ được duyệt tự động: “Mở khóa” và “Mở khoá” gần như
                    giống hệt nhau và một trong hai là sai.
                  </div>
                )}
              </div>
            )}

            <div className="block">
              <h4>Từ điển đề xuất</h4>
              {glossing ? (
                <div style={{ color: "var(--text-faint)", fontSize: 12 }}>
                  <span className="spin" /> đang tra…
                </div>
              ) : !gloss ? (
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, lineHeight: 1.6 }}>
                  Từ điển không tra được đủ chuỗi này. Ghép vài từ nó tình cờ biết lại thành câu
                  sẽ ra thứ trông như bản dịch mà không phải, nên nó không đề xuất gì.
                </div>
              ) : (
                <div className="cand-box">
                  <div className="row" style={{ flexWrap: "wrap" }}>
                    <span className="pill">{gloss.engine}</span>
                    <span className={gloss.completeness === "complete" ? "pill ok" : "pill warn"}>
                      {gloss.completeness === "complete" ? "tra đủ" : "tra một phần"}
                    </span>
                    <span className="pill">{gloss.confidence.toFixed(2)}</span>
                  </div>
                  <div className="t">{gloss.text}</div>
                  {gloss.terms.length > 0 && (
                    <div className="wrap" style={{ marginBottom: 8 }}>
                      {gloss.terms.map((t, k) => (
                        <span className="pill" key={k} title={t.domain}>
                          {t.source} → {t.target}
                        </span>
                      ))}
                    </div>
                  )}
                  {gloss.unresolved.length > 0 && (
                    <div style={{ color: "var(--warn)", fontSize: 11.5, marginBottom: 8 }}>
                      chưa tra được: {gloss.unresolved.join(" · ")}
                    </div>
                  )}
                  <button className="small" onClick={() => setDraft(gloss.text)}>
                    Dùng làm nháp
                  </button>
                </div>
              )}
              {gloss && (
                <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 8, lineHeight: 1.6 }}>
                  Từ điển tra thuật ngữ, không dịch câu. Kể cả khi tra đủ, đây vẫn là điểm bắt đầu
                  cho người dịch — không bao giờ được duyệt tự động.
                </div>
              )}
            </div>

            {onEngine && (
              <div className="block">
                <h4>Máy dịch ngoài</h4>
                {!engine ? (
                  <>
                    <button
                      className="small"
                      disabled={asking}
                      onClick={async () => {
                        setAsking(true);
                        try {
                          setEngine(await onEngine(current.id));
                        } finally {
                          setAsking(false);
                        }
                      }}
                    >
                      {asking ? <span className="spin" /> : null} Hỏi máy dịch về câu này
                    </button>
                    <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 7, lineHeight: 1.6 }}>
                      Gửi riêng câu này đi, khi anh bấm. Không có gì tự động gửi.
                    </div>
                  </>
                ) : engine.completeness === "none" ? (
                  <div className="banner bad">
                    <span>{engine.notes.join(" · ")}</span>
                  </div>
                ) : (
                  <div className="cand-box">
                    <div className="row" style={{ flexWrap: "wrap" }}>
                      <span className="pill">{engine.engine}</span>
                      <span className={engine.notes.length === 0 ? "pill ok" : "pill warn"}>
                        {engine.notes.length === 0 ? "qua kiểm tra" : `${engine.notes.length} cảnh báo`}
                      </span>
                      <span className="pill">{engine.confidence.toFixed(2)}</span>
                    </div>
                    <div className="t">{engine.text}</div>
                    {engine.notes.map((n, k) => (
                      <div className="issue" key={k}>
                        <span>{n}</span>
                      </div>
                    ))}
                    <button className="small" onClick={() => setDraft(engine.text)}>
                      Dùng làm nháp
                    </button>
                  </div>
                )}
              </div>
            )}

            <div className="block">
              <h4>Bản dịch</h4>
              <textarea
                rows={4}
                value={draft}
                placeholder="Nhập bản dịch tiếng Việt…"
                onChange={(e) => setDraft(e.target.value)}
                onBlur={commit}
              />
              <div className="row" style={{ marginTop: 8 }}>
                <button
                  className="primary small"
                  disabled={draft === (current.target ?? "")}
                  onClick={commit}
                >
                  Lưu
                </button>
                <button
                  className="ghost small"
                  disabled={!current.target}
                  onClick={() => {
                    setDraft("");
                    onSetTranslation(current.id, "");
                  }}
                >
                  Xoá bản dịch
                </button>
              </div>
            </div>

            {current.issues.length > 0 && (
              <div className="block">
                <h4>Cảnh báo</h4>
                {current.issues.map((i, k) => (
                  <div className={i.blocking ? "issue blocking" : "issue"} key={k}>
                    <span className="code">{i.code}</span>
                    <span>{i.detail}</span>
                  </div>
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
