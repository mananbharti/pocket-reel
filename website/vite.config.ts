import { defineConfig } from "vite";

export default defineConfig({
  base: "/pocket-reel/",
  build: { target: "es2020", outDir: "dist", assetsInlineLimit: 0, chunkSizeWarningLimit: 800 },
});
