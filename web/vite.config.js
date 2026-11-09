import { defineConfig } from "vite";
import preact from "@preact/preset-vite";
import { readFileSync, writeFileSync } from "node:fs";
import { createHash } from "node:crypto";
import { resolve } from "node:path";
let output;
export default defineConfig({
  plugins: [
    preact({ devToolsEnabled: false }),
    {
      name: "kanlo-cache-version",
      configResolved(config) {
        output = resolve(config.root, config.build.outDir);
      },
      closeBundle() {
        const hash = createHash("sha256");
        for (const name of ["index.html", "app.js", "style.css"])
          hash.update(readFileSync(resolve(output, name)));
        const path = resolve(output, "sw.js");
        writeFileSync(
          path,
          readFileSync(path, "utf8").replace(
            "__KANLO_ASSET_VERSION__",
            hash.digest("hex").slice(0, 16),
          ),
        );
      },
    },
  ],
  build: {
    outDir: "../crates/kanlo-api/assets",
    emptyOutDir: true,
    target: "es2022",
    rolldownOptions: {
      output: {
        entryFileNames: "app.js",
        chunkFileNames: "chunk-[hash].js",
        assetFileNames: "style[extname]",
      },
    },
  },
});
