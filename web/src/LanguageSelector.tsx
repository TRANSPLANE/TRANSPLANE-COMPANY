import { setLocale, useLocale, type Locale } from "./locale";
import { copy } from "./copy";

export function LanguageSelector() {
  const locale = useLocale();
  return (
    <label className="language-selector">
      <span className="sr-only">{copy.language}</span>
      <select
        value={locale}
        onChange={(event) => setLocale(event.target.value as Locale)}
        aria-label={copy.language}
      >
        <option value="zh-CN" lang="zh-CN">
          简体中文
        </option>
        <option value="en" lang="en">
          English
        </option>
      </select>
    </label>
  );
}
