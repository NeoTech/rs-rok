# Lessons Learned

Rules and patterns to follow for the rs-rok project. Updated after corrections or discoveries.

---

## 1. Wrangler must be run through `bun`, not `bunx`

- **Pattern**: Using `bunx wrangler ...` to run wrangler commands.
- **Rule**: Always use `bun wrangler ...` or `bun run <script>` (where the script calls wrangler). Never use `bunx wrangler`.
- **Applies to**: `wrangler dev`, `wrangler deploy`, `wrangler kv`, `wrangler d1`, etc.

## 2. vitest + @cloudflare/vitest-pool-workers hangs on Windows with Bun

- **Pattern**: Using `vitest` with `@cloudflare/vitest-pool-workers` on Windows with Bun as the runtime.
- **Problem**: The workers pool spawns workerd which hangs indefinitely. Also, plain vitest with `pool: "forks"` or `"threads"` fails with `File URL path must be an absolute path` or `port.addListener is not a function` under Bun.
- **Rule**: Use `bun:test` (Bun's native test runner) instead of vitest for testing Worker TypeScript code. Import from `"bun:test"` and run with `bun test`.
- **Cleanup**: Remove `vitest` and `@cloudflare/vitest-pool-workers` from devDependencies. Remove `vitest.config.ts`.

## 3. wasm-pack incompatible with workspace-inherited `license` field

- **Pattern**: Using `license.workspace = true` in a crate built with `wasm-pack`.
- **Problem**: wasm-pack fails with `invalid type: map, expected a string for key package.license`.
- **Rule**: For crates built with `wasm-pack`, always set `license = "MIT"` (or the actual license string) directly instead of inheriting from the workspace.

## 5. Cloudflare Durable Objects are revived fresh for each HTTP request

- **Pattern**: Storing WebSocket references or derived metadata (slug, origin URL) as instance variables on a Durable Object.
- **Problem**: Each HTTP request creates a **fresh DO instance** — instance variables set during a prior WebSocket upgrade are `null`.
- **Rule**: Use `this.state.storage.put()` to persist any cross-request state (slugs, origins, configs) during the WebSocket upgrade. In `fetch()` and WebSocket message handlers, call `this.state.getWebSockets("tag")` rather than `this.cliSocket` to retrieve active WebSockets.

## 6. Durable Object name must match between WebSocket and HTTP routes

- **Pattern**: Using `idFromName(uuid)` for the WebSocket route (`/ws/:uuid`) but `idFromName(slug)` for the HTTP proxy route (`/tunnel/:slug`) where slug is different from the uuid.
- **Problem**: Two different DOs are created — one has the WebSocket connection, the other has nothing.
- **Rule**: Both the WebSocket registration URL and the public tunnel URL must use the **same ID** as the DO name. Use the CLI's tunnel UUID as both the WS path segment and the public tunnel slug. Don't generate a separate random slug.

## 7. tokio-tungstenite requires wss:// not https:// 

- **Pattern**: Storing the endpoint as `https://...` and using it directly as a WebSocket URL.
- **Problem**: `connect_async("https://...")` fails with "URL scheme not supported".
- **Rule**: Before connecting, replace `https://` → `wss://` and `http://` → `ws://` in the endpoint URL.

## 8. Follow the user's active troubleshooting scope

- **Pattern**: Continuing to investigate a previously discussed issue after the user redirects the task.
- **Problem**: Time is spent on unrelated fixes while the current blocker remains unresolved.
- **Rule**: When the user narrows scope (for example, "focus on websocket/long-poll behavior"), immediately pivot and avoid additional work on out-of-scope issues unless explicitly requested.

## 9. Never spawn `cargo` from inside `build.rs`

- **Pattern**: `build.rs` runs `bun run build:bundle` which calls `wasm-pack build`, which internally runs `cargo build --target wasm32-unknown-unknown`.
- **Problem**: The parent `cargo build` holds the Cargo build lock. The child `cargo` spawned by wasm-pack blocks waiting for the same lock → deadlock. Appears as `cargo build --release` "hanging forever".
- **Rule**: If `build.rs` needs artifacts produced by another cargo build (e.g. WASM), check if pre-built artifacts exist and use them. Never unconditionally spawn a child cargo process. The worker bundle must be built as a separate step (`cd worker && bun run build:bundle`) before `cargo build`.
## 10. Deploy must write ALL changed fields back to the active profile

- **Pattern**: Adding a new `auth_token` parameter to deploy, updating `endpoint` in settings after deploy, but forgetting to also persist `auth_token`.
- **Problem**: `settings.json` stays at `"auth_token": null` after a deploy that specified a token.
- **Second pattern (TUI only)**: Upsert-by-worker-name — trying to find an existing profile by matching `worker_name` (the CF worker script name) against profile names. These are unrelated concepts; worker_name never matches → a new duplicate profile is created.
- **Rule**: After deploy, always update the **active profile** directly (`settings.active_profile_mut()`). Write every field that the deploy action changed (endpoint AND auth_token). Never try to match a CF worker name against settings profile names.