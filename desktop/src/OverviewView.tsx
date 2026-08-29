import type { CapabilityView, ProjectSummary } from "./types";

interface Props {
  project: ProjectSummary;
  capabilities: CapabilityView[];
  busy: string | null;
  onAnalyze: () => void;
  onExtract: () => void;
  onSuggest: () => void;
  onApplySafe: () => void;
  onLearn: () => void;
  onSetBranding: (enabled: boolean) => void;
  onSetLocalization: (target: string, style: string) => void;
}

/** The pipeline, in the order §22 runs it, with each step's state visible rather than implied. */
export function OverviewView(props: Props) {
  const { project: p, capabilities, busy } = props;
  const analyzed = !p.needsAnalyze;
  const extracted = !p.needsExtract;

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
              <Step
                n={1}
                done
                title="Nhập bản gốc"
                detail={`Đã băm và khoá — sửa file gốc sẽ bị báo lỗi`}
              />
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
                  <button
                    className="small"
                    disabled={busy !== null || !analyzed}
                    onClick={props.onExtract}
                  >
                    {extracted ? "Trích xuất lại" : "Trích xuất"}
                  </button>
                }
              />
              <Step
                n={4}
                done={p.approvedCount > 0}
                title="Dịch"
                detail={`${p.approvedCount}/${p.translatableCount} đã duyệt`}
                action={
                  <div className="row">
                    <button
                      className="small"
                      disabled={busy !== null || !extracted}
                      onClick={props.onSuggest}
                    >
                      Gợi ý
                    </button>
                    <button
                      className="small"
                      disabled={busy !== null || !extracted}
                      onClick={props.onApplySafe}
                    >
                      Duyệt phần chắc chắn
                    </button>
                  </div>
                }
              />
              <Step
                n={5}
                done={p.buildCount > 0}
                title="Đóng gói và kiểm tra"
                detail={
                  p.buildCount > 0 ? `${p.buildCount} bản build đã ghi nhận` : "Chưa build lần nào"
                }
              />
            </div>
          </div>

          <div className="card">
            <h3>Bộ nhớ dịch</h3>
            <div className="sub">
              Đưa các bản dịch đã duyệt vào bộ nhớ để dự án sau dùng lại.
            </div>
            <button disabled={busy !== null || p.approvedCount === 0} onClick={props.onLearn}>
              Ghi nhớ {p.approvedCount} bản dịch
            </button>
          </div>
        </div>

        <div>
          <div className="card">
            <h3>Khả năng phát hiện được</h3>
            <div className="sub">
              Phần lõi hỏi game này <i>làm được gì</i>, không bao giờ hỏi nó <i>là game nào</i>.
            </div>
            {capabilities.length === 0 ? (
              <div style={{ color: "var(--text-faint)", fontSize: 12.5 }}>
                Chưa phân tích.
              </div>
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
          </div>

          <div className="card">
            <h3>Thiết lập</h3>
            <div className="sub">Ghi vào project.json và áp dụng cho lần build sau.</div>
            <div className="row" style={{ marginBottom: 10 }}>
              <label style={{ width: 108, color: "var(--text-faint)", fontSize: 12 }}>
                Ngôn ngữ đích
              </label>
              <input
                value={p.targetLanguage}
                onChange={(e) => props.onSetLocalization(e.target.value, p.styleProfile)}
              />
            </div>
            <div className="row" style={{ marginBottom: 12 }}>
              <label style={{ width: 108, color: "var(--text-faint)", fontSize: 12 }}>
                Văn phong
              </label>
              <select
                value={p.styleProfile}
                onChange={(e) => props.onSetLocalization(p.targetLanguage, e.target.value)}
              >
                <option value="natural-dialogue">natural-dialogue</option>
                <option value="formal">formal</option>
                <option value="literal">literal</option>
              </select>
            </div>
            <label className="row" style={{ cursor: "pointer" }}>
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
