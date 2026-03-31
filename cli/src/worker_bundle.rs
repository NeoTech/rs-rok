/// Embedded Cloudflare Worker artifacts, baked in at compile time by build.rs.
pub const WORKER_SCRIPT: &[u8] = include_bytes!("embedded/worker.js");
pub const WORKER_WASM: &[u8] = include_bytes!("embedded/worker.wasm");

/// The WASM module filename used in the JS bundle's import statement.
/// The Cloudflare Workers API multipart upload must reference this exact name.
pub const WASM_MODULE_NAME: &str = include_str!("embedded/wasm_module_name.txt");

pub const COMPATIBILITY_DATE: &str = "2024-12-30";
