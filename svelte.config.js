import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import("@sveltejs/vite-plugin-svelte").SvelteConfig} */
export default {
  // Scripts are plain TS (no experimental features), so the Vite preprocessor is enough.
  preprocess: vitePreprocess(),
  compilerOptions: {
    // Svelte 5 runes are the default; being explicit keeps the intent visible in reviews.
    runes: true,
  },
};
