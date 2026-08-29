import { useEffect, useState } from "react";
import { api } from "./api";
import type { ContextView } from "./types";

interface Props {
  path: string;
  /** Re-read when extraction has run again. */
  revision: number;
}

/**
 * Who the game's lines belong to.
 *
 * A string on its own often settles nothing: "Yes" is a button by its length and a reply by its
 * company. So the lines around it are read too - the keys beside it, the strings either side of it
 * in the same class, and whether a line names its speaker.
 *
 * The stance shown here is a leaning, not a setting. Nothing applies it; Vietnamese has no neutral
 * second person, and choosing between "ngươi" and "bạn" for a character is a decision a person
 * makes once and the whole game then follows.
 */
export function CastCard({ path, revision }: Props) {
  const [context, setContext] = useState<ContextView | null>(null);

  useEffect(() => {
    let alive = true;
    api
      .context(path)
      .then((found) => {
        if (alive) setContext(found);
      })
      .catch(() => {
        if (alive) setContext(null);
      });
    return () => {
      alive = false;
    };
  }, [path, revision]);

  if (!context || (context.cast.length === 0 && context.readings === 0)) return null;

  return (
    <div className="card">
      <h3>Nhân vật và ngữ cảnh</h3>
      <div className="sub">
        {context.readings} dòng được đọc thêm từ những dòng xung quanh nó — khoá bên cạnh, chuỗi
        đứng trước và sau trong cùng một class, hoặc chính dòng đó có tên người nói.
      </div>

      {context.cast.length === 0 ? (
        <div style={{ color: "var(--text-faint)", fontSize: 12 }}>
          Không dòng nào ghi tên người nói. Game này viết hội thoại theo cách khác.
        </div>
      ) : (
        context.cast.map((character) => (
          <div key={character.name} style={{ marginTop: 8 }}>
            <div className="row" style={{ gap: 8 }}>
              <b style={{ fontSize: 12.5 }}>{character.name}</b>
              <span style={{ color: "var(--text-faint)", fontSize: 11.5 }}>
                {character.lines} dòng · {character.appearsIn.join(", ")}
              </span>
            </div>
            {character.beside.length > 0 && (
              <div style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 2 }}>
                cùng file với {character.beside.join(", ")}
              </div>
            )}
            {character.stance && (
              <div style={{ color: "var(--text-dim)", fontSize: 11.5, marginTop: 2 }}>
                nghe có vẻ {character.stance} — vì có chữ {character.because.join(", ")} — công cụ
                không tự áp dụng, bạn tự quyết
              </div>
            )}
          </div>
        ))
      )}
    </div>
  );
}
