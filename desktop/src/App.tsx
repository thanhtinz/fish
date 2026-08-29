import { useCallback, useEffect, useState } from "react";
import { api, isDesktop, pickFolder, pickJar } from "./api";
import { OverviewView } from "./OverviewView";
import { TextView } from "./TextView";
import { BuildsView } from "./BuildView";
import type { BuildView, CapabilityView, NodeView, ProjectSummary } from "./types";

type Tab = "overview" | "text" | "build";

export function App() {
  const [recents, setRecents] = useState<ProjectSummary[]>([]);
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [capabilities, setCapabilities] = useState<CapabilityView[]>([]);
  const [nodes, setNodes] = useState<NodeView[]>([]);
  const [builds, setBuilds] = useState<BuildView[]>([]);
  const [outputPath, setOutputPath] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("overview");
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<{ text: string; bad: boolean } | null>(null);

  const say = useCallback((text: string, bad = false) => {
    setToast({ text, bad });
    window.setTimeout(() => setToast(null), bad ? 6000 : 2600);
  }, []);

  /** Every action funnels through here so no path can leave the interface stuck on a spinner. */
  const run = useCallback(
    async <T,>(label: string, work: () => Promise<T>): Promise<T | null> => {
      setBusy(label);
      try {
        return await work();
      } catch (e) {
        say(String(e instanceof Error ? e.message : e), true);
        return null;
      } finally {
        setBusy(null);
      }
    },
    [say],
  );

  const loadProject = useCallback(
    async (path: string) => {
      const summary = await run("open", () => api.openProject(path));
      if (!summary) return;
      setProject(summary);
      setCapabilities(await api.capabilities(path).catch(() => []));
      setNodes(summary.needsExtract ? [] : await api.nodes(path).catch(() => []));
      setBuilds(await api.builds(path).catch(() => []));
      setOutputPath(await api.outputPath(path).catch(() => null));
      setRecents(await api.recentProjects().catch(() => []));
    },
    [run],
  );

  /** Re-reads everything the backend owns. Cheaper than tracking what each action invalidated. */
  const refresh = useCallback(async () => {
    if (!project) return;
    const path = project.path;
    setProject(await api.projectSummary(path).catch(() => project));
    setNodes(await api.nodes(path).catch(() => []));
    setBuilds(await api.builds(path).catch(() => []));
    setOutputPath(await api.outputPath(path).catch(() => null));
  }, [project]);

  useEffect(() => {
    if (!isDesktop) return;
    api.recentProjects().then(setRecents).catch(() => setRecents([]));
  }, []);

  async function doImport() {
    const jar = await pickJar();
    if (!jar) return;
    const into = await pickFolder();
    if (!into) return;
    const summary = await run("import", () => api.importJar(jar, into, null));
    if (summary) {
      await loadProject(summary.path);
      say(`Đã nhập ${summary.name}`);
    }
  }

  async function doOpen() {
    const dir = await pickFolder();
    if (dir) await loadProject(dir);
  }

  if (!isDesktop) {
    return (
      <div className="app">
        <div className="empty-state">
          <div className="inner">
            <h2>Cần vỏ ứng dụng desktop</h2>
            <p>
              Giao diện này gọi thẳng vào phần lõi Rust qua Tauri. Chạy trong trình duyệt thì không
              có backend nào để gọi, nên nó không hiển thị gì cả — thà trống còn hơn bày dữ liệu
              không có thật.
            </p>
            <p style={{ fontFamily: "var(--mono)", fontSize: 12 }}>cargo tauri dev</p>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="app">
      <div className="titlebar">
        <span className="brand">
          <span className="mark">TJ</span>
          TJLocalizer
        </span>
        {project && (
          <span className="crumb">
            <span className="sep">/</span>
            <b>{project.name}</b>
            <span className="pill">{project.targetLanguage}</span>
            <span className="pill">{project.styleProfile}</span>
          </span>
        )}
        <span className="spacer" />
        {busy && (
          <span style={{ color: "var(--text-faint)", fontSize: 12 }}>
            <span className="spin" /> {busy}…
          </span>
        )}
      </div>

      <div className="body">
        <div className="sidebar">
          <div className="head">Dự án gần đây</div>
          <div className="list">
            {recents.length === 0 && (
              <div style={{ padding: "4px 10px", color: "var(--text-faint)", fontSize: 12 }}>
                Chưa có dự án nào.
              </div>
            )}
            {recents.map((r) => (
              <button
                key={r.path}
                className={project?.path === r.path ? "proj on" : "proj"}
                onClick={() => loadProject(r.path)}
              >
                <div className="n">{r.name}</div>
                <div className="m">
                  {r.targetLanguage} · {r.approvedCount}/{r.translatableCount} đã dịch
                </div>
              </button>
            ))}
          </div>
          <div className="foot">
            <button className="primary" disabled={busy !== null} onClick={doImport}>
              Nhập file JAR…
            </button>
            <button disabled={busy !== null} onClick={doOpen}>
              Mở dự án có sẵn…
            </button>
          </div>
        </div>

        <div className="main">
          {!project ? (
            <div className="empty-state">
              <div className="inner">
                <h2>Chưa mở dự án nào</h2>
                <p>
                  Nhập một file JAR để bắt đầu. Bản gốc được băm và giữ nguyên vẹn; mọi thứ khác
                  đều dựng lại được từ nó.
                </p>
                <button className="primary" onClick={doImport}>
                  Nhập file JAR…
                </button>
              </div>
            </div>
          ) : (
            <>
              <div className="tabs">
                <button
                  className={tab === "overview" ? "tab on" : "tab"}
                  onClick={() => setTab("overview")}
                >
                  Tổng quan
                </button>
                <button
                  className={tab === "text" ? "tab on" : "tab"}
                  disabled={project.needsExtract}
                  onClick={() => setTab("text")}
                >
                  Văn bản {project.nodeCount > 0 && `(${project.nodeCount})`}
                </button>
                <button
                  className={tab === "build" ? "tab on" : "tab"}
                  onClick={() => setTab("build")}
                >
                  Đóng gói {project.buildCount > 0 && `(${project.buildCount})`}
                </button>
              </div>

              <div className="view">
                {tab === "overview" && (
                  <OverviewView
                    project={project}
                    capabilities={capabilities}
                    busy={busy}
                    onAnalyze={async () => {
                      const caps = await run("analyze", () => api.analyze(project.path));
                      if (caps) {
                        setCapabilities(caps);
                        await refresh();
                        say(`${caps.length} khả năng được phát hiện`);
                      }
                    }}
                    onExtract={async () => {
                      const n = await run("extract", () => api.extract(project.path));
                      if (n !== null) {
                        await refresh();
                        say(`${n} chuỗi được trích xuất`);
                      }
                    }}
                    onSuggest={async () => {
                      const n = await run("suggest", () => api.suggestAll(project.path, 0.75));
                      if (n !== null) {
                        await refresh();
                        say(`${n} gợi ý`);
                      }
                    }}
                    onApplySafe={async () => {
                      const n = await run("apply", () => api.applySafe(project.path));
                      if (n !== null) {
                        await refresh();
                        say(`Đã duyệt ${n} bản dịch chắc chắn`);
                      }
                    }}
                    onLearn={async () => {
                      const n = await run("learn", () => api.learn(project.path));
                      if (n !== null) say(`Bộ nhớ dịch có ${n} mục`);
                    }}
                    onSetBranding={async (enabled) => {
                      const s = await run("save", () => api.setBranding(project.path, enabled));
                      if (s) setProject(s);
                    }}
                    onSetLocalization={async (target, style) => {
                      const s = await run("save", () =>
                        api.setLocalization(project.path, target, style),
                      );
                      if (s) setProject(s);
                    }}
                  />
                )}

                {tab === "text" && (
                  <TextView
                    nodes={nodes}
                    onSetTranslation={async (nodeId, target) => {
                      await run("save", () => api.setTranslation(project.path, nodeId, target));
                      await refresh();
                    }}
                  />
                )}

                {tab === "build" && (
                  <BuildsView
                    project={project}
                    builds={builds}
                    outputPath={outputPath}
                    busy={busy}
                    onBuild={async () => {
                      const b = await run("build", () => api.build(project.path));
                      if (b) {
                        await refresh();
                        say(
                          b.ok
                            ? `Build ${b.revision} đạt kiểm tra`
                            : `Build ${b.revision} không đạt kiểm tra`,
                          !b.ok,
                        );
                      }
                    }}
                    onRollback={async (revision) => {
                      const b = await run("rollback", () => api.rollback(project.path, revision));
                      if (b) {
                        await refresh();
                        say(`Đã khôi phục bản build ${b.revision}`);
                      }
                    }}
                  />
                )}
              </div>
            </>
          )}
        </div>
      </div>

      <div className="statusbar">
        {project ? (
          <>
            <span>{project.nodeCount} chuỗi</span>
            <span className="sep">·</span>
            <span>
              {project.approvedCount}/{project.translatableCount} đã duyệt
            </span>
            <span className="sep">·</span>
            <span>{project.buildCount} bản build</span>
            <span className="sep">·</span>
            <span>hồ sơ r{project.revision}</span>
          </>
        ) : (
          <span>Sẵn sàng</span>
        )}
        <span style={{ marginLeft: "auto" }}>Việt hoá bởi Thanhtinz · © 2026</span>
      </div>

      {toast && <div className={toast.bad ? "toast bad" : "toast"}>{toast.text}</div>}
    </div>
  );
}
