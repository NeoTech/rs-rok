# rs-rok Data Flow

## Binary Frame Format

All data between the CLI and the Cloudflare Worker travels as WebSocket binary messages.
Every message is one or more frames in the following layout:

```
 0        1        2        3        4        5        6        7        8
[type 1B][--------request_id u32 LE--------][--------payload_len u32 LE--------]
[--------------------------------- payload (variable) ----------------------------...]
```

| Byte offset | Field        | Size |
|-------------|--------------|------|
| 0           | frame type   | 1 B  |
| 1–4         | request_id   | 4 B (LE u32) |
| 5–8         | payload_len  | 4 B (LE u32) |
| 9+          | payload      | payload_len bytes |

Frame types:

| Hex  | Name          | Direction  |
|------|---------------|------------|
| 0x01 | REGISTER      | CLI → DO   |
| 0x02 | REGISTER_ACK  | DO → CLI   |
| 0x03 | REQUEST       | DO → CLI   |
| 0x04 | RESPONSE      | CLI → DO   |
| 0x05 | PING          | DO → CLI   |
| 0x06 | PONG          | CLI → DO   |
| 0x07 | ERROR         | DO → CLI   |
| 0x08 | WS_OPEN       | DO → CLI   |
| 0x09 | WS_DATA       | bidirectional |
| 0x0A | WS_CLOSE      | bidirectional |
| 0x0E | TCP_OPEN      | DO → CLI   |
| 0x0F | TCP_OPEN_ACK  | CLI → DO   |
| 0x10 | TCP_DATA      | bidirectional |
| 0x11 | TCP_CLOSE     | bidirectional |

Encoding and decoding are implemented once in the `rs_rok_protocol` Rust crate
and compiled to two targets:

- **CLI** — native Rust via `protocol::encode()` / `protocol::decode()`
- **Worker** — WebAssembly via `wasm-pack` into `rs_rok_worker_wasm.wasm`,
  called from TypeScript through `wasm-bridge.ts`

---

## 1. CLI Registration

```
CLI (tunnel.rs)                  Cloudflare Worker (index.ts)        TunnelRegistry DO
      |                                    |                                  |
      |--- WS upgrade GET                  |                                  |
      |    /__rsrok_cli__/<slug>  -------> |                                  |
      |                                    |                                  |
      |                          ModeRegistry.register(mode)                  |
      |                          409 if mode conflict                         |
      |                                    |                                  |
      |                          idFromName(slug)  ----------------------->   |
      |                                    |  WS upgrade forwarded            |
      |<-----------------------------------+------ 101 Switching Protocols ---|
      |                                    |                                  |
      |-- binary: REGISTER frame -----------------------------------------> |
      |   [0x01][req_id][len]                                                 |
      |   [tunnel_id 16B][auth_token 32B][type 1B]                           |
      |                                    |                                  |
      |                                    |  WASM: parse_frame()             |
      |                                    |  validates auth, persists slug   |
      |                                    |  computes public_url             |
      |                                    |                                  |
      |<-- binary: REGISTER_ACK frame ------------------------------------ --|
      |   [0x02][req_id][len]                                                 |
      |   [tunnel_id 16B][url_len u16][public_url UTF-8]                     |
      |                                    |                                  |
      | TUI: TunnelEvent::Connected        |                                  |
      | status = Active                    |                                  |
```

---

## 2. HTTP Request Flow

```
Browser / curl            Worker (index.ts)         TunnelRegistry DO          CLI (tunnel.rs)        Local Service
      |                         |                          |                         |                       |
      |-- GET /path  ---------> |                          |                         |                       |
      |                         |                          |                         |                       |
      |                  ModeRegistry.mode()               |                         |                       |
      |                  root  → idFromName("__root__")    |                         |                       |
      |                  named → idFromName(slug)          |                         |                       |
      |                         |                          |                         |                       |
      |                         |-- fetch(request) ------> |                         |                       |
      |                         |                          |                         |                       |
      |                         |              WASM: encodeRequestFrame()            |                       |
      |                         |              [0x03][req_id][len]                   |                       |
      |                         |              [method 1B][url_len u16][url]         |                       |
      |                         |              [hdr_count u16][name/value pairs]     |                       |
      |                         |              [body_len u32][body bytes]            |                       |
      |                         |                          |                         |                       |
      |                         |                          |-- WS binary msg ------> |                       |
      |                         |                          |                         |                       |
      |                         |                          |         protocol::decode() → Frame::Request     |
      |                         |                          |                         |                       |
      |                         |                          |                         |-- HTTP req ---------->|
      |                         |                          |                         |   http(s)://localhost |
      |                         |                          |                         |                       |
      |                         |                          |                         |<-- HTTP response -----|
      |                         |                          |                         |                       |
      |                         |                          |         protocol::encode(Frame::Response)       |
      |                         |                          |         [0x04][req_id][len]                     |
      |                         |                          |         [status u16][hdr_count u16][...]        |
      |                         |                          |         [body_len u32][body bytes]              |
      |                         |                          |                         |                       |
      |                         |                          |<-- WS binary msg -------|                       |
      |                         |                          |                         |                       |
      |                         |              WASM: parse_frame() → RESPONSE        |                       |
      |                         |              pendingRequests.get(req_id).resolve() |                       |
      |                         |                          |                         |                       |
      |<-- HTTP response -------|                          |                         |                       |
```

---

## 3. WebSocket Upgrade Flow

```
WS Client                 Worker                    TunnelRegistry DO          CLI (tunnel.rs)       Local WS Server
      |                      |                             |                         |                       |
      |-- WS upgrade ------> |                             |                         |                       |
      |                      | idFromName(slug)            |                         |                       |
      |                      |-- fetch(upgrade) ---------> |                         |                       |
      |                      |                             |                         |                       |
      |                      |          WebSocketPair, acceptWebSocket("public")     |                       |
      |                      |                             |                         |                       |
      |<-- 101 Connected ----|                             |                         |                       |
      |                      |                             |                         |                       |
      |                      |          WASM: encodeWsOpenFrame()                    |                       |
      |                      |          [0x08][req_id][len]                          |                       |
      |                      |          [ws_id u32][url_len u16][url]               |                       |
      |                      |          [hdr_count u16][headers][protocol_count...] |                       |
      |                      |                             |-- WS binary msg ------> |                       |
      |                      |                             |                         |-- WS connect -------> |
      |                      |                             |                         |   ws://localhost/...  |
      |                      |                             |                         |                       |
      |-- WS message ------> |                             |                         |                       |
      |                      |     WASM: encodeWsDataFrame()                         |                       |
      |                      |     [0x09][req_id][len][ws_id u32][flags 1B][data]   |                       |
      |                      |                             |-- WS binary msg ------> |                       |
      |                      |                             |            protocol::decode() → Frame::WsData   |
      |                      |                             |                         |-- WS message -------> |
      |                      |                             |                         |                       |
      |                      |                             |                         |<-- WS message --------|
      |                      |                             |     protocol::encode(Frame::WsData)             |
      |                      |                             |<-- WS binary msg -------|                       |
      |                      |     WASM: parse_frame() → WS_DATA                    |                       |
      |                      |     getPublicSocket(ws_id).send()                    |                       |
      |<-- WS message -------|                             |                         |                       |
      |                      |                             |                         |                       |
      |-- close -----------> |    encodeWsCloseFrame()     |-- WS binary msg ------> |-- close ---------->   |
      |                      |                             |<-- WS binary msg -------|                       |
      |                      |    parse_frame() → WS_CLOSE |                         |                       |
      |<-- close ------------|    publicSocket.close()     |                         |                       |
```

---

## 4. TCP Tunnel Flow

```
TCP Client (rs-rok connect)       Worker                TunnelRegistry DO         CLI (tunnel.rs)     Local TCP Service
           |                         |                          |                       |                      |
           |-- WS upgrade ---------->|                          |                       |                      |
           |   /__rsrok_tcp__/<slug> |                          |                       |                      |
           |                         | idFromName(slug)         |                       |                      |
           |                         |-- fetch(upgrade) ------> |                       |                      |
           |                         |                          |                       |                      |
           |                         |      acceptWebSocket("tcp"), WebSocketPair       |                      |
           |<-- 101 Connected -------|                          |                       |                      |
           |                         |                          |                       |                      |
           |-- TCP_OPEN frame -----> |                          |                       |                      |
           |  [0x0E][req_id][len]    |                          |                       |                      |
           |  [stream_id u32]        |   parse_frame()          |                       |                      |
           |  [token_len u16][token] |-- forward to CLI DO ---> |                       |                      |
           |                         |                          |                       |                      |
           |                         |          WASM: encodeTcpOpenFrame()              |                      |
           |                         |          [0x0E][req_id][len]                     |                      |
           |                         |          [stream_id u32][token_len u16][token]   |                      |
           |                         |                          |-- WS binary msg -----> |                      |
           |                         |                          |      decode → Frame::TcpOpen / validate token |
           |                         |                          |                       |-- TCP connect -----> |
           |                         |                          |                       |   localhost:port     |
           |                         |                          |                       |                      |
           |                         |                          |<-- TCP_OPEN_ACK frame-|                      |
           |                         |          parse_frame()   |                       |                      |
           |<-- TCP_OPEN_ACK --------|                          |                       |                      |
           |                         |                          |                       |                      |
           |== TCP data ===========> |   parse_frame→TCP_DATA   |-- TCP_DATA frame ----> |== local write ====> |
           |                         |                          |                       |                      |
           |<== TCP data ============|   encode TCP_DATA        |<-- TCP_DATA frame -----|<== local read ====  |
           |                         |                          |                       |                      |
           |-- TCP_CLOSE ----------->|                          |-- TCP_CLOSE frame ---> |-- socket close ---> |
           |                         |                          |<-- TCP_CLOSE ----------|                      |
           |<-- TCP_CLOSE -----------|                          |                       |                      |
```

---

## 5. WASM Module — Where Encoding Lives

```
  Workspace crates
  ┌──────────────────────────────────────────────────────────────────┐
  │  protocol/src/lib.rs                                             │
  │                                                                  │
  │  Frame enum  ──  encode(frame) → Vec<u8>                        │
  │                                                                  │
  │  [type 1B][req_id 4B LE][len 4B LE][payload]                    │
  │                                                                  │
  │  decode(&[u8]) → (Frame, consumed: usize)                       │
  └──────────────┬────────────────────────┬────────────────────────-┘
                 │                        │
     native link │                wasm-pack target
                 │                (--target bundler)
                 ▼                        ▼
  ┌──────────────────────┐   ┌────────────────────────────────────┐
  │  cli/src/tunnel.rs   │   │  worker-wasm/src/lib.rs            │
  │                      │   │                                    │
  │  decode() → Frame    │   │  #[wasm_bindgen]                   │
  │  encode(Frame) →     │   │  parse_frame(data: &[u8])          │
  │    Vec<u8>           │   │    → JsValue (JS object)           │
  │                      │   │                                    │
  │  sends as WS binary  │   │  encode_request(...)               │
  │  message             │   │  encode_response(...)  etc.        │
  └──────────────────────┘   │    → Uint8Array                    │
                              └───────────────┬────────────────────┘
                                              │ compiled to
                                              ▼
                              pkg/rs_rok_worker_wasm_bg.wasm
                              pkg/rs_rok_worker_wasm.js  (JS glue)
                                              │
                                              │ imported by
                                              ▼
                              worker/src/wasm-bridge.ts
                                              │
                                  initSync({ module: wasmModule })
                                  called once per DO instance
                                              │
                                              ▼
                              worker/src/tunnel-registry.ts
                                  → parseFrame()
                                  → encodeRequestFrame()
                                  → encodeResponseFrame()  etc.
```

---

## 6. Routing Decision Tree (Worker fetch handler)

```
Incoming Request
        │
        ├── pathname === "/health"
        │       └── return 200 "ok"
        │
        ├── Upgrade: websocket  AND  path ~ /__rsrok_cli__/<slug>
        │       │
        │       ├── ModeRegistry.register(mode)
        │       │       └── 409 if conflicting mode (root vs named)
        │       │
        │       └── idFromName(slug) → TunnelRegistry DO
        │               └── handleCliWsUpgrade()
        │                     ├── 409 if CLI already connected (getWebSockets("cli"))
        │                     └── accept WebSocket, send REGISTER_ACK after REGISTER frame
        │
        ├── Upgrade: websocket  AND  path ~ /__rsrok_tcp__/<slug>
        │       └── idFromName(slug) → TunnelRegistry DO
        │               └── handleTcpClientWsUpgrade()
        │
        ├── Upgrade: websocket  (public WebSocket to proxy)
        │       └── idFromName(slug) → TunnelRegistry DO
        │               └── handlePublicWsUpgrade()
        │
        └── HTTP request (proxy)
                │
                ├── ModeRegistry.mode() == "root"
                │       └── idFromName("__root__") → TunnelRegistry DO
                │               └── handleTunnelRequest()  forward full path
                │
                ├── ModeRegistry.mode() == "named"
                │       └── first path segment = slug
                │           idFromName(slug) → TunnelRegistry DO
                │               └── handleTunnelRequest()  strip slug prefix
                │
                └── mode == null → 502 No tunnel connected
```
