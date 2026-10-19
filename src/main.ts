/**
 * Application entry point.
 *
 * Order matters: fonts and the token layer must be loaded before the first component mounts,
 * and `settings.ts` applies `data-theme` / `dir` at import time (see its module tail) so the
 * shell never paints with the wrong theme or the wrong direction.
 */

import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@fontsource/inter/700.css";

import "@fontsource/cairo/400.css";
import "@fontsource/cairo/500.css";
import "@fontsource/cairo/600.css";
import "@fontsource/cairo/700.css";

import "@fontsource/tajawal/400.css";
import "@fontsource/tajawal/500.css";

import "@fontsource/ibm-plex-sans-arabic/400.css";
import "@fontsource/ibm-plex-sans-arabic/500.css";
import "@fontsource/ibm-plex-sans-arabic/600.css";

import "@fontsource/amiri/400.css";
import "@fontsource/amiri/700.css";

import "./app.css";
import "./lib/settings";

import { mount } from "svelte";

import App from "./App.svelte";

const target = document.getElementById("app");
if (target === null) {
  throw new Error('#app is missing from index.html — the mount point was renamed without updating main.ts');
}

const app = mount(App, { target });

export default app;
