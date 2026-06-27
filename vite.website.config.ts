import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const websiteRoot = fileURLToPath(new URL("./website", import.meta.url));
const outputDirectory = fileURLToPath(new URL("./site-dist", import.meta.url));

export default defineConfig({
  root: websiteRoot,
  base: "./",
  build: {
    modulePreload: {
      polyfill: false
    },
    outDir: outputDirectory,
    emptyOutDir: true,
    rollupOptions: {
      input: {
        home: fileURLToPath(new URL("./website/index.html", import.meta.url)),
        security: fileURLToPath(new URL("./website/security.html", import.meta.url)),
        support: fileURLToPath(new URL("./website/support.html", import.meta.url))
      }
    }
  }
});
