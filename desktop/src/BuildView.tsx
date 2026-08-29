import type { BuildView as Build, ProjectSummary } from "./types";

interface Props {
  project: ProjectSummary;
  language: string;
  builds: Build[];
  outputPath: string | null;
  busy: string | null;
  onBuild: () => void;
  onBuildAll: () => void;
  onRollback: (revision: number) => void;
  onExport: () => void;
  /// Null unless this project's game is a directory, so the patch controls are absent rather
  /// than present and refusing.
  onApplyPatch: (() => void) | null;
}

/** Build, validation result, and the history that makes a rollback possible. */
export function BuildsView(props: Props) {
  const { project: p, builds, busy, language } = props;
  const latest = builds[0] ?? null;
  const target = p.targets.find((t) => t.tag === language);
  const enabled = p.targets.filter((t) => t.enabled);

  return (
    <div className="pad">
      <div className="card">
        <h3>Đóng gói</h3>
        <div className="sub">
          Áp bản dịch đã duyệt, đóng gói lại, rồi kiểm tra. Kết quả ghi vào builds/ trước, chép
          sang output/ sau — nên output/ chỉ chứa bản build đã hoàn tất.
          {props.onApplyPatch && (
            <>
              {" "}Game này là một thư mục, nên kết quả là một <b>gói vá</b>: chỉ những file đã đổi.
              Build <b>không bao giờ</b> ghi vào thư mục game — áp vá là một việc riêng, bấm riêng.
            </>
          )}
        </div>
        <div className="row" style={{ flexWrap: "wrap" }}>
          <button
            className="primary"
            disabled={busy !== null || p.needsExtract}
            onClick={props.onBuild}
          >
            {busy === "build" ? <span className="spin" /> : null} Build {target?.name ?? language}
          </button>
          {enabled.length > 1 && (
            <button disabled={busy !== null || p.needsExtract} onClick={props.onBuildAll}>
              Build cả {enabled.length} ngôn ngữ
            </button>
          )}
          <button disabled={busy !== null || !props.outputPath} onClick={props.onExport}>
            Xuất file ra…
          </button>
          {props.onApplyPatch && (
            <button
              disabled={busy !== null || !props.outputPath}
              onClick={props.onApplyPatch}
              title="Ghi các file đã đổi vào thư mục game. Bản cũ được giữ lại."
            >
              Áp vá vào game…
            </button>
          )}
          <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
            {target?.approvedCount ?? 0} bản dịch sẽ được áp
          </span>
        </div>
        {props.outputPath && (
          <dl className="facts" style={{ marginTop: 14 }}>
            <dt>Kết quả</dt>
            <dd>{props.outputPath}</dd>
          </dl>
        )}
      </div>

      {latest && (
        <div className="card">
          <h3>
            Kiểm tra bản build {String(latest.revision).padStart(4, "0")}{" "}
            {latest.ok ? (
              <span className="pill ok">đạt</span>
            ) : (
              <span className="pill bad">không đạt</span>
            )}
          </h3>
          <div className="sub">
            Không mất entry, mọi lớp còn parse được, entry point còn đó, placeholder còn nguyên,
            và manifest gốc không bị đổi.
          </div>
          {latest.findings.length === 0 ? (
            <div style={{ color: "var(--ok)", fontSize: 12.5 }}>
              Không phát hiện vấn đề nào.
            </div>
          ) : (
            latest.findings.map((f, k) => (
              <div className="finding" key={k}>
                <span className={`sv ${f.severity}`}>
                  {f.severity === "error" ? "lỗi" : "cảnh báo"}
                </span>
                <span className="ck">{f.check}</span>
                <span>{f.detail}</span>
              </div>
            ))
          )}
        </div>
      )}

      <div className="card">
        <h3>Lịch sử build</h3>
        <div className="sub">
          Mỗi bản giữ lại file kết quả và hash bản gốc nó được tạo ra từ đó, nên quay lui được mà
          không mất bản mới hơn.
        </div>
        {builds.length === 0 ? (
          <div style={{ color: "var(--text-faint)", fontSize: 12.5 }}>Chưa có bản build nào.</div>
        ) : (
          builds.map((b) => (
            <div className="build-row" key={b.revision}>
              <span className="rev">{String(b.revision).padStart(4, "0")}</span>
              <span>
                <div className="sum">
                  {b.literalsPatched} chuỗi trong lớp, {b.resourcesPatched} trong tài nguyên ·{" "}
                  {b.ok ? (
                    <span style={{ color: "var(--ok)" }}>đạt</span>
                  ) : (
                    <span style={{ color: "var(--bad)" }}>không đạt</span>
                  )}
                </div>
                <div className="sha">{b.outputSha256}</div>
              </span>
              <button
                className="small"
                disabled={busy !== null}
                onClick={() => props.onRollback(b.revision)}
              >
                Khôi phục
              </button>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
