import CanvasKitInit from "canvaskit-wasm";
import initVdt from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm.js";
import vdtWasm from "../vendor/vdtoolkit-wasm/vdtoolkit_wasm_bg.wasm";
import { note } from "./columns.ts";
import { DrawableTool } from "./tool-drawable.ts";
import { IconTool } from "./tool-icon.ts";
import { initXmlPane, placeXmlPane } from "./xml-pane.ts";

interface Tool {
  id: string;
  section: HTMLElement;
  activate(hash: string): void;
  open(file: File): Promise<void>;
}

async function main(): Promise<void> {
  const [ck] = await Promise.all([
    CanvasKitInit({ locateFile: (file: string) => new URL(file, import.meta.url).href }),
    initVdt({ module_or_path: new URL(vdtWasm as unknown as string, import.meta.url) }),
  ]);
  initXmlPane();

  const tools: Tool[] = [new DrawableTool(ck), new IconTool(ck)];
  let active = tools[0];

  const switchTo = (tool: Tool, hash: string) => {
    active = tool;
    document.body.dataset.tool = tool.id;
    placeXmlPane(tool.id);
    for (const other of tools) other.section.hidden = other !== tool;
    for (const button of document.querySelectorAll<HTMLButtonElement>(".tools button")) {
      button.setAttribute("aria-pressed", String(button.dataset.tool === tool.id));
    }
    tool.activate(hash);
  };
  for (const button of document.querySelectorAll<HTMLButtonElement>(".tools button")) {
    button.addEventListener("click", () => {
      const tool = tools.find((t) => t.id === button.dataset.tool)!;
      history.replaceState(null, "", tool.id === "drawable" ? location.pathname : `#${tool.id}`);
      switchTo(tool, "");
    });
  }

  const openFile = (file: File | undefined) => {
    if (file) active.open(file);
  };
  const picker = document.getElementById("file") as HTMLInputElement;
  picker.addEventListener("change", () => openFile(picker.files?.[0]));

  let dragDepth = 0;
  document.addEventListener("dragenter", (event) => {
    event.preventDefault();
    if (dragDepth++ === 0) document.body.classList.add("dragging");
  });
  document.addEventListener("dragover", (event) => event.preventDefault());
  document.addEventListener("dragleave", () => {
    if (--dragDepth === 0) document.body.classList.remove("dragging");
  });
  document.addEventListener("drop", (event) => {
    event.preventDefault();
    dragDepth = 0;
    document.body.classList.remove("dragging");
    openFile(event.dataTransfer?.files[0]);
  });

  const initial = tools.find((tool) => `#${tool.id}` === location.hash) ?? tools[0];
  switchTo(initial, location.hash);
}

main().catch((error) => {
  document.getElementById("columns")!.replaceChildren(
    note("error", `The page could not start: ${error instanceof Error ? error.message : error}`),
  );
});
