import type { BlockedRow, Issue, ReadyRow, ReviewRow } from "./export-review";
import type { SandboxMessage } from "./messages";
import { createZip } from "./zip";

type Post = (message: SandboxMessage) => void;

// What the user unchecked, kept across refreshes so re-checking layers doesn't undo their choices.
const unchecked = new Set<string>();

const STYLES = `
  :root { color-scheme: light dark; }
  body {
    margin: 0; height: 100vh; display: flex; flex-direction: column;
    font: 11px/16px Inter, system-ui, sans-serif;
    color: var(--figma-color-text, #1e1e1e); background: var(--figma-color-bg, #fff);
  }
  .status { padding: 16px; color: var(--figma-color-text-secondary, #757575); }
  .summary { margin: 0; padding: 12px 16px; border-bottom: 1px solid var(--figma-color-border, #e6e6e6); }
  .list { flex: 1; overflow-y: auto; padding-bottom: 8px; }
  h2 {
    display: flex; align-items: center; gap: 8px; margin: 0; padding: 12px 16px 4px;
    font-size: 11px; font-weight: 600;
  }
  h2 .count { color: var(--figma-color-text-secondary, #757575); font-weight: 400; }
  .row { display: grid; grid-template-columns: 16px 1fr auto; column-gap: 8px; padding: 6px 16px; }
  .row:hover { background: var(--figma-color-bg-hover, #f5f5f5); }
  .row input { margin: 0; align-self: start; margin-top: 2px; }
  .blocked-mark {
    width: 12px; height: 12px; margin: 2px; border-radius: 50%; align-self: start;
    display: grid; place-items: center; font-size: 9px; font-weight: 700; line-height: 1;
    background: var(--figma-color-bg-danger, #f24822); color: var(--figma-color-text-ondanger, #fff);
  }
  .row label { display: flex; flex-direction: column; min-width: 0; cursor: pointer; }
  .row.blocked label { cursor: default; }
  .name { font-weight: 500; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file { color: var(--figma-color-text-secondary, #757575); font-family: ui-monospace, Menlo, monospace; font-size: 10px; }
  .issues { grid-column: 2 / 4; margin: 4px 0 0; padding: 0; list-style: none; }
  .issues li { margin-top: 2px; }
  .issues.warning { color: var(--figma-color-text-warning, #b86200); }
  .issues.error { color: var(--figma-color-text-danger, #dc3412); }
  .issues code { font-size: 10px; opacity: 0.8; }
  .suggestion { color: var(--figma-color-text-secondary, #757575); }
  button {
    font: inherit; border-radius: 5px; padding: 0 12px; height: 28px; cursor: pointer;
    border: 1px solid var(--figma-color-border, #e6e6e6); background: transparent; color: inherit;
  }
  .show { height: 20px; padding: 0 6px; border-color: transparent; color: var(--figma-color-text-secondary, #757575); }
  .show:hover { border-color: var(--figma-color-border, #e6e6e6); }
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
  const ready = rows.filter((row): row is ReadyRow => row.status === "ready");
  const blocked = rows.filter((row): row is BlockedRow => row.status === "blocked");

  const checkboxes = ready.map((row) => {
    const checkbox = element("input", { type: "checkbox", checked: !unchecked.has(row.nodeId), id: `row-${row.nodeId}` });
    checkbox.addEventListener("change", update);
    return checkbox;
  });
  const selectAll = element("input", { type: "checkbox", checked: true, title: "Select all" });
  selectAll.addEventListener("change", () => {
    for (const checkbox of checkboxes) checkbox.checked = selectAll.checked;
    update();
  });

  const exportButton = element("button", { className: "primary" });
  exportButton.addEventListener("click", () => {
    const chosen = ready.filter((_, index) => checkboxes[index].checked);
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

  const list = element("section", { className: "list" });
  if (ready.length > 0) {
    list.append(
      element("h2", {}, selectAll, "Ready to export", element("span", { className: "count" }, `${ready.length}`)),
      ...ready.map((row, index) => readyRow(row, checkboxes[index], post)),
    );
  }
  if (blocked.length > 0) {
    list.append(
      element("h2", {}, "Needs fixes in Figma", element("span", { className: "count" }, `${blocked.length}`)),
      ...blocked.map((row) => blockedRow(row, post)),
    );
  }

  const scrollTop = document.querySelector(".list")?.scrollTop ?? 0;
  document.body.replaceChildren(
    element("p", { className: "summary" }, summary(ready.length, rows.length)),
    list,
    element("footer", {}, refreshButton, cancelButton, exportButton),
  );
  list.scrollTop = scrollTop;
  update();

  function update() {
    ready.forEach((row, index) =>
      checkboxes[index].checked ? unchecked.delete(row.nodeId) : unchecked.add(row.nodeId),
    );
    const count = checkboxes.filter((checkbox) => checkbox.checked).length;
    selectAll.checked = count > 0 && count === checkboxes.length;
    selectAll.indeterminate = count > 0 && count < checkboxes.length;
    exportButton.disabled = count === 0;
    exportButton.textContent = count === 1 ? "Export 1 drawable" : `Export ${count} drawables`;
  }
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
    element(
      "label",
      { htmlFor: checkbox.id },
      element("span", { className: "name", title: row.name }, row.name),
      element("span", { className: "file" }, row.fileName),
    ),
    showButton(row.nodeId, post),
    ...(row.warnings.length > 0 ? [issueList(row.warnings, "warning")] : []),
  );
}

function blockedRow(row: BlockedRow, post: Post): HTMLElement {
  return element(
    "div",
    { className: "row blocked" },
    element("span", { className: "blocked-mark", title: "Fix the issues below to export this layer" }, "!"),
    element("label", {}, element("span", { className: "name", title: row.name }, row.name)),
    showButton(row.nodeId, post),
    issueList(row.issues, "error"),
  );
}

function showButton(nodeId: string, post: Post): HTMLElement {
  const button = element("button", { className: "show", title: "Zoom to this layer" }, "Show");
  button.addEventListener("click", () => post({ type: "focus", nodeId }));
  return button;
}

function issueList(issues: Issue[], kind: "warning" | "error"): HTMLElement {
  return element(
    "ul",
    { className: `issues ${kind}` },
    ...issues.map((issue) =>
      element(
        "li",
        {},
        issue.message,
        ...(issue.code ? [" ", element("code", {}, issue.code)] : []),
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
