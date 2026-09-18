// The XML panel at the bottom of the page, shared by every tool.

import type { Analysis } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const el = {
  heading: $("xml-heading"),
  facts: $("facts"),
  diagnostics: $("diagnostics"),
  xml: $("xml"),
  file: $("xml-file"),
  copy: $<HTMLButtonElement>("copy"),
  download: $<HTMLButtonElement>("download"),
};

export interface XmlPaneContent {
  heading: string;
  xml: string | null;
  /** Shown instead of XML when there is none. */
  placeholder?: string;
  analysis?: Analysis | null;
  extraFacts?: [string, string][];
  downloadName: string;
}

const COMPATIBILITY: Record<Analysis["compatibility"], string> = {
  exact: "exact",
  exact_with_normalization: "exact after normalizing",
  approximate: "approximate",
  unsupported: "unsupported",
};

const encoder = new TextEncoder();
let current: XmlPaneContent | null = null;
let generation = 0;

/** The pane belongs to whichever tool is on stage. */
export function placeXmlPane(tool: string): void {
  const slot = document.getElementById(tool === "icon" ? "icon-xml-slot" : "drawable-xml-slot");
  slot?.append(document.getElementById("xml-section")!);
}

export function showXml(content: XmlPaneContent): void {
  current = content;
  const gen = ++generation;
  const { xml, analysis } = content;
  el.heading.textContent = content.heading;
  el.file.textContent = content.downloadName;
  if (xml !== null) {
    el.xml.textContent = xml; // readable at once; highlighting lands behind it
    void highlight(xml).then((html) => {
      if (gen === generation) el.xml.innerHTML = html;
    });
  } else {
    el.xml.textContent = content.placeholder ?? "";
  }
  el.copy.disabled = el.download.disabled = xml === null;

  const facts: [string, string][] = [];
  if (analysis) {
    facts.push(
      ["Minimum API", analysis.minimum_api === null ? "none" : String(analysis.minimum_api)],
      ["Fidelity", COMPATIBILITY[analysis.compatibility]],
      ["Paths", String(analysis.metrics.paths)],
    );
  }
  if (xml) facts.push(["Size", `${encoder.encode(xml).length.toLocaleString()} bytes`]);
  facts.push(...(content.extraFacts ?? []));
  el.facts.replaceChildren(...facts.flatMap(([term, value]) => [
    Object.assign(document.createElement("dt"), { textContent: term }),
    Object.assign(document.createElement("dd"), { textContent: value }),
  ]));

  el.diagnostics.replaceChildren(
    ...(analysis?.diagnostics ?? []).map((diagnostic) => {
      const item = document.createElement("li");
      item.className = diagnostic.severity;
      item.append(
        Object.assign(document.createElement("span"), { className: "code", textContent: diagnostic.code }),
        diagnostic.message,
      );
      return item;
    }),
  );
}

/**
 * Highlight with gpu-lexer, Shu Ding's tiny WebGPU lexer: one model labels
 * every token plain/comment/string/number/keyword/type/function/constant/
 * operator, whatever the language. Without WebGPU — or if the model hiccups —
 * the hand-rolled quote-aware scanner below takes over. Either way, every hex
 * color literal gets an IDE-style chip of that color in front of it.
 */
async function highlight(xml: string): Promise<string> {
  const spans = await gpuSpans(xml);
  const html = spans ? spansToHtml(xml, spans) : highlightXml(xml);
  el.xml.dataset.engine = spans ? "gpu" : "classic";
  return html;
}

let lexer: { parse(code: string): Promise<SyntaxSpan[]> } | null | undefined;

interface SyntaxSpan {
  type: "plain" | "comment" | "string" | "number" | "keyword" | "type" | "function" | "constant" | "operator";
  start: number;
  end: number;
}

async function gpuSpans(xml: string): Promise<SyntaxSpan[] | null> {
  if (lexer === undefined) {
    lexer = "gpu" in navigator ? await import("gpu-lexer").catch(() => null) : null;
  }
  if (!lexer) return null;
  try {
    return await lexer.parse(xml);
  } catch {
    lexer = null; // WebGPU vanished or the model failed; do not ask again
    return null;
  }
}

/** gpu-lexer's classes, dressed in the page's code palette. */
const GPU_CLASS: Record<SyntaxSpan["type"], string | null> = {
  keyword: "x-tag",
  type: "x-attr",
  function: "x-attr",
  string: "x-value",
  constant: "x-value",
  number: "x-number",
  comment: "x-comment",
  operator: "x-bracket",
  plain: null,
};

function spansToHtml(xml: string, spans: SyntaxSpan[]): string {
  let out = "";
  let pos = 0;
  for (const span of spans) {
    const start = Math.max(span.start, pos);
    if (start > pos) out += escapeHtml(xml.slice(pos, start));
    const text = xml.slice(start, Math.max(span.end, start));
    const cls = GPU_CLASS[span.type];
    out += cls === null ? escapeHtml(text) : `<span class="${cls}">${withSwatches(text)}</span>`;
    pos = span.end;
  }
  return out + escapeHtml(xml.slice(pos));
}

/** Escape, then drop a color chip in front of every hex literal. */
function withSwatches(text: string): string {
  return escapeHtml(text).replace(/#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{8})\b/g, (hex) => `<i class="sw" style="--c:${hex}"></i>${hex}`);
}

/**
 * Tokenize VectorDrawable XML into highlighted HTML. Every attribute value
 * that is a hex color gets an IDE-style chip of that color in front of it.
 * Only runs on converter output, but escapes everything regardless.
 */
export function highlightXml(xml: string): string {
  let out = "";
  let i = 0;
  while (i < xml.length) {
    const lt = xml.indexOf("<", i);
    if (lt === -1) {
      out += escapeHtml(xml.slice(i));
      break;
    }
    out += escapeHtml(xml.slice(i, lt));
    if (xml.startsWith("<!--", lt)) {
      const end = xml.indexOf("-->", lt + 4);
      const stop = end === -1 ? xml.length : end + 3;
      out += `<span class="x-comment">${escapeHtml(xml.slice(lt, stop))}</span>`;
      i = stop;
    } else if (xml.startsWith("<?", lt)) {
      const end = xml.indexOf("?>", lt + 2);
      const stop = end === -1 ? xml.length : end + 2;
      out += `<span class="x-decl">${escapeHtml(xml.slice(lt, stop))}</span>`;
      i = stop;
    } else {
      const [html, next] = highlightTag(xml, lt);
      out += html;
      i = next;
    }
  }
  return out;
}

/** Highlight one tag starting at `<`, consuming attributes quote-aware so a `>` inside a value stays put. */
function highlightTag(xml: string, start: number): [string, number] {
  const nameMatch = /^(<\/?)([A-Za-z_][\w:.-]*)/.exec(xml.slice(start, start + 128));
  if (!nameMatch) return ["&lt;", start + 1];
  let html =
    `<span class="x-bracket">${escapeHtml(nameMatch[1])}</span>` +
    `<span class="x-tag">${escapeHtml(nameMatch[2])}</span>`;
  let i = start + nameMatch[0].length;
  while (i < xml.length) {
    if (xml[i] === ">") return [html + `<span class="x-bracket">&gt;</span>`, i + 1];
    if (xml[i] === "/" && xml[i + 1] === ">") return [html + `<span class="x-bracket">/&gt;</span>`, i + 2];
    const attr = /^([A-Za-z_][\w:.-]*)(\s*=\s*)("[^"]*"|'[^']*')/.exec(xml.slice(i));
    if (!attr) {
      html += escapeHtml(xml[i]); // whitespace between attributes
      i += 1;
      continue;
    }
    const [, name, eq, quoted] = attr;
    const raw = quoted.slice(1, -1);
    const swatch = HEX_COLOR.test(raw) ? `<i class="sw" style="--c:${raw}"></i>` : "";
    html +=
      `<span class="x-attr">${escapeHtml(name)}</span>${escapeHtml(eq)}` +
      `<span class="x-value">${quoted[0]}${swatch}${escapeHtml(raw)}${quoted[0]}</span>`;
    i += attr[0].length;
  }
  return [html, xml.length];
}

const HEX_COLOR = /^#(?:[0-9a-fA-F]{6}|[0-9a-fA-F]{8})$/;

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

const CHECK_ICON = `<svg viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M3 8.75 6.25 12 13 4.5"/></svg>`;

export function initXmlPane(): void {
  const copyIcon = el.copy.innerHTML;
  el.copy.addEventListener("click", async () => {
    if (!current?.xml) return;
    await navigator.clipboard.writeText(current.xml);
    el.copy.innerHTML = CHECK_ICON;
    el.copy.classList.add("copied");
    el.copy.setAttribute("aria-label", "Copied");
    setTimeout(() => {
      el.copy.innerHTML = copyIcon;
      el.copy.classList.remove("copied");
      el.copy.setAttribute("aria-label", "Copy XML");
    }, 1500);
  });
  el.download.addEventListener("click", () => {
    if (!current?.xml) return;
    downloadBlob(new Blob([current.xml], { type: "text/xml" }), current.downloadName);
  });
}

export function downloadBlob(blob: Blob, name: string): void {
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = name;
  link.click();
  URL.revokeObjectURL(link.href);
}

export function resourceName(fileName: string): string {
  return fileName.replace(/\.[^.]+$/, "").replace(/[^a-z0-9_]+/gi, "_").toLowerCase();
}
