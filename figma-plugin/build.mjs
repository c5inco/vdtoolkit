import { build } from "esbuild";
import { mkdir, readFile, writeFile } from "node:fs/promises";

await mkdir("dist", { recursive: true });

await build({
  entryPoints: ["src/code.ts"],
  bundle: true,
  outfile: "dist/code.js",
  platform: "browser",
  target: "es2022",
  format: "iife",
  logLevel: "info",
});

await build({
  entryPoints: ["src/ui.ts"],
  bundle: true,
  outfile: "dist/ui.js",
  platform: "browser",
  target: "es2022",
  format: "iife",
  define: { "import.meta.url": '""' },
  loader: { ".wasm": "binary" },
  logLevel: "info",
});

const script = (await readFile("dist/ui.js", "utf8")).replaceAll("</script", "<\\/script");
await writeFile("dist/ui.html", `<!doctype html><meta charset="utf-8"><body><script>${script}</script>\n`);
