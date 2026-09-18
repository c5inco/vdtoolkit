declare module "*.svg" {
  const text: string;
  export default text;
}

declare module "*.xml" {
  const text: string;
  export default text;
}

declare module "*.wasm" {
  const url: string;
  export default url;
}
