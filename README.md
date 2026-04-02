# rs-rok

A Cloudflare-backed reverse tunnel tool. Expose local services behind firewalls
to the public internet via HTTPS or TCP, similar to ngrok.

## Download

Pre-built binaries are published for every tagged release:

| Platform | File |
|----------|------|
| Windows x64 | `rsrok-windows-x64.exe` |
| Linux x64 | `rsrok-linux-x64` |
| Linux ARM64 | `rsrok-linux-arm64` |
| macOS x64 (Intel) | `rsrok-macos-x64` |
| macOS ARM64 (Apple Silicon) | `rsrok-macos-arm64` |
| Self-host Docker bundle | `rs-rok-workerd.zip` |

Grab the latest from the [Releases page](../../releases).

On Linux/macOS, mark the binary executable after downloading:
```bash
chmod +x rsrok-linux-x64
./rsrok-linux-x64 --help
```

## Architecture

- **CLI** (`rsrok`): Rust binary that opens a WebSocket tunnel to Cloudflare and
  proxies traffic to a local port. Supports HTTP, HTTPS, and TCP tunnels. The
  Cloudflare Worker is embedded in the binary and can be deployed directly to
  your account.
- **Worker**: Cloudflare Worker (TypeScript + Rust WASM) that brokers connections
  between internet clients and CLI tunnels via a Durable Object.
- **Protocol**: Shared binary framing crate (`no_std`-compatible) used by both the
  CLI and the WASM bridge inside the Worker.

## Quick Start

### 1. Get the binary

**Option A: Download a pre-built release** (recommended)

Download the binary for your platform from the [Releases page](../../releases)
and place it on your `$PATH`. On Linux/macOS run `chmod +x rsrok-*` after downloading.

**Option B: Build from source**

```bash
# Build the worker bundle first (one-time, or after worker code changes)
cd worker && bun install && bun run build:bundle && cd ..

# Build the CLI (embeds the worker bundle)
cargo build --release -p rs-rok-cli
```

The resulting binary is at `target/release/rsrok.exe` (Windows) or
`target/release/rsrok` (Linux/macOS). It is fully self-contained.

### 2. Create a Cloudflare API Token

The CLI deploys a Worker to your Cloudflare account. You need an API token with
the right permissions.

1. Go to [Cloudflare API Tokens](https://dash.cloudflare.com/profile/api-tokens)
2. Click **Create Token**
3. Select the **"Edit Cloudflare Workers"** template
4. Under **Account Resources**, select your account
5. Click **Continue to summary**, then **Create Token**
6. Copy the token

### 3. Deploy the Worker

```bash
# Store your Cloudflare credentials
rsrok config set-cf-credentials \
  --account-id <your-account-id> \
  --api-token <your-api-token>

# Deploy the embedded Worker to your account
rsrok deploy
```

On success, the CLI prints the public Worker URL and saves it as your endpoint.
You can also pass credentials inline or via environment variables:

```bash
# Inline flags
rsrok deploy --account-id <id> --api-token <token>

# Environment variables
CF_ACCOUNT_ID=<id> CF_API_TOKEN=<token> rsrok deploy
```

### 4. Expose a local service

```bash
# Expose local port 8080 over HTTP
rsrok http 8080

# Expose with a stable tunnel name
rsrok http 8080 --name myapp

# Expose a local HTTPS service
rsrok https 8443
```

### 5. Expose a local TCP service

TCP tunnels let you forward raw TCP traffic (databases, SSH, game servers, etc.)
through the Cloudflare tunnel. TCP requires a named tunnel and token-based auth.

```bash
# On the server side: expose local TCP port 5432 (e.g. PostgreSQL)
rsrok tcp 5432 --name mydb
# Prints a one-time token, e.g.: TCP tunnel token: abc123...

# On the client side: connect and map to a local port
rsrok connect mydb --token abc123... --port 15432

# Now connect to localhost:15432 as if it were the remote service
psql -h 127.0.0.1 -p 15432 -U myuser mydb
```

The server side (`rsrok tcp`) generates a single-use token that the client
(`rsrok connect`) uses to authenticate. Multiple clients can connect
simultaneously, each getting an independent TCP stream multiplexed over the
WebSocket tunnel.

## Interactive TUI

Running `rsrok` with no arguments in a terminal launches an interactive TUI:

```bash
rsrok
```

Key bindings:

| Key | Action |
|-----|--------|
| `n` | Open new tunnel form |
| `m` | Tunnel manager (navigate, restart, delete saved tunnels) |
| `d` | Stop selected tunnel |
| `r` | Restart stopped tunnel |
| `x` | Delete tunnel from list and session file |
| `s` | Settings (edit profiles and Cloudflare credentials) |
| `p` | Switch active profile |
| `D` | Deploy Worker to Cloudflare |
| `t` | Test endpoint for selected HTTP tunnel |
| `Tab` | Switch focus between tunnel list and log view |
| `q` | Quit |

The TUI persists open tunnels to `~/.rs-rok/tunnels.json` and restores them
automatically on next launch. Tunnels that were running when you quit are
reconnected; stopped tunnels are shown in the manager but not auto-connected.

## Self-hosted (Docker)

You can run the Worker locally using [workerd](https://github.com/cloudflare/workerd)
(Cloudflare's open-source runtime) in a Docker container, without needing a
Cloudflare account.

### Prerequisites

- Docker
- Bun, Rust, and wasm-pack (for building the Worker bundle)

### Build and run

If you have Docker, Bun, Rust, and wasm-pack available:

```bash
# Build the Worker bundle and normalize the WASM filename
cd worker && bun run build:workerd

# Start the container (builds a ~35 MB image)
docker compose up --build
```

Alternatively, download `rs-rok-workerd.zip` from the
[Releases page](../../releases) — it contains the pre-built
`index.js`, `worker.wasm`, `Dockerfile.workerd`, `docker-compose.yml`,
and `workerd.capnp`. Unzip and run:

```bash
unzip rs-rok-workerd.zip
docker compose -f worker/docker-compose.yml up --build
```

The Worker is available at `http://localhost:8787`. Durable Object state is
persisted to a Docker volume (`do-data`).

### Point the CLI at the local instance

```bash
rsrok config set-endpoint http://localhost:8787
```

Then use `rsrok http`, `rsrok tcp`, etc. as normal -- traffic routes through
the local container instead of Cloudflare.

### Stop the container

```bash
cd worker && docker compose down
```

To also remove persisted Durable Object data:

```bash
cd worker && docker compose down -v
```

## Configuration

Settings are stored in `~/.rs-rok/`:

| File | Purpose |
|------|---------|
| `settings.json` | Named profiles: endpoint URL, auth token, default region |
| `cloudflare.json` | Cloudflare account ID and API token |
| `tunnels.json` | TUI session: saved tunnels with their last-known state |

### CLI Commands

```
rsrok http <port>              Expose a local HTTP service
rsrok https <port>             Expose a local HTTPS service
rsrok tcp <port> --name <name> Expose a local TCP service (named tunnel required)
rsrok connect <name> --token <tok> --port <port>
                                Connect to a TCP tunnel as a client
rsrok deploy                   Deploy the Worker to Cloudflare
rsrok config show              Print current configuration
rsrok config add-token <tok>   Store an auth token
rsrok config set-endpoint <url> Set the worker endpoint URL
rsrok config set-cf-credentials Store Cloudflare credentials
```

### Environment Variables

| Variable | Overrides |
|----------|-----------|
| `RS_ROK_TOKEN` | `settings.json` auth_token |
| `RS_ROK_ENDPOINT` | `settings.json` endpoint |
| `CF_ACCOUNT_ID` | `cloudflare.json` account_id |
| `CF_API_TOKEN` | `cloudflare.json` api_token |

## Development

```bash
# Check all Rust crates
cargo check --workspace

# Run all Rust tests
cargo test --workspace

# Worker dev server
cd worker && bun install && bun run dev

# Worker tests
cd worker && bun run test

# Rebuild the worker bundle (after changing worker/worker-wasm/protocol code)
cd worker && bun run build:bundle
```

## License

MIT
