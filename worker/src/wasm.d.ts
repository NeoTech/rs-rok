// Type declarations for Cloudflare Workers WASM module imports
declare module "*.wasm" {
  const module: WebAssembly.Module;
  export default module;
}
