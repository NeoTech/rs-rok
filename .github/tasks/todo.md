# rs-rok -- Cloudflare-backed ngrok clone

## Goal

Build a Rust CLI tool (`rs-rok`) + Cloudflare Worker that replicates ngrok functionality:
expose local services behind firewalls to the public internet via HTTPS tunnels
through Cloudflare's edge network.

## Definition of Done

- `rs-rok http 8080` opens a tunnel and prints a public `https://<id>.workers.dev` URL
- Hitting that URL proxies traffic to `localhost:8080` on the machine running the CLI
- Binary protocol with raw framing over WSS between CLI and Durable Object
- Rust WASM handles frame encode/decode inside the Worker
- Full unit test coverage for protocol, CLI, and Worker
- Integration test: mock-service + wrangler dev + CLI + HTTP round-trip assertion
- Config stored in `~/.rs-rok/settings.json`

## Architecture

```
Internet User
    |  HTTPS (random-id.workers.dev)
    v
Cloudflare Worker  (TS entry + Rust WASM framing)
    |  routes to TunnelRegistry Durable Object by tunnel_id
    v
Durable Object  (WebSocket hibernation API)
    ^  raw binary frames over WSS
    |
rs-rok CLI  (Rust, tokio + tokio-tungstenite)
    |  hyper HTTP client
    v
Local Service  (e.g. your app on :8080)
```

## Binary Protocol

All frames: `[1B type][4B request_id LE][4B payload_len LE][payload]`

| Type | Name        | Direction | Payload summary                                  |
|------|-------------|-----------|--------------------------------------------------|
| 0x01 | REGISTER     | CLI->DO   | tunnel_id(16B UUID), auth_token(32B), type(1B)   |
| 0x02 | REGISTER_ACK | DO->CLI   | tunnel_id(16B), public_url(u16-len + UTF-8)      |
| 0x03 | REQUEST      | DO->CLI   | method(1B), url, headers, body                   |
| 0x04 | RESPONSE     | CLI->DO   | status(u16), headers, body                       |
| 0x05 | PING         | both      | empty                                            |
| 0x06 | PONG         | both      | empty                                            |
| 0x07 | ERROR        | both      | code(u16), message                               |

## Repository Structure

```
rs-rok/
  Cargo.toml              # workspace: cli, protocol, worker-wasm, mock-service
  package.json            # bun workspace scripts
  cli/                    # Rust CLI binary
  protocol/               # no_std binary protocol crate (shared by CLI + WASM)
  worker-wasm/            # wasm-bindgen exports of protocol crate
  worker/                 # Cloudflare Worker (TS + WASM, deployed via wrangler)
  mock-service/           # axum echo server for testing
  tests/integration/      # e2e test harness
```

---

## Phase 1 -- Foundation

Purpose: scaffold every crate and package so `cargo check --workspace` and `bun install` succeed.

- [x] 1.1 Create root `Cargo.toml` (workspace members), `.gitignore`, `README.md`
  - Test: `cargo check --workspace` exits 0
- [x] 1.2 Scaffold `protocol/` crate with minimal `Cargo.toml` + `src/lib.rs`
  - Test: `cargo check -p rs-rok-protocol`
- [x] 1.3 Scaffold `cli/` crate with minimal `Cargo.toml` + `src/main.rs`
  - Test: `cargo check -p rs-rok-cli`
- [x] 1.4 Scaffold `worker-wasm/` crate with minimal `Cargo.toml` + `src/lib.rs`
  - Test: `cargo check -p rs-rok-worker-wasm`
- [x] 1.5 Scaffold `mock-service/` crate with minimal `Cargo.toml` + `src/main.rs`
  - Test: `cargo check -p rs-rok-mock-service`
- [x] 1.6 Scaffold `worker/` package: `package.json`, `wrangler.toml`, `tsconfig.json`, stub `src/index.ts`
  - Test: `cd worker && bun install` exits 0

## Phase 2 -- Protocol Layer

Purpose: implement the canonical binary frame format in a `no_std`-compatible crate
shared by both the CLI and the WASM bridge.

- [x] 2.1 Define `Frame` enum with all 7 frame types in `protocol/src/lib.rs`
- [x] 2.2 Implement `encode(&Frame) -> Vec<u8>` serializer
- [x] 2.3 Implement `decode(&[u8]) -> Result<(Frame, usize), DecodeError>` parser
- [x] 2.4 Write unit tests: round-trip every frame type, malformed input, partial reads, zero-len payloads
  - Test: `cargo test -p rs-rok-protocol` all green (25 tests)
- [x] 2.5 Implement `worker-wasm/src/lib.rs`: wasm-bindgen exports (`parse_frame`, `encode_frame`)
  - Test: `cargo check -p rs-rok-worker-wasm` passes

## Phase 3 -- Cloudflare Worker

Purpose: build the edge-side connection broker: a Worker that routes HTTPS requests
to a Durable Object holding persistent WebSocket connections from CLI clients.

- [x] 3.1 Configure `wrangler.toml`: TunnelRegistry DO, KV `TUNNEL_AUTH`, D1 `AUDIT_LOG`
- [x] 3.2 Implement `wasm-bridge.ts`: placeholder for future WASM integration (pure TS encode/decode inline in DO for now)
- [x] 3.3 Implement `tunnel-registry.ts` Durable Object:
  - `webSocketMessage()`: dispatch REGISTER, RESPONSE, PING frames
  - `fetch()`: encode inbound HTTP as REQUEST frame, resolve via request_id promise map
- [x] 3.4 Implement `index.ts` Worker entry: parse tunnel_id from URL path, forward to DO
- [x] 3.5 Write bun:test unit tests for binary protocol encoding/decoding
  - Test: `cd worker && bun test` all green (8 tests)

## Phase 4 -- Rust CLI

Purpose: build the CLI binary that opens a WSS tunnel to the Durable Object
and proxies traffic to local services.

- [x] 4.1 Implement `cli/src/cli.rs`: clap subcommands `http <port>`, `https <port>`, `config add-token`, `config show`, `config set-endpoint`; global flags `--config`, `--log`
- [x] 4.2 Implement `cli/src/config.rs`: load/save `~/.rs-rok/settings.json`, env var overrides (`RS_ROK_TOKEN`, `RS_ROK_ENDPOINT`)
- [x] 4.3 Implement `cli/src/tunnel.rs`: WSS connect, REGISTER handshake, main loop (REQUEST/RESPONSE/PING/PONG), exponential backoff reconnect
- [x] 4.4 Implement `cli/src/proxy.rs`: on REQUEST frame spawn tokio task, hyper client forward to localhost:<port>, encode RESPONSE
- [x] 4.5 Write unit tests: config round-trips
  - Test: `cargo test -p rs-rok-cli` all green (3 tests)

## Phase 5 -- Mock Service

Purpose: a configurable echo HTTP server used as the "local app" in integration tests.

- [x] 5.1 Implement `mock-service/src/main.rs`: axum server, `--port` flag, routes: `GET/POST /echo`, `GET /status/:code`, `GET /slow/:ms`, `GET /health`, structured JSON logs
  - Test: `cargo check -p rs-rok-mock-service` passes

## Phase 6 -- Integration Tests

Purpose: prove the full tunnel round-trip works locally.

- [x] 6.1 Write `cli/tests/tunnel_e2e.rs`:
  - Spawn mock-service on random free port
  - Spawn `wrangler dev` (local worker)
  - Spawn `rs-rok http <port>` targeting local wrangler endpoint
  - Poll until REGISTER_ACK
  - `reqwest::get` through tunnel -> assert 200 + echoed body
  - Teardown all processes, assert clean exit
- [x] 6.2 Add `bun run test:e2e` script that wraps `cargo test -p rs-rok-cli --test tunnel_e2e -- --ignored`
  - Test: `bun run test:e2e` compiles and builds

## Phase 7 -- Verification

- [x] 7.1 `cargo test --workspace` -- all unit tests green (28 tests: 25 protocol + 3 config)
- [x] 7.2 `cd worker && bun test` -- 8 tests green
- [ ] 7.3 Full local integration test passes (requires running `bun run test:e2e` manually)
- [ ] 7.4 `wrangler deploy`, smoke-test `rs-rok http 8080` against live Cloudflare edge

---

## Decisions & Scope

- v1: HTTP + HTTPS (TLS terminated at Cloudflare edge, forwarded as plain HTTP to CLI)
- Raw TCP tunnels deferred to v2 (Cloudflare inbound TCP not GA)
- Auth v1: pre-shared token in KV; no dashboard
- WASM role: binary frame encode/decode only; routing logic stays in TypeScript
- Subdomain: DO generates 12-char nanoid on REGISTER, persists in DO SQLite
- Config priority: CLI args > env vars > settings.json > defaults

## Review

### What was done
- Phase 1-6 fully implemented. All code compiles and unit tests pass.
- **28 Rust tests** (25 protocol + 3 config) passing via `cargo test --workspace`
- **8 TypeScript tests** passing via `bun test` in worker/
- Integration test scaffolded in `cli/tests/tunnel_e2e.rs` (marked `#[ignore]`, requires manual run with `bun run test:e2e`)

### Architecture decisions during implementation
- Switched from `vitest` + `@cloudflare/vitest-pool-workers` to `bun:test` due to compatibility issues on Windows
- WASM bridge is a placeholder; binary frame encoding/decoding is inline TypeScript in the Durable Object for now
- Integration test uses `ProcessGuard` RAII pattern for reliable cleanup of spawned processes

### Remaining for production
- 7.3: Run full integration test (`bun run test:e2e`) to prove end-to-end tunnel works locally
- 7.4: Deploy to Cloudflare (`bun wrangler deploy`) and smoke-test against live edge
- Auth gate via KV not yet wired (auth token is accepted but not validated in DO)
- D1 audit logging not yet wired

---

## Phase 8 -- Subdomain Routing (requires custom domain)

Purpose: replace path-based routing (`/myapp/...`) with proper subdomain routing
(`myapp.yourdomain.com/...`) so any app with absolute paths works without changes.

### Blocker
Requires a domain added to Cloudflare. `workers.dev` subdomains are not controllable.

### Plan

- [ ] 8.1 Add domain to Cloudflare and configure wildcard DNS
  - Add `*.yourdomain.com` CNAME pointing to `rs-rok-worker.andreas-016.workers.dev`
  - Test: `nslookup anything.yourdomain.com` resolves to Cloudflare IPs

- [ ] 8.2 Add wildcard route to `wrangler.toml`
  - Add `routes = [{ pattern = "*.yourdomain.com/*", zone_name = "yourdomain.com" }]`
  - Test: `wrangler deploy` succeeds with route registered

- [ ] 8.3 Update Worker routing to read tunnel name from Host header
  - In `index.ts`: extract subdomain from `request.headers.get("host")` (e.g. `myapp.yourdomain.com` -> `myapp`)
  - Keep `/ws/:tunnelId` WebSocket path unchanged (CLI connects to main worker domain)
  - Route all subdomain requests to `env.TUNNEL_REGISTRY.idFromName(subdomain)`
  - Test: `curl https://myapp.yourdomain.com/health` returns `ok`

- [ ] 8.4 Update DO to skip path stripping (no prefix to remove)
  - `handleTunnelRequest` forwards full path as-is
  - Test: `curl https://myapp.yourdomain.com/api/users` forwards `/api/users` to local service

- [ ] 8.5 Update CLI to print subdomain URL in REGISTER_ACK
  - Worker sends `https://myapp.yourdomain.com` as public_url in REGISTER_ACK
  - Test: CLI prints `Tunnel: https://myapp.yourdomain.com`

- [ ] 8.6 Smoke test with a Vite app using absolute paths
  - Start `vite dev` on port 5173, run `rs-rok http 5173 --name myapp`
  - Open `https://myapp.yourdomain.com` in browser, verify all assets load
  - Test: browser devtools shows no 404s for JS/CSS assets

---

## Phase 9 -- Self-Deploy (Bundle Worker into CLI Binary)

Purpose: embed the compiled Cloudflare Worker (JS bundle + WASM binary) directly into 
the `rs-rok` CLI executable so users can run `rs-rok deploy` to self-host their own 
worker instance via the Cloudflare REST API -- no manual wrangler usage required.

### Definition of Done
- `cargo build` automatically compiles the Worker and embeds artifacts in the binary
- `rs-rok deploy --account-id <ID> --api-token <TOKEN>` deploys a working Worker
- Cloudflare dashboard shows the Worker with both Durable Objects
- `rs-rok http 8080` works against the self-deployed Worker endpoint
- Credentials persist in `~/.rs-rok/cloudflare.json` for subsequent deploys

### Phase 9a -- Build Infrastructure

- [x] 9.1 Investigate wrangler dry-run output
  - Run `cd worker && bun wrangler deploy --dry-run --outdir dist/` and inspect output
  - Identify: JS bundle filename, WASM filename, how bundled JS imports the WASM
  - This determines the multipart upload part names for the Cloudflare API
  - Result: `index.js` + `{hash}-rs_rok_worker_wasm_bg.wasm`, JS imports WASM by hash-prefixed filename

- [x] 9.2 Add `build:bundle` script to `worker/package.json`
  - Script: `"build:bundle": "bun run build:wasm && wrangler deploy --dry-run --outdir dist"`
  - Test: `cd worker && bun run build:bundle` produces `worker/dist/` with JS + WASM

- [x] 9.3 Create `cli/build.rs`
  - Runs `bun run build:bundle` in `../worker/` via `std::process::Command`
  - Copies `worker/dist/*.js` -> `cli/src/embedded/worker.js`
  - Copies `worker/dist/*.wasm` -> `cli/src/embedded/worker.wasm`
  - Writes WASM module name to `cli/src/embedded/wasm_module_name.txt`
  - Falls back to existing artifacts if bun/wrangler unavailable; panics if missing
  - Emits `cargo:rerun-if-changed` for worker/protocol/worker-wasm source files
  - Test: `cargo build -p rs-rok-cli` succeeds and produces embedded artifacts

### Phase 9b -- Embed Artifacts

- [x] 9.4 Create `cli/src/embedded/` directory
  - Add `.gitkeep`; gitignore `cli/src/embedded/*.js`, `*.wasm`, `wasm_module_name.txt`
  - Test: `git status` shows `.gitkeep` tracked, build artifacts ignored

- [x] 9.5 Create `cli/src/worker_bundle.rs`
  - `include_bytes!("embedded/worker.js")` + `include_bytes!("embedded/worker.wasm")`
  - `include_str!("embedded/wasm_module_name.txt")` for dynamic WASM module name
  - Constants for compatibility date (from wrangler.toml)
  - Test: `cargo build -p rs-rok-cli` compiles with embedded artifacts

### Phase 9c -- Cloudflare Credentials Config

- [x] 9.6 Create `cli/src/cloudflare_config.rs`
  - Struct: `CloudflareConfig { account_id, api_token }`
  - Path: `~/.rs-rok/cloudflare.json`
  - Load with env overrides: `CF_ACCOUNT_ID`, `CF_API_TOKEN`
  - Save method (same pattern as `config.rs`)
  - Test: `cargo test -p rs-rok-cli` -- 2 new tests pass (round-trip + missing file)

### Phase 9d -- Deploy Module

- [x] 9.7 Move `reqwest` from dev-dependencies to dependencies in `cli/Cargo.toml`
  - Features: `["json", "multipart", "blocking"]` (blocking kept for integration test)
  - Test: `cargo check -p rs-rok-cli` passes

- [x] 9.8 Create `cli/src/deploy.rs`
  - `pub async fn deploy_worker(cf, worker_name) -> Result<String, DeployError>`
  - Builds metadata JSON: main_module, compatibility_date, compatibility_flags,
    DO bindings (TUNNEL_REGISTRY, MODE_REGISTRY), migrations (v2, 2 steps)
  - Builds multipart form: metadata + index.js + WASM file (dynamic name)
  - PUT to `https://api.cloudflare.com/client/v4/accounts/{id}/workers/scripts/{name}`
  - Enables workers.dev subdomain, fetches subdomain name, returns full URL
  - Test: `cargo check -p rs-rok-cli` passes; manual test with real credentials pending

### Phase 9e -- CLI Wiring

- [x] 9.9 Add `Deploy` command to `cli/src/cli.rs`
  - `rs-rok deploy [--account-id <ID>] [--api-token <TOKEN>] [--name rs-rok]`

- [x] 9.10 Add `ConfigAction::SetCfCredentials` to `cli/src/cli.rs`
  - `rs-rok config set-cf-credentials --account-id <ID> --api-token <TOKEN>`

- [x] 9.11 Handle `Deploy` + `SetCfCredentials` in `cli/src/main.rs`
  - Deploy: load CloudflareConfig, apply flag overrides, call deploy_worker,
    on success save credentials + update settings.endpoint
  - SetCfCredentials: save CloudflareConfig to disk

### Phase 9f -- Verification

- [ ] 9.12 Full verification (requires real Cloudflare credentials)
  - `cargo build --release` embeds worker artifacts, binary size increases
  - `rs-rok deploy --account-id $ID --api-token $TOKEN` deploys to Cloudflare
  - Dashboard shows `rs-rok` Worker with TunnelRegistry + ModeRegistry DOs
  - `rs-rok http 8080` against self-deployed endpoint works end-to-end
