import { build, transform } from "esbuild";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

// wasm-bindgen's glue creates one WebAssembly instance and keeps it for good, but an
// instance that trapped must not be used again. Importing the glue with "?factory"
// gives a function that evaluates a fresh copy of it, with its own instance, per call.
const glueFactory = {
  name: "glue-factory",
  setup(plugin) {
    plugin.onResolve({ filter: /\?factory$/ }, (args) => ({
      path: path.resolve(args.resolveDir, args.path.slice(0, -"?factory".length)),
      namespace: "glue-factory",
    }));
    plugin.onLoad({ filter: /.*/, namespace: "glue-factory" }, async (args) => {
      const glue = await transform(await readFile(args.path, "utf8"), {
        format: "cjs",
        define: { "import.meta.url": '""' },
      });
      return {
        contents: `export default function () {\nconst module = { exports: {} };\n${glue.code}\nreturn module.exports;\n}\n`,
        watchFiles: [args.path],
      };
    });
  },
};

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
  plugins: [glueFactory],
  logLevel: "info",
});

const script = (await readFile("dist/ui.js", "utf8")).replaceAll("</script", "<\\/script");
await writeFile("dist/ui.html", `<!doctype html><meta charset="utf-8"><body><script>${script}</script>\n`);
