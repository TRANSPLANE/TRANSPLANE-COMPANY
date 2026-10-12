import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { localized } from "./locale";
import "./markdown.css";

const text = localized(
  { image: "查看图片", table: "表格" },
  { image: "View image", table: "Table" },
);

/** Document links are explicit navigation; rendering never fetches remote assets. */
export function safeMarkdownUrl(value: string): string | undefined {
  if (/[\u0000-\u0020\u007f]/.test(value)) return undefined;
  try {
    const url = new URL(value);
    return ["https:", "http:", "mailto:"].includes(url.protocol)
      ? url.href
      : undefined;
  } catch {
    return undefined;
  }
}

export function Markdown({
  source,
  className = "",
}: {
  source: string;
  className?: string;
}) {
  return (
    <div className={`markdown-body ${className}`}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        skipHtml
        urlTransform={(url) => safeMarkdownUrl(url) || ""}
        components={{
          a: ({ href, children }) => {
            const safe = href && safeMarkdownUrl(href);
            return safe ? (
              <a href={safe} target="_blank" rel="noopener noreferrer">
                {children}
              </a>
            ) : (
              <span>{children}</span>
            );
          },
          img: ({ src, alt }) => {
            const safe = typeof src === "string" && safeMarkdownUrl(src);
            return safe ? (
              <a href={safe} target="_blank" rel="noopener noreferrer">
                {alt || text.image}
              </a>
            ) : (
              <span>{alt}</span>
            );
          },
          table: ({ children }) => (
            <div className="markdown-table" tabIndex={0} role="region" aria-label={text.table}>
              <table>{children}</table>
            </div>
          ),
        }}
      >
        {source}
      </ReactMarkdown>
    </div>
  );
}
