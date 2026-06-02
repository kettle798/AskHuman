import type { ThemeMode } from "./types";

/// 套用主题：显式 light/dark 加类名，system 交给 prefers-color-scheme 兜底。
export function applyTheme(theme: ThemeMode): void {
  const root = document.documentElement;
  root.classList.remove("theme-light", "theme-dark");
  if (theme === "light") root.classList.add("theme-light");
  else if (theme === "dark") root.classList.add("theme-dark");
}

/// 把文件/Blob 读成 base64 data URL 字符串。
export function fileToDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = () => reject(reader.error);
    reader.readAsDataURL(file);
  });
}
