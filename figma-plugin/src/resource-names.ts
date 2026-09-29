// Android resource files must be named with lowercase letters, digits, and underscores,
// starting with a letter or underscore and not a Java keyword. The converter names them, so a
// layer is exported under the same name the CLI gives a file: "Icons/Arrow Left" becomes
// "icons_arrow_left" and "class" becomes "ic_class". A name with no letters or digits to keep
// falls back to "vector".
export type ResourceName = (layerName: string) => string | undefined;

// Layers with the same name would overwrite each other in the download, so later ones get a suffix.
export function uniqueResourceNames(layerNames: string[], resourceName: ResourceName): string[] {
  const taken = new Set<string>();
  return layerNames.map((layerName) => {
    const base = resourceName(layerName) ?? "vector";
    let name = base;
    for (let suffix = 2; taken.has(name); suffix++) name = `${base}_${suffix}`;
    taken.add(name);
    return name;
  });
}
