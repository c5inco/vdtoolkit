import { matchesFilter } from "./export-review";
import type { BlockedRow, Issue, ReadyRow, ReviewRow } from "./export-review";
import type { SandboxMessage } from "./messages";
import { createZip } from "./zip";

type Post = (message: SandboxMessage) => void;

// What the user unchecked, kept across refreshes so re-checking layers doesn't undo their choices.
const unchecked = new Set<string>();

// The filter text, kept across refreshes for the same reason.
let filterQuery = "";

// Thumbnail object URLs from the current render, released when the list is redrawn.
let previewUrls: string[] = [];

const STYLES = `
  :root { color-scheme: light dark; }
  [hidden] { display: none !important; }
  body {
    margin: 0; height: 100vh; display: flex; flex-direction: column;
    font: 11px/16px Inter, system-ui, sans-serif;
    color: var(--figma-color-text, #1e1e1e); background: var(--figma-color-bg, #fff);
  }
  /* An explicit size stops grid rows stretching their checkbox (which draws the box centered)
     while the select-all checkbox keeps its natural width, leaving them 1–2px apart. */
  input[type="checkbox"] { margin: 0; width: 14px; height: 14px; justify-self: start; }
  input[type="checkbox"]:disabled { opacity: 0.4; cursor: not-allowed; }
  .status { padding: 16px; color: var(--figma-color-text-secondary, #757575); }
  header {
    display: flex; flex-direction: column; gap: 8px; padding: 12px 16px;
    border-bottom: 1px solid var(--figma-color-border, #e6e6e6);
  }
  .summary { margin: 0; }
  .filter {
    font: inherit; height: 28px; padding: 0 8px; border-radius: 5px; color: inherit;
    border: 1px solid var(--figma-color-border, #e6e6e6); background: var(--figma-color-bg, #fff);
  }
  .filter:focus {
    outline: none; border-color: var(--figma-color-border-selected, #0d99ff);
    box-shadow: inset 0 0 0 1px var(--figma-color-border-selected, #0d99ff);
  }
  .list { flex: 1; overflow-y: auto; padding-bottom: 8px; }
  .empty { margin: 0; padding: 16px; color: var(--figma-color-text-secondary, #757575); }
  /* Section headers stay pinned while their rows scroll, keeping select-all in reach. */
  h2 {
    display: flex; align-items: center; gap: 8px; margin: 0; padding: 12px 16px 4px;
    font-size: 11px; font-weight: 600;
    position: sticky; top: 0; z-index: 1; background: var(--figma-color-bg, #fff);
  }
  h2 label { display: flex; align-items: center; gap: 8px; cursor: pointer; }
  h2 .count { color: var(--figma-color-text-secondary, #757575); font-weight: 400; }
  .row {
    display: grid; grid-template-columns: 16px 32px 1fr auto; column-gap: 8px; align-items: center;
    padding: 6px 16px;
  }
  .row:hover { background: var(--figma-color-bg-hover, #f5f5f5); }
  /* A light checkerboard shows transparency and keeps dark icons visible in dark theme. */
  .thumb {
    width: 32px; height: 32px; border-radius: 4px; display: grid; place-items: center; overflow: hidden;
    background: repeating-conic-gradient(#e6e6e6 0 25%, #fff 0 50%) 0 0 / 8px 8px;
    box-shadow: inset 0 0 0 1px var(--figma-color-border, #e6e6e6);
  }
  .thumb img { width: 28px; height: 28px; object-fit: contain; }
  .row label { display: flex; flex-direction: column; min-width: 0; cursor: pointer; }
  .row.blocked label { cursor: default; }
  .name { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .meta { color: var(--figma-color-text-secondary, #757575); overflow-wrap: anywhere; }
  .file { font-family: ui-monospace, Menlo, monospace; font-size: 10px; }
  .size { display: flex; flex-wrap: wrap; column-gap: 6px; }
  .resized { color: var(--figma-color-text-warning, #b86200); }
  .issues { grid-column: 3 / 5; margin: 4px 0 0; padding: 0; list-style: none; }
  .issues li { margin-top: 2px; }
  .issues.warning { color: var(--figma-color-text-warning, #b86200); }
  .issues.error { color: var(--figma-color-text-danger, #dc3412); }
  .suggestion { color: var(--figma-color-text-secondary, #757575); }
  button {
    font: inherit; border-radius: 5px; padding: 0 12px; height: 28px; cursor: pointer;
    border: 1px solid var(--figma-color-border, #e6e6e6); background: transparent; color: inherit;
  }
  .show {
    width: 24px; height: 24px; padding: 0; display: grid; place-items: center; border: none;
    color: var(--figma-color-icon-secondary, #757575); opacity: 0;
  }
  /* Hidden until its row is hovered; keyboard focus reveals it too so it stays reachable. */
  .row:hover .show, .show:focus-visible { opacity: 1; }
  .show:hover { background: var(--figma-color-bg-pressed, #e6e6e6); color: var(--figma-color-icon, #1e1e1e); }
  footer {
    display: flex; justify-content: flex-end; gap: 8px; padding: 12px 16px;
    border-top: 1px solid var(--figma-color-border, #e6e6e6);
  }
  .primary {
    border-color: transparent; font-weight: 500;
    background: var(--figma-color-bg-brand, #0d99ff); color: var(--figma-color-text-onbrand, #fff);
  }
  .primary:disabled { opacity: 0.4; cursor: default; }
  .refresh { margin-right: auto; }
  .refresh:disabled { opacity: 0.6; cursor: default; }
`;

export function renderPreparing(): void {
  document.head.append(element("style", {}, STYLES));
  document.body.replaceChildren(element("p", { className: "status" }, "Checking selected layers…"));
}

export function renderExportDialog(rows: ReviewRow[], post: Post): void {
  for (const url of previewUrls) URL.revokeObjectURL(url);
  previewUrls = [];
  const ready = rows.filter((row): row is ReadyRow => row.status === "ready");
  const blocked = rows.filter((row): row is BlockedRow => row.status === "blocked");
  const isShown = (row: ReviewRow) => matchesFilter(row, filterQuery);

  const checkboxes = ready.map((row) => {
    const checkbox = element("input", { type: "checkbox", checked: !unchecked.has(row.nodeId), id: `row-${row.nodeId}` });
    checkbox.addEventListener("change", update);
    return checkbox;
  });
  // Select all, like Export, only acts on the rows the filter shows.
  const selectAll = element("input", { type: "checkbox", title: "Select all shown", id: "select-all" });
  selectAll.addEventListener("change", () => {
    ready.forEach((row, index) => {
      if (isShown(row)) checkboxes[index].checked = selectAll.checked;
    });
    update();
  });

  const exportButton = element("button", { className: "primary" });
  exportButton.addEventListener("click", () => {
    const chosen = ready.filter((row, index) => checkboxes[index].checked && isShown(row));
    if (chosen.length === 0) return;
    download(chosen);
    post({ type: "exported", count: chosen.length });
  });
  const cancelButton = element("button", {}, "Cancel");
  cancelButton.addEventListener("click", () => post({ type: "close" }));
  const refreshButton = element(
    "button",
    { className: "refresh", title: "Check the same layers again after editing them in Figma" },
    "Refresh",
  );
  refreshButton.addEventListener("click", () => {
    refreshButton.disabled = true;
    refreshButton.textContent = "Checking…";
    post({ type: "refresh" });
  });

  const filter = element("input", {
    type: "search",
    className: "filter",
    placeholder: "Filter by layer or file name",
    value: filterQuery,
  });
  filter.addEventListener("input", () => {
    filterQuery = filter.value;
    update();
  });

  const readyElements = ready.map((row, index) => readyRow(row, checkboxes[index], post));
  const blockedElements = blocked.map((row) => blockedRow(row, post));
  const readyCount = element("span", { className: "count" });
  const blockedCount = element("span", { className: "count" });
  const readyHeader = element(
    "h2",
    {},
    element("label", { htmlFor: selectAll.id }, selectAll, "Ready to export"),
    readyCount,
  );
  const blockedHeader = element("h2", {}, "Needs fixes in Figma", blockedCount);
  const empty = element("p", { className: "empty" }, "No layers match the filter.");
  const list = element(
    "section",
    { className: "list" },
    readyHeader,
    ...readyElements,
    blockedHeader,
    ...blockedElements,
    empty,
  );

  // Esc closes the dialog, except that it first clears a filter being typed in, as search boxes do.
  // Assigned rather than added, so a refresh doesn't stack a second handler.
  document.onkeydown = (event) => {
    if (event.key !== "Escape" || event.defaultPrevented) return;
    if (document.activeElement === filter && filter.value !== "") return;
    post({ type: "close" });
  };

  const filterWasFocused = document.activeElement?.classList.contains("filter") ?? false;
  const scrollTop = document.querySelector(".list")?.scrollTop ?? 0;
  document.body.replaceChildren(
    element("header", {}, element("p", { className: "summary" }, summary(ready.length, rows.length)), filter),
    list,
    element("footer", {}, refreshButton, cancelButton, exportButton),
  );
  list.scrollTop = scrollTop;
  if (filterWasFocused) filter.focus();
  update();

  function update() {
    ready.forEach((row, index) =>
      checkboxes[index].checked ? unchecked.delete(row.nodeId) : unchecked.add(row.nodeId),
    );
    readyElements.forEach((rowElement, index) => (rowElement.hidden = !isShown(ready[index])));
    blockedElements.forEach((rowElement, index) => (rowElement.hidden = !isShown(blocked[index])));

    const shownReady = ready.filter(isShown).length;
    const shownBlocked = blocked.filter(isShown).length;
    const count = ready.filter((row, index) => checkboxes[index].checked && isShown(row)).length;
    readyHeader.hidden = shownReady === 0;
    readyCount.textContent = selectedLabel(count, shownReady, ready.length);
    blockedHeader.hidden = shownBlocked === 0;
    blockedCount.textContent = countLabel(shownBlocked, blocked.length);
    empty.hidden = rows.length === 0 || shownReady + shownBlocked > 0;

    selectAll.checked = count > 0 && count === shownReady;
    selectAll.indeterminate = count > 0 && count < shownReady;
    exportButton.disabled = count === 0;
    exportButton.textContent = count === 1 ? "Export 1 drawable" : `Export ${count} drawables`;
  }
}

// Counts only shown rows as selected, matching what Export will download.
function selectedLabel(selected: number, shown: number, total: number): string {
  return shown === total ? `${selected} of ${total} selected` : `${selected} selected · ${shown} of ${total} shown`;
}

function countLabel(shown: number, total: number): string {
  return shown === total ? `${total}` : `${shown} of ${total}`;
}

function summary(readyCount: number, total: number): string {
  if (total === 0) return "The selected layers have been deleted.";
  if (readyCount === total) {
    return total === 1 ? "This layer is ready to export." : `All ${total} layers are ready to export.`;
  }
  if (readyCount === 0) return "None of the selected layers can be exported yet.";
  return `${readyCount} of ${total} layers are ready. The rest need fixes in Figma and won't be exported.`;
}

function readyRow(row: ReadyRow, checkbox: HTMLInputElement, post: Post): HTMLElement {
  return element(
    "div",
    { className: "row" },
    checkbox,
    thumbnail(row),
    element(
      "label",
      { htmlFor: checkbox.id },
      element("span", { className: "name", title: row.name }, row.name),
      element("span", { className: "meta file" }, row.fileName),
      element(
        "span",
        // The original Figma size moves to a tooltip so the note fits beside the exported size.
        { className: "meta size", title: row.resized ? `Resized from ${sizeLabel(row)}` : "" },
        element("span", {}, row.resized ? `${dimensions(row.exportWidth, row.exportHeight)}dp` : (sizeLabel(row) ?? "")),
        ...(row.resized ? [element("span", { className: "resized" }, "Resized to recommended maximum")] : []),
      ),
    ),
    showButton(row.nodeId, post),
    ...(row.warnings.length > 0 ? [issueList(row.warnings, "warning")] : []),
  );
}

function blockedRow(row: BlockedRow, post: Post): HTMLElement {
  const size = sizeLabel(row);
  return element(
    "div",
    { className: "row blocked" },
    element("input", {
      type: "checkbox",
      checked: false,
      disabled: true,
      title: "Fix the issues below to export this layer",
    }),
    thumbnail(row),
    element(
      "label",
      {},
      element("span", { className: "name", title: row.name }, row.name),
      ...(size ? [element("span", { className: "meta size" }, size)] : []),
    ),
    showButton(row.nodeId, post),
    issueList(row.issues, "error"),
  );
}

// The layer's Figma size (for blocked and resized rows), or the drawable's size otherwise.
function sizeLabel(row: ReviewRow): string | undefined {
  if (row.width !== undefined && row.height !== undefined && (row.status === "blocked" || row.resized)) {
    return `${dimensions(row.width, row.height)}dp`;
  }
  return row.status === "ready" ? `${dimensions(row.exportWidth, row.exportHeight)}dp` : undefined;
}

function dimensions(width: number, height: number): string {
  const round = (dp: number) => Math.round(dp * 10) / 10;
  return `${round(width)}×${round(height)}`;
}

// Shows Figma's SVG export, which can differ slightly from the converted drawable.
// An <img> never runs scripts inside the SVG.
function thumbnail(row: ReviewRow): HTMLElement {
  const tile = element("span", { className: "thumb" });
  if (row.preview) {
    const url = URL.createObjectURL(new Blob([row.preview as Uint8Array<ArrayBuffer>], { type: "image/svg+xml" }));
    previewUrls.push(url);
    tile.append(element("img", { src: url, alt: "" }));
  }
  return tile;
}

// A crosshair, the usual "locate on canvas" symbol.
const LOCATE_ICON = `<svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor"
  stroke-linecap="round" aria-hidden="true"><circle cx="8" cy="8" r="4.5"/><path d="M8 1v3M8 12v3M1 8h3M12 8h3"/></svg>`;

function showButton(nodeId: string, post: Post): HTMLElement {
  const button = element("button", { className: "show", title: "Zoom to layer", ariaLabel: "Zoom to layer" });
  button.innerHTML = LOCATE_ICON;
  button.addEventListener("click", () => post({ type: "focus", nodeId }));
  return button;
}

// Diagnostic codes are left out: they describe Figma's SVG export, which designers never see.
function issueList(issues: Issue[], kind: "warning" | "error"): HTMLElement {
  return element(
    "ul",
    { className: `issues ${kind}` },
    ...issues.map((issue) =>
      element(
        "li",
        {},
        issue.message,
        ...(issue.suggestion ? [element("div", { className: "suggestion" }, `→ ${issue.suggestion}`)] : []),
      ),
    ),
  );
}

// One drawable downloads as plain XML; several are bundled so the browser asks only once.
function download(rows: ReadyRow[]): void {
  const encoder = new TextEncoder();
  const [fileName, bytes, type] =
    rows.length === 1
      ? [rows[0].fileName, encoder.encode(rows[0].xml), "application/xml"]
      : [
          "vector-drawables.zip",
          createZip(rows.map((row) => ({ path: row.fileName, data: encoder.encode(row.xml) }))),
          "application/zip",
        ];
  const url = URL.createObjectURL(new Blob([bytes as Uint8Array<ArrayBuffer>], { type }));
  const link = element("a", { href: url, download: fileName });
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1_000);
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  properties: Partial<HTMLElementTagNameMap[K]>,
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const node = Object.assign(document.createElement(tag), properties);
  node.append(...children);
  return node;
}
