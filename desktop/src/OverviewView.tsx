import { Fragment, useState } from "react";
import { AnalystCard } from "./AnalystCard";
import { EngineCard } from "./EngineCard";
import { CastCard } from "./CastCard";
import { PluginsCard } from "./PluginsCard";
import type {
  AnalystView,
  CapabilityView,
  DictionaryView,
  EnginePreview,
  EngineView,
  InspectionView,
  LanguageView,
  ProjectSummary,
  ScanPreview,
  StyleView,
  SuggestionView,
} from "./types";

interface Props {
  project: ProjectSummary;
  language: string;
  capabilities: CapabilityView[];
  languages: LanguageView[];
  styles: StyleView[];
  dictionaries: DictionaryView[];
  busy: string | null;
  onAnalyze: () => void;
  onExtract: () => void;
  onSuggest: () => void;
  onApplySafe: () => void;
  onLearn: () => void;
  onSetBranding: (enabled: boolean) => void;
  onSetSourceLanguage: (tag: string) => void;
  onSetStyle: (tag: string) => void;
  onAddTarget: (tag: string) => void;
  onRemoveTarget: (tag: string) => void;
  onImportDictionary: () => void;
  engine: EngineView | null;
  onSaveEngine: (kind: string, endpoint: string, model: string | null, enabled: boolean) => void;
  onSaveEngineKey: (key: string) => void;
  onPreviewEngine: (text: string) => Promise<EnginePreview | null>;
  analyst: AnalystView | null;
  suggestions: SuggestionView[];
  onSaveAnalyst: (model: string, enabled: boolean) => void;
  onSaveAnalystKey: (key: string) => void;
  onPreviewScan: () => Promise<ScanPreview | null>;
  onScan: () => void;
  onInspect: (entry: string) => Promise<InspectionView | null>;
}

/** The pipeline in the order §22 runs it, with each step's state visible rather than implied. */
export function OverviewView(props: Props) {
  const { project: p, capabilities, busy, language } = props;
  const [newLang, setNewLang] = useState("");
  const analyzed = !p.needsAnalyze;
  const extracted = !p.needsExtract;
  const target = p.targets.find((t) => t.tag === language);
  const styles = props.styles.filter(
    (s) => s.language.split("-")[0] === language.split("-")[0],
  );

  // The directions this project can actually gloss in - a dictionary for the wrong source
  // language is no dictionary at all, and that is invisible unless it is said.
  const usable = props.dictionaries.filter(
    (d) =>
      d.from.split("-")[0] === p.sourceLanguage.split("-")[0] &&
      d.to.split("-")[0] === language.split("-")[0],
  );

  return (
    <div className="pad">
      <div className="grid2">
        <div>
          <div className="card">
            <h3>Quy trình</h3>
            <div className="sub">
              Mỗi bước ghi kết quả xuống đĩa, nên có thể dừng ở đây và làm tiếp sau.
            </div>
            <div className="steps">
              <Step n={1} done title="Nhập bản gốc" detail="Đã băm và khoá — sửa file gốc sẽ bị báo lỗi" />
              <Step
                n={2}
                done={analyzed}
                title="Phân tích khả năng"
                detail={
                  analyzed
                    ? `${capabilities.length} khả năng được phát hiện`
                    : "Xem game này hỗ trợ gì — không hỏi nó là game nào"
                }
                action={
                  <button className="small" disabled={busy !== null} onClick={props.onAnalyze}>
                    {analyzed ? "Phân tích lại" : "Phân tích"}
                  </button>
                }
              />
              <Step
                n={3}
                done={extracted}
                title="Trích xuất văn bản"
                detail={
                  extracted
                    ? `${p.nodeCount} chuỗi, ${p.translatableCount} dịch được`
                    : "Tìm chuỗi thật sự là nội dung game"
                }
                action={
                  <button className="small" disabled={busy !== null || !analyzed} onClick={props.onExtract}>
                    {extracted ? "Trích xuất lại" : "Trích xuất"}
                  </button>
                }
              />
              <Step
                n={4}
                done={(target?.approvedCount ?? 0) > 0}
                title={`Dịch sang ${target?.name ?? language}`}
                detail={`${target?.approvedCount ?? 0}/${p.translatableCount} đã duyệt`}
                action={
                  <div className="row">
                    <button className="small" disabled={busy !== null || !extracted} onClick={props.onSuggest}>
                      Gợi ý
                    </button>
                    <button className="small" disabled={busy !== null || !extracted} onClick={props.onApplySafe}>
                      Duyệt phần chắc chắn
                    </button>
                  </div>
                }
              />
              <Step
                n={5}
                done={(target?.buildCount ?? 0) > 0}
                title="Đóng gói và kiểm tra"
                detail={
                  (target?.buildCount ?? 0) > 0
                    ? `${target?.buildCount} bản build đã ghi nhận`
                    : "Chưa build lần nào"
                }
              />
            </div>
          </div>

          <div className="card">
            <h3>Ngôn ngữ</h3>
            <div className="sub">
              Một dự án có thể xuất ra nhiều ngôn ngữ. Văn bản gốc dùng chung; bản dịch, văn phong
              và bản build thì riêng cho từng thứ tiếng.
            </div>

            <div className="row" style={{ marginBottom: 12 }}>
              <label style={{ width: 108, color: "var(--text-faint)", fontSize: 12 }}>Ngôn ngữ gốc</label>
              <select
                value={p.sourceLanguage}
                onChange={(e) => props.onSetSourceLanguage(e.target.value)}
              >
                {!props.languages.some((l) => l.tag === p.sourceLanguage) && (
                  <option value={p.sourceLanguage}>{p.sourceLanguage}</option>
                )}
                {props.languages.map((l) => (
                  <option key={l.tag} value={l.tag}>
                    {l.name} ({l.tag})
                  </option>
                ))}
              </select>
            </div>
            {p.sourceLanguageDetected && (
              <div className="banner note" style={{ marginBottom: 12 }}>
                <span>
                  Ngôn ngữ gốc là <b>đoán</b> từ chữ trong game, chưa ai xác nhận. Đoán sai thì
                  toàn bộ từ điển im lặng ngừng hoạt động — nên hãy kiểm lại.
                </span>
              </div>
            )}

            <div style={{ marginBottom: 10 }}>
              {p.targets.map((t) => (
                <div className="build-row" key={t.tag} style={{ marginBottom: 6 }}>
                  <span className="rev">{t.tag}</span>
                  <span>
                    <div className="sum">
                      {t.name} · {t.approvedCount}/{p.translatableCount} đã dịch ·{" "}
                      {t.buildCount} build
                    </div>
                    <div className="sha">{t.styleProfile}</div>
                  </span>
                  <button
                    className="small"
                    disabled={busy !== null || p.targets.length < 2}
                    title={
                      p.targets.length < 2
                        ? "Dự án phải có ít nhất một ngôn ngữ đích"
                        : "Bản dịch vẫn được giữ trên đĩa"
                    }
                    onClick={() => props.onRemoveTarget(t.tag)}
                  >
                    Bỏ
                  </button>
                </div>
              ))}
            </div>

            <div className="row">
              <select value={newLang} onChange={(e) => setNewLang(e.target.value)}>
                <option value="">Thêm ngôn ngữ…</option>
                {props.languages
                  .filter((l) => !p.targets.some((t) => t.tag === l.tag))
                  .map((l) => (
                    <option key={l.tag} value={l.tag}>
                      {l.name} ({l.tag})
                    </option>
                  ))}
              </select>
              <button
                disabled={busy !== null || newLang === ""}
                onClick={() => {
                  props.onAddTarget(newLang);
                  setNewLang("");
                }}
              >
                Thêm
              </button>
            </div>
          </div>

          <div className="card">
            <h3>Bộ nhớ dịch</h3>
            <div className="sub">
              Đưa bản dịch đã duyệt vào bộ nhớ để dự án sau dùng lại. Bộ nhớ riêng cho từng cặp
              ngôn ngữ.
            </div>
            <button
              disabled={busy !== null || (target?.approvedCount ?? 0) === 0}
              onClick={props.onLearn}
            >
              Ghi nhớ {target?.approvedCount ?? 0} bản dịch
            </button>
          </div>
        </div>

        <div>
          <div className="card">
            <h3>Văn phong · {target?.name ?? language}</h3>
            <div className="sub">
              Tiếng Việt không có đại từ trung tính, nên phải chọn. NPC kiếm hiệp nói{" "}
              <i>“Ngươi chắc chứ?”</i>, game hiện đại nói <i>“Bạn có chắc không?”</i> — cùng một
              câu gốc.
            </div>
            {styles.length === 0 ? (
              <div style={{ color: "var(--text-faint)", fontSize: 12.5, lineHeight: 1.6 }}>
                Bản dựng này chưa có bộ luật xưng hô cho {target?.name ?? language}. Bản dịch vẫn
                làm được, chỉ là không có kiểm tra văn phong.
              </div>
            ) : (
              styles.map((s) => (
                <label
                  key={s.id}
                  className="step"
                  style={{ cursor: "pointer", marginBottom: 6 }}
                >
                  <input
                    type="radio"
                    style={{ width: "auto" }}
                    name="style"
                    checked={target?.styleProfile === s.id}
                    onChange={() => props.onSetStyle(s.id)}
                  />
                  <span className="t">
                    <div>{s.id}</div>
                    <div className="d">{s.description}</div>
                    {s.secondPerson && (
                      <div className="d">
                        tôi → <b>{s.firstPerson}</b> · bạn → <b>{s.secondPerson}</b>
                      </div>
                    )}
                  </span>
                </label>
              ))
            )}
          </div>

          <div className="card">
            <h3>Từ điển</h3>
            <div className="sub">
              Từ điển game, không phải từ điển thường: 装备 là “trang bị” chứ không phải “thiết
              bị”, Guild là “bang hội” chứ không phải “hiệp hội”.
            </div>
            {usable.length === 0 ? (
              <div className="banner bad">
                <span>
                  Không có từ điển nào cho {p.sourceLanguageName} → {target?.name ?? language}.
                  Phần gợi ý thuật ngữ sẽ không hoạt động cho cặp này.
                </span>
              </div>
            ) : (
              usable.map((d) => (
                <div className="cap" key={`${d.from}-${d.to}`}>
                  <span className="id">
                    {d.from} → {d.to}
                  </span>
                  <span className="cf">{d.entries}</span>
                  <span className="ev">
                    {d.fromName} sang {d.toName}
                  </span>
                </div>
              ))
            )}
            <div className="row" style={{ marginTop: 12 }}>
              <button disabled={busy !== null} onClick={props.onImportDictionary}>
                Nhập gói từ điển…
              </button>
              <span style={{ color: "var(--text-faint)", fontSize: 11.5 }}>
                {props.dictionaries.reduce((n, d) => n + d.entries, 0)} mục, tổng mọi hướng
              </span>
            </div>
          </div>

          <EngineCard
            engine={props.engine}
            busy={busy}
            onSave={props.onSaveEngine}
            onSaveKey={props.onSaveEngineKey}
            onPreview={props.onPreviewEngine}
          />

          <AnalystCard
            analyst={props.analyst}
            suggestions={props.suggestions}
            busy={busy}
            onSave={props.onSaveAnalyst}
            onSaveKey={props.onSaveAnalystKey}
            onPreview={props.onPreviewScan}
            onScan={props.onScan}
            onInspect={props.onInspect}
          />

          <CastCard path={p.path} revision={p.translatableCount} />

          <PluginsCard path={p.path} />

          <div className="card">
            <h3>Loại gói</h3>
            <div className="sub">
              Nhận ra từ thứ nằm bên trong, không phải từ đuôi file. JAR, APK, IPA hay một file
              zip đều là kho nén giống nhau ở tầng dưới.
            </div>
            <div className="row" style={{ marginBottom: 10, gap: 9 }}>
              <span className={p.package.canRepackage ? "pill ok" : "pill warn"}>
                {p.package.label}
              </span>
              <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
                {p.package.evidence.join(" · ")}
              </span>
            </div>
            {p.package.note && (
              <p style={{ color: "var(--warn)", fontSize: 12.5, margin: "0 0 12px" }}>
                Không đóng gói lại được ở đây: {p.package.note}. Văn bản vẫn đọc, dịch và xuất
                được — chỉ là file kết quả cần bạn tự ký lại bằng khoá của mình.
              </p>
            )}

            {p.package.readable.length > 0 && (
              <>
                <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 5 }}>
                  Đọc được:
                </div>
                <dl className="facts" style={{ marginBottom: 12 }}>
                  {p.package.readable.slice(0, 8).map((r) => (
                    <Fragment key={r.entry}>
                      <dt>{r.format}</dt>
                      <dd>
                        {r.entry} · {r.fields} chuỗi
                        {!r.writable && (
                          <span style={{ color: "var(--warn)" }}> · chỉ đọc, chưa ghi lại được</span>
                        )}
                      </dd>
                    </Fragment>
                  ))}
                </dl>
              </>
            )}

            {p.package.opaque.length > 0 && (
              <>
                <div style={{ fontSize: 12, color: "var(--text-dim)", marginBottom: 5 }}>
                  Chưa đọc được — không phải game không có chữ ở đó:
                </div>
                <dl className="facts">
                  {p.package.opaque.map((o) => (
                    <Fragment key={o.entry}>
                      <dt>{o.entry}</dt>
                      <dd style={{ fontFamily: "inherit", color: "var(--text-faint)" }}>
                        {o.reason}
                      </dd>
                    </Fragment>
                  ))}
                </dl>
              </>
            )}
          </div>

          <div className="card">
            <h3>Khả năng phát hiện được</h3>
            <div className="sub">
              Phần lõi hỏi game này <i>làm được gì</i>, không bao giờ hỏi nó <i>là game nào</i>.
            </div>
            {capabilities.length === 0 ? (
              <div style={{ color: "var(--text-faint)", fontSize: 12.5 }}>Chưa phân tích.</div>
            ) : (
              capabilities.map((c) => (
                <div className="cap" key={c.id}>
                  <span className="id">{c.id}</span>
                  <span className="cf">{c.confidence.toFixed(2)}</span>
                  <span className="ev">{c.evidence.join(" · ")}</span>
                </div>
              ))
            )}
          </div>

          <div className="card">
            <h3>Dự án</h3>
            <div className="sub">Bản gốc là bất biến; hồ sơ có đánh số phiên bản.</div>
            <dl className="facts">
              <dt>Thư mục</dt>
              <dd>{p.path}</dd>
              <dt>SHA-256 gốc</dt>
              <dd>{p.sourceSha256}</dd>
              <dt>Phiên bản hồ sơ</dt>
              <dd>{p.revision}</dd>
            </dl>
            <label className="row" style={{ cursor: "pointer", marginTop: 12 }}>
              <input
                type="checkbox"
                style={{ width: "auto" }}
                checked={p.brandingEnabled}
                onChange={(e) => props.onSetBranding(e.target.checked)}
              />
              <span>Ghi công phần Việt hoá cho Thanhtinz</span>
            </label>
            <div style={{ color: "var(--text-faint)", fontSize: 11.5, marginTop: 7, lineHeight: 1.6 }}>
              Ghi vào file META-INF riêng. Không đụng tới manifest gốc, và không hề nhận quyền sở
              hữu game gốc.
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function Step(props: {
  n: number;
  done?: boolean;
  title: string;
  detail: string;
  action?: React.ReactNode;
}) {
  return (
    <div className={props.done ? "step done" : "step"}>
      <span className="num">{props.done ? "✓" : props.n}</span>
      <span className="t">
        <div>{props.title}</div>
        <div className="d">{props.detail}</div>
      </span>
      {props.action}
    </div>
  );
}
