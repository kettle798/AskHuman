import { defineConfig, type PluginOption } from "vite";
import vue from "@vitejs/plugin-vue";
import { visualizer } from "rollup-plugin-visualizer";

// Tauri 期望固定端口；前端构建产物输出到 dist/（tauri.conf.json 的 frontendDist）
const host = process.env.TAURI_DEV_HOST;

// 按需分析 bundle 构成：ANALYZE=1 pnpm build → 仓库根 bundle-stats.html（gitignored）。
const analyze = process.env.ANALYZE
  ? [visualizer({ filename: "bundle-stats.html", gzipSize: true }) as PluginOption]
  : [];

export default defineConfig({
  plugins: [vue(), ...analyze],
  // 前端源码与入口 index.html 都在 src/，故以 src 为 Vite 根目录。
  root: "src",
  // Tauri CLI 通过 env 注入，避免 vite 清屏吞掉 Rust 日志
  clearScreen: false,
  server: {
    port: 5180,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 5181 }
      : undefined,
    watch: {
      // 不监听 Rust 与 Swift 源码，减少无谓重载
      ignored: ["**/src-tauri/**", "**/Sources/**", "**/target/**"],
    },
  },
  build: {
    target: "es2021",
    // 输出到仓库根的 dist/（tauri.conf.json 的 frontendDist=../dist）。
    outDir: "../dist",
    emptyOutDir: true,
  },
});
