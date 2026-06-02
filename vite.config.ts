import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 期望固定端口；前端构建产物输出到 dist/（tauri.conf.json 的 frontendDist）
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
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
    outDir: "dist",
    emptyOutDir: true,
  },
});
