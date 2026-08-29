import { useEffect, useState } from "react";
import { api } from "./api";
import type { PluginsView } from "./types";

interface Props {
  path: string;
}

/**
 * Adapters written as data.
 *
 * The core knows formats, never games, and that is what lets it open a game nobody here has seen.
 * It is also its limit: a game keeping its text in a file of a shape no detector recognises is
 * invisible until somebody says so. A plugin says so - in JSON, as data, never as code, because a
 * plugin arrives by the same route as the game does and "open this game" must never mean "run this
 * stranger's program".
 *
 * Shown only when the project has one. A panel explaining a mechanism nobody is using is noise.
 */
export function PluginsCard({ path }: Props) {
  const [plugins, setPlugins] = useState<PluginsView | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .plugins(path)
      .then((found) => {
        if (alive) setPlugins(found);
      })
      .catch(() => {
        if (alive) setPlugins(null);
      });
    return () => {
      alive = false;
    };
  }, [path]);

  if (!plugins || (plugins.loaded.length === 0 && plugins.broken.length === 0)) return null;

  return (
    <div className="card">
      <h3>Plugin</h3>
      <div className="sub">
        Mỗi plugin là một file JSON trong thư mục <code>plugins/</code> của dự án, nói cho công cụ
        biết game này giấu chữ ở đâu. Nó là <b>dữ liệu, không phải mã</b> — không có gì trong đó
        được chạy, nên một plugin không làm được gì mà công cụ vốn đã không làm được với mọi file
        game.
      </div>

      {plugins.broken.map((broken) => (
        <div key={broken.path} className="row" style={{ gap: 8, marginTop: 6 }}>
          <span className="pill bad">không đọc được</span>
          <span style={{ fontSize: 12 }}>
            {broken.path} — {broken.reason}
          </span>
        </div>
      ))}

      {plugins.loaded.map((plugin) => (
        <div key={plugin.id} style={{ marginTop: 12 }}>
          <div className="row" style={{ gap: 8 }}>
            <b style={{ fontFamily: "var(--mono)", fontSize: 12.5 }}>{plugin.id}</b>
            {plugin.author && (
              <span style={{ color: "var(--text-faint)", fontSize: 11.5 }}>
                của {plugin.author}
              </span>
            )}
          </div>
          {plugin.description && (
            <div style={{ color: "var(--text-dim)", fontSize: 12, marginTop: 2 }}>
              {plugin.description}
            </div>
          )}
          {plugin.problems.map((problem) => (
            <div key={problem} style={{ color: "var(--bad)", fontSize: 11.5, marginTop: 3 }}>
              · hỏng: {problem}
            </div>
          ))}
          {plugin.capabilities.map((claim) => (
            <div
              key={`c-${claim.what}`}
              style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}
            >
              · nhận dạng <b>{claim.what}</b> ({claim.detail}) —{" "}
              {claim.matches > 0 ? "khớp game này" : "không khớp game này"}
            </div>
          ))}
          {plugin.resources.map((claim) => (
            <div
              key={`r-${claim.what}`}
              style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}
            >
              · đọc <code>{claim.what}</code> như {claim.detail} — {claim.matches} file trong game
            </div>
          ))}
          {plugin.fonts.map((claim) => (
            <div
              key={`f-${claim.what}`}
              style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}
            >
              · font <code>{claim.what}</code> ({claim.detail}) — {claim.matches} file trong game
            </div>
          ))}
          {plugin.rules.map((rule) => (
            <div key={rule} style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}>
              · luật <code>{rule}</code> — đang tắt, phải tự bật ở tab Font
            </div>
          ))}
          {plugin.dictionaryEntries > 0 && (
            <div style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 3 }}>
              · {plugin.dictionaryEntries} từ thêm vào từ điển
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
