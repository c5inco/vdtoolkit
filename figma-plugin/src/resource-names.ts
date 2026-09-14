// Android resource files must be named with lowercase letters, digits, and underscores,
// starting with a letter, so "Icons/Arrow Left" becomes "icons_arrow_left".
export function resourceName(layerName: string): string {
  const name = layerName
    .normalize("NFKD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (name === "") return "vector";
  return /^[a-z]/.test(name) ? name : `ic_${name}`;
}

// Layers with the same name would overwrite each other in the download, so later ones get a suffix.
export function uniqueResourceNames(layerNames: string[]): string[] {
  const taken = new Set<string>();
  return layerNames.map((layerName) => {
    const base = resourceName(layerName);
    let name = base;
    for (let suffix = 2; taken.has(name); suffix++) name = `${base}_${suffix}`;
    taken.add(name);
    return name;
  });
}
