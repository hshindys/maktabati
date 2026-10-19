import { fileURLToPath, URL } from "node:url";

import tailwindcss from "@tailwindcss/vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import { defineConfig } from "vitest/config";

// Vite runs inside the Tauri dev workflow; `TAURI_DEV_HOST` is set when the CLI needs the dev
// server reachable from another device on the LAN.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [svelte(), tailwindcss()],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Vite clears the screen on every rebuild, which is hostile inside `tauri dev`.
  clearScreen: false,

  server: {
    port: 1420,
    strictPort: true,
    host: host ?? false,
    hmr: host
      ? { protocol: "ws", host, port: 1421 }
      : undefined,
    watch: {
      // Rust rebuilds are owned by the Tauri CLI; watching them here doubles the work.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },

  build: {
    // Matches the WebKit2GTK / WebView2 baselines Tauri 2 targets.
    target: "es2022",
    // Source maps are omitted in release to keep the bundle (and the shipped IP surface) small;
    // the dev server still has them.
    sourcemap: false,
    rollupOptions: {
      output: {
        // Keep the font payloads out of the main chunk so the shell paints before they land.
        // Rolldown (Vite 8's bundler) only accepts the function form — the object form is
        // Rollup-specific and is rejected as an invalid option type.
        manualChunks(id: string): string | undefined {
          return id.includes("@fontsource/") ? "fonts" : undefined;
        },
      },
    },
  },

  test: {
    environment: "node",
    include: ["src/**/*.{test,spec}.ts"],
    passWithNoTests: false,
  },
});
