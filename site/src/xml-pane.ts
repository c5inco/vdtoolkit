// The XML panel at the bottom of the page, shared by every tool.

import type { Analysis } from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const el = {
  heading: $("xml-heading"),
  facts: $("facts"),
  diagnostics: $("diagnostics"),
  xml: $("xml"),
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

export function showXml(content: XmlPaneContent): void {
  current = content;
  const { xml, analysis } = content;
  el.heading.textContent = content.heading;
  el.xml.textContent = xml ?? content.placeholder ?? "";
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

export function initXmlPane(): void {
  el.copy.addEventListener("click", async () => {
    if (!current?.xml) return;
    await navigator.clipboard.writeText(current.xml);
    el.copy.textContent = "Copied";
    setTimeout(() => (el.copy.textContent = "Copy XML"), 1500);
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
