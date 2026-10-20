import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vite";

// Vite serves the webview UI from ui/ while developing,
// and builds it into dist/ for the packaged app.
export default defineConfig({
  root: "ui",
  plugins: [svelte()],
  // Keep Rust compiler errors visible in the terminal.
  clearScreen: false,
  server: {
    // Tauri expects this exact port (devUrl in src-tauri/tauri.conf.json).
    port: 1420,
    strictPort: true,
  },
  build: {
    outDir: "../dist",
    emptyOutDir: true,
  },
});
