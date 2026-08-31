import { useCallback, useEffect, useState } from "react";
import { api, isDesktop, pickFile, pickFolder, pickJar, pickSavePath } from "./api";
import { OverviewView } from "./OverviewView";
import { TextView } from "./TextView";
import { BuildsView } from "./BuildView";
import { AssetsView } from "./AssetsView";
import { FontView } from "./FontView";
import type {
  AnalystView,
  BuildView,
  CapabilityView,
  DictionaryView,
  EmulatorSearch,
  EngineView,
  LanguageView,
  NodeView,
  JournalView,
  ProjectSummary,
  RecentView,
  ReviewNoteView,
  StyleView,
  SuggestionView,
} from "./types";

type Tab = "overview" | "text" | "font" | "assets" | "build";

export function App() {
  const [recents, setRecents] = useState<RecentView[]>([]);
  const [project, setProject] = useState<ProjectSummary | null>(null);
  const [language, setLanguage] = useState<string>("");
  const [capabilities, setCapabilities] = useState<CapabilityView[]>([]);
  const [nodes, setNodes] = useState<NodeView[]>([]);
  const [builds, setBuilds] = useState<BuildView[]>([]);
  const [languages, setLanguages] = useState<LanguageView[]>([]);
  const [styles, setStyles] = useState<StyleView[]>([]);
  const [dictionaries, setDictionaries] = useState<DictionaryView[]>([]);
  const [engine, setEngineState] = useState<EngineView | null>(null);
  const [analyst, setAnalystState] = useState<AnalystView | null>(null);
  const [suggestions, setSuggestions] = useState<SuggestionView[]>([]);
  const [journal, setJournal] = useState<JournalView[]>([]);
  // Null until somebody asks: searching the disk is not something to do on every project open.
  const [emulator, setEmulator] = useState<EmulatorSearch | null>(null);
  // Kept in memory rather than on disk: a note is about one reading of one revision, and a stale
  // note left lying beside a row that has since been rewritten is worse than no note.
  const [reviewNotes, setReviewNotes] = useState<Record<string, ReviewNoteView[]>>({});
  const [tab, setTab] = useState<Tab>("overview");
  const [busy, setBusy] = useState<string | null>(null);
  const [toast, setToast] = useState<{ text: string; bad: boolean } | null>(null);

  const say = useCallback((text: string, bad = false) => {
    setToast({ text, bad });
    window.setTimeout(() => setToast(null), bad ? 7000 : 3000);
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

  const target = project?.targets.find((t) => t.tag === language) ?? null;

  // Hoisted deliberately: this was written inline in the Text tab, which made it a hook called
  // only when that tab was open. React tracks hooks by call order, so switching tabs would have
  // shifted every hook after it.
  const projectPath = project?.path ?? "";
  // Null unless an engine is configured and switched on, so the Text tab shows the button only
  // when pressing it could do something - and never when pressing it would reach the network by
  // surprise.
  const engineFor = useCallback(
    (nodeId: string) =>
      projectPath && language && engine?.enabled && engine.hasKey
        ? api.engineTranslate(projectPath, language, nodeId)
        : Promise.resolve(null),
    [projectPath, language, engine?.enabled, engine?.hasKey],
  );

  const glossFor = useCallback(
    (nodeId: string) =>
      projectPath && language
        ? api.gloss(projectPath, language, nodeId)
        : Promise.resolve(null),
    [projectPath, language],
  );

  // Returns null rather than throwing when the game has no declared sheet: not knowing what the
  // font is is the normal state of a fresh project, not an error to interrupt translating with.
  const renderWithGameFont = useCallback(
    (text: string) =>
      projectPath
        ? api.renderText(projectPath, text, 3).catch(() => null)
        : Promise.resolve(null),
    [projectPath],
  );

  const shorterFor = useCallback(
    (nodeId: string) =>
      projectPath && language
        ? api.shorter(projectPath, language, nodeId).catch(() => [])
        : Promise.resolve([]),
    [projectPath, language],
  );

  const loadProject = useCallback(
    async (path: string) => {
      const summary = await run("open", () => api.openProject(path));
      if (!summary) return;
      const first = summary.targets.find((t) => t.enabled)?.tag ?? "";
      setProject(summary);
      setLanguage(first);
      setCapabilities(await api.capabilities(path).catch(() => []));
      setNodes(
        summary.needsExtract || !first ? [] : await api.nodes(path, first).catch(() => []),
      );
      setBuilds(first ? await api.builds(path, first).catch(() => []) : []);
      setDictionaries(await api.dictionaries(path).catch(() => []));
      setEngineState(await api.engine(path).catch(() => null));
      setAnalystState(await api.analyst(path).catch(() => null));
      setSuggestions(await api.suggestions(path).catch(() => []));
      setJournal(await api.journal(path, 12).catch(() => []));
      setEmulator(null);
      setRecents(await api.recentProjects().catch(() => []));
    },
    [run],
  );

  /** Re-reads everything the backend owns. Cheaper than tracking what each action invalidated. */
  const refresh = useCallback(async () => {
    if (!project || !language) return;
    const path = project.path;
    setProject(await api.projectSummary(path).catch(() => project));
    setNodes(await api.nodes(path, language).catch(() => []));
    setBuilds(await api.builds(path, language).catch(() => []));
  }, [project, language]);

  useEffect(() => {
    if (!isDesktop) return;
    api.recentProjects().then(setRecents).catch(() => setRecents([]));
    api.languages().then(setLanguages).catch(() => setLanguages([]));
    api.dictionaries(null).then(setDictionaries).catch(() => setDictionaries([]));
  }, []);

  // Styles are per language, so they follow the selected target rather than the project.
  useEffect(() => {
    if (!isDesktop || !language) return;
    api.styles(language).then(setStyles).catch(() => setStyles([]));
  }, [language]);

  // Switching language reloads only what belongs to it.
  useEffect(() => {
    if (!project || !language || project.needsExtract) return;
    let cancelled = false;
    const path = project.path;
    Promise.all([api.nodes(path, language), api.builds(path, language)])
      .then(([n, b]) => {
        if (cancelled) return;
        setNodes(n);
        setBuilds(b);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [language, project?.path, project?.needsExtract]);

  async function doImport() {
    const jar = await pickJar();
    if (!jar) return;
    const into = await pickFolder("Chọn thư mục chứa dự án");
    if (!into) return;
    const summary = await run("import", () => api.importJar(jar, into, null, []));
    if (summary) {
      await loadProject(summary.path);
      say(`Đã nhập ${summary.name}`);
    }
  }

  /// A PC game is installed as a folder, not shipped as a file. The two numbers reported back are
  /// the point: how big the game is, and how little of it needed reading.
  async function doImportFolder() {
    const game = await pickFolder("Chọn thư mục game đã cài");
    if (!game) return;
    const into = await pickFolder("Chọn thư mục chứa dự án");
    if (!into) return;
    const found = await run("import", () => api.importTree(game, into, null, []));
    if (found) {
      await loadProject(found.project.path);
      const skipped = found.skipped.length
        ? `; ${found.skipped.length} file bị bỏ qua, xem tab Tổng quan`
        : "";
      say(
        `${found.scanned.toLocaleString("vi-VN")} file, đọc ${found.read}${skipped}`,
      );
    }
  }

  async function doOpen() {
    const dir = await pickFolder("Chọn thư mục dự án");
    if (dir) await loadProject(dir);
  }

  async function doExportBuild() {
    if (!project || !target?.outputPath) return;
    const suggested = target.outputPath.split(/[\\/]/).pop() ?? `${project.name}.jar`;
    // The build keeps the kind it came in as, so the save dialog has to as well: offering to
    // write an Android package as a .jar renames a file nothing will then open.
    const extension = suggested.includes(".") ? suggested.split(".").pop()! : "jar";
    const destination = await pickSavePath(
      "Xuất file game đã Việt hoá",
      suggested,
      project.package.label,
      [extension],
    );
    if (!destination) return;
    // The save dialog already asked about overwriting, so passing true here does not skip a
    // confirmation - it avoids asking the same question twice.
    const written = await run("export", () =>
      api.exportBuild(project.path, language, destination, true),
    );
    if (written) say(`Đã xuất ra ${written}`);
  }

  async function doExportCsv() {
    if (!project) return;
    const destination = await pickSavePath(
      "Xuất bản dịch ra CSV",
      `${project.name}-${language}.csv`,
      "CSV",
      ["csv"],
    );
    if (!destination) return;
    const rows = await run("export", () =>
      api.exportTranslations(project.path, language, destination, false),
    );
    if (rows !== null) say(`Đã xuất ${rows} dòng ra ${destination}`);
  }

  async function doImportCsv() {
    if (!project) return;
    const source = await pickFile("Chọn file CSV người dịch gửi về", "CSV", ["csv"]);
    if (!source) return;
    const report = await run("import", () =>
      api.importTranslations(project.path, language, source),
    );
    if (!report) return;
    await refresh();
    const notes = [`${report.applied} bản dịch được áp`];
    if (report.unchanged) notes.push(`${report.unchanged} không đổi`);
    if (report.unknown) notes.push(`${report.unknown} dòng không khớp chuỗi nào`);
    if (report.malformed.length) notes.push(`${report.malformed.length} dòng hỏng`);
    say(notes.join(" · "), report.unknown > 0 || report.malformed.length > 0);
  }

  async function doImportDictionary() {
    if (!project) return;
    const source = await pickFile("Chọn gói từ điển (JSON)", "Dictionary pack", ["json"]);
    if (!source) return;
    const count = await run("import", () => api.importDictionary(project.path, source));
    if (count !== null) {
      setDictionaries(await api.dictionaries(project.path).catch(() => dictionaries));
      say(`Đã thêm gói từ điển ${count} mục`);
    }
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
            <p style={{ fontFamily: "var(--mono)", fontSize: 12 }}>cargo run -p tjlocalizer-desktop</p>
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
            <span className="pill">{project.sourceLanguageName}</span>
            <span style={{ color: "var(--text-faint)" }}>→</span>
            <select
              value={language}
              onChange={(e) => setLanguage(e.target.value)}
              style={{ width: "auto", padding: "3px 8px" }}
            >
              {project.targets.map((t) => (
                <option key={t.tag} value={t.tag}>
                  {t.name} · {t.approvedCount}/{project.translatableCount}
                </option>
              ))}
            </select>
            {target && <span className="pill">{target.styleProfile}</span>}
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
                title={r.error ?? r.path}
                onClick={() => loadProject(r.path)}
              >
                <div className="n">
                  {r.summary?.name ?? r.path.split(/[\\/]/).pop()}
                  {r.error && <span className="pill bad" style={{ marginLeft: 6 }}>lỗi</span>}
                </div>
                <div className="m">
                  {r.summary
                    ? `${r.summary.targets.map((t) => t.tag).join(", ")} · ${r.summary.translatableCount} chuỗi`
                    : r.error}
                </div>
              </button>
            ))}
          </div>
          <div className="foot">
            <button className="primary" disabled={busy !== null} onClick={doImport}>
              Nhập file game…
            </button>
            <button disabled={busy !== null} onClick={doImportFolder}>
              Nhập thư mục game…
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
                  Nhập một file game (.jar, .apk, .ipa, .zip), hoặc trỏ thẳng vào thư mục một game PC
                  đã cài. Bản gốc được băm và giữ nguyên vẹn; mọi thứ khác đều dựng lại được từ nó.
                </p>
                <div className="row" style={{ justifyContent: "center" }}>
                  <button className="primary" onClick={doImport}>
                    Nhập file game…
                  </button>
                  <button onClick={doImportFolder}>Nhập thư mục game…</button>
                </div>
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
                  className={tab === "font" ? "tab on" : "tab"}
                  onClick={() => setTab("font")}
                >
                  Font
                </button>
                <button
                  className={tab === "assets" ? "tab on" : "tab"}
                  onClick={() => setTab("assets")}
                >
                  Ảnh
                </button>
                <button
                  className={tab === "build" ? "tab on" : "tab"}
                  onClick={() => setTab("build")}
                >
                  Đóng gói {(target?.buildCount ?? 0) > 0 && `(${target?.buildCount})`}
                </button>
              </div>

              <div className="view">
                {tab === "overview" && (
                  <OverviewView
                    project={project}
                    language={language}
                    capabilities={capabilities}
                    languages={languages}
                    styles={styles}
                    dictionaries={dictionaries}
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
                      const n = await run("suggest", () =>
                        api.suggestAll(project.path, language, 0.75),
                      );
                      if (n !== null) {
                        await refresh();
                        say(`${n} gợi ý`);
                      }
                    }}
                    onApplySafe={async () => {
                      const n = await run("apply", () => api.applySafe(project.path, language));
                      if (n !== null) {
                        await refresh();
                        say(`Đã duyệt ${n} bản dịch chắc chắn`);
                      }
                    }}
                    onLearn={async () => {
                      const n = await run("learn", () => api.learn(project.path, language));
                      if (n !== null) say(`Bộ nhớ dịch có ${n} mục`);
                    }}
                    onSetBranding={async (enabled) => {
                      const s = await run("save", () => api.setBranding(project.path, enabled));
                      if (s) setProject(s);
                    }}
                    onSetSourceLanguage={async (tag) => {
                      const s = await run("save", () => api.setSourceLanguage(project.path, tag));
                      if (s) setProject(s);
                    }}
                    onSetStyle={async (id) => {
                      const s = await run("save", () =>
                        api.setStyle(project.path, language, id),
                      );
                      if (s) setProject(s);
                    }}
                    onAddTarget={async (tag) => {
                      const s = await run("save", () => api.addTarget(project.path, tag, null));
                      if (s) {
                        setProject(s);
                        setLanguage(tag);
                        say(`Đã thêm ${tag}`);
                      }
                    }}
                    onRemoveTarget={async (tag) => {
                      const s = await run("save", () => api.removeTarget(project.path, tag));
                      if (s) {
                        setProject(s);
                        if (language === tag) {
                          setLanguage(s.targets.find((t) => t.enabled)?.tag ?? "");
                        }
                        say("Đã bỏ khỏi dự án; bản dịch vẫn giữ trên đĩa");
                      }
                    }}
                    onImportDictionary={doImportDictionary}
                    engine={engine}
                    onSaveEngine={async (kind, endpoint, model, enabled) => {
                      const e = await run("save", () =>
                        api.setEngine(project.path, kind, endpoint, model, enabled),
                      );
                      if (e) {
                        setEngineState(e);
                        say(
                          e.enabled
                            ? "Máy dịch đã bật — chữ trong game sẽ được gửi tới dịch vụ này"
                            : "Máy dịch đã tắt",
                          e.enabled,
                        );
                      }
                    }}
                    onSaveEngineKey={async (key) => {
                      const e = await run("save", () => api.setEngineKey(project.path, key));
                      if (e) {
                        setEngineState(e);
                        say("Đã lưu khoá, ngoài thư mục dự án");
                      }
                    }}
                    onPreviewEngine={(text) =>
                      api.enginePreview(project.path, language, text).catch(() => null)
                    }
                    analyst={analyst}
                    suggestions={suggestions}
                    onSaveAnalyst={async (model, enabled) => {
                      const a = await run("save", () =>
                        api.setAnalyst(project.path, model, enabled),
                      );
                      if (a) {
                        setAnalystState(a);
                        say(
                          a.enabled
                            ? "Đã bật — tên file sẽ được gửi khi anh bấm quét"
                            : "Đã tắt — không có gì rời khỏi máy",
                          a.enabled,
                        );
                      }
                    }}
                    onSaveAnalystKey={async (key) => {
                      const a = await run("save", () => api.setAnalystKey(project.path, key));
                      if (a) {
                        setAnalystState(a);
                        say("Đã lưu khoá, ngoài thư mục dự án");
                      }
                    }}
                    onPreviewScan={() => api.scanPreview(project.path).catch(() => null)}
                    onScan={async () => {
                      const found = await run("scan", () => api.scan(project.path));
                      if (found) {
                        setSuggestions(found);
                        say(`${found.length} gợi ý — phỏng đoán, không đổi gì phần lõi đã xác định`);
                      }
                    }}
                    journal={journal}
                    onNote={async (text) => {
                      const updated = await run("note", () => api.addNote(project.path, text));
                      if (updated) {
                        setJournal(updated);
                        say("Đã ghi vào nhật ký");
                      }
                    }}
                    onInspect={(entry) =>
                      api.inspectEntry(project.path, entry).catch((e) => {
                        say(String(e), true);
                        return null;
                      })
                    }
                  />
                )}

                {tab === "text" && (
                  <TextView
                    nodes={nodes}
                    onGloss={glossFor}
                    onEngine={engine?.enabled && engine.hasKey ? engineFor : null}
                    onSetTranslation={async (nodeId, target) => {
                      await run("save", () =>
                        api.setTranslation(project.path, language, nodeId, target),
                      );
                      await refresh();
                    }}
                    onRender={renderWithGameFont}
                    onShorter={shorterFor}
                    onExport={doExportCsv}
                    onImport={doImportCsv}
                    reviewNotes={reviewNotes}
                    onReview={
                      analyst?.enabled && analyst.hasKey
                        ? async () => {
                            const notes = await run("review", () =>
                              api.reviewLanguage(project.path, language, 100),
                            );
                            if (!notes) return;
                            const byNode: Record<string, ReviewNoteView[]> = {};
                            for (const note of notes) {
                              (byNode[note.nodeId] ??= []).push(note);
                            }
                            setReviewNotes(byNode);
                            say(
                              notes.length === 0
                                ? "Không có gì bị nêu — với các dòng đã duyệt"
                                : `${notes.length} ghi chú. Là ghi chú, không sửa gì.`,
                            );
                          }
                        : null
                    }
                  />
                )}

                {tab === "font" && <FontView path={project.path} say={say} />}

                {tab === "assets" && <AssetsView path={project.path} say={say} />}

                {tab === "build" && (
                  <BuildsView
                    project={project}
                    language={language}
                    say={say}
                    builds={builds}
                    outputPath={target?.outputPath ?? null}
                    busy={busy}
                    emulator={emulator}
                    onFindEmulators={async () => {
                      const found = await run("emulator", () => api.findEmulators(project.path));
                      if (found) {
                        setEmulator(found);
                        say(
                          found.found.length === 0
                            ? "Không tìm thấy giả lập nào — xem danh sách chỗ đã dò"
                            : `Tìm thấy ${found.found.length} giả lập`,
                          found.found.length === 0,
                        );
                      }
                    }}
                    onUseEmulator={async (emulatorPath) => {
                      const found = await run("emulator", () =>
                        api.useEmulator(project.path, emulatorPath),
                      );
                      if (found) {
                        setEmulator(found);
                        say("Đã ghi lại giả lập cho dự án này");
                      }
                    }}
                    onPlay={async () => {
                      const outcome = await run("emulator", () =>
                        api.play(project.path, language),
                      );
                      if (outcome) say(outcome);
                      setJournal(await api.journal(project.path, 12).catch(() => journal));
                    }}
                    onApplyPatch={
                      project.package.label === "directory"
                        ? async () => {
                            const game = await pickFolder("Chọn thư mục game để áp vá");
                            if (!game) return;
                            const plan = await run("apply", () =>
                              api.planPatch(project.path, language, game),
                            );
                            if (!plan) return;
                            if (!plan.applicable) {
                              // Named, not summarised: "the patch does not fit" without saying
                              // which file is a message nobody can act on.
                              const why = plan.mismatched
                                .map((m) => `  ${m.path} — ${m.reason}`)
                                .join("\n");
                              say(`Gói vá không khớp bản game này:\n${why}`, true);
                              return;
                            }
                            // The list before the act. Writing into somebody's game directory is
                            // the most destructive thing this tool does.
                            const ok = window.confirm(
                              `Ghi đè ${plan.ready.length} file trong ${game}:\n\n` +
                                plan.ready.map((f) => `  ${f}`).join("\n") +
                                "\n\nBản cũ được giữ trong builds/ và phục hồi được.",
                            );
                            if (!ok) return;
                            const written = await run("apply", () =>
                              api.applyPatch(project.path, language, game),
                            );
                            if (written) {
                              say(`Đã ghi ${written.length} file; bản cũ giữ trong builds/`);
                            }
                          }
                        : null
                    }
                    onBuild={async () => {
                      const b = await run("build", () => api.build(project.path, language));
                      if (b) {
                        await refresh();
                        say(
                          b.ok ? `Build ${b.revision} đạt kiểm tra` : `Build ${b.revision} không đạt`,
                          !b.ok,
                        );
                      }
                    }}
                    onBuildAll={async () => {
                      const all = await run("build", () => api.buildAll(project.path));
                      if (all) {
                        await refresh();
                        const bad = all.filter((b) => !b.ok);
                        say(
                          bad.length === 0
                            ? `${all.length} ngôn ngữ đều đạt kiểm tra`
                            : `${bad.length}/${all.length} ngôn ngữ không đạt: ${bad
                                .map((b) => b.language)
                                .join(", ")}`,
                          bad.length > 0,
                        );
                      }
                    }}
                    onRollback={async (revision) => {
                      const b = await run("rollback", () =>
                        api.rollback(project.path, language, revision),
                      );
                      if (b) {
                        await refresh();
                        say(`Đã khôi phục bản build ${b.revision}`);
                      }
                    }}
                    onExport={doExportBuild}
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
              {target?.approvedCount ?? 0}/{project.translatableCount} đã duyệt
            </span>
            <span className="sep">·</span>
            <span>{project.targets.length} ngôn ngữ</span>
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
