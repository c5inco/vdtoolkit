import { build } from "esbuild";
import { copyFile, mkdir } from "node:fs/promises";

await mkdir("dist", { recursive: true });

await build({
  entryPoints: ["src/main.ts"],
  bundle: true,
  outfile: "dist/main.js",
  platform: "browser",
  target: "es2022",
  format: "esm",
  // canvaskit.js only reaches for these when it runs under Node.
  external: ["fs", "path"],
  loader: { ".svg": "text", ".xml": "text", ".wasm": "file" },
  assetNames: "[name]-[hash]",
  minify: true,
  logLevel: "info",
});

await copyFile("index.html", "dist/index.html");
await copyFile("src/styles.css", "dist/styles.css");
await copyFile("node_modules/canvaskit-wasm/bin/canvaskit.wasm", "dist/canvaskit.wasm");
