# rs-rok

A Cloudflare-backed reverse tunnel tool. Expose local services behind firewalls
to the public internet via HTTPS, similar to ngrok.

## Architecture

- **CLI** (`rs-rok`): Rust binary that opens a WebSocket tunnel to Cloudflare and
  proxies traffic to a local port. The Cloudflare Worker is embedded in the binary
  and can be deployed directly to your account.
- **Worker**: Cloudflare Worker (TypeScript + Rust WASM) that brokers connections
  between internet clients and CLI tunnels via a Durable Object.
- **Protocol**: Shared binary framing crate (`no_std`-compatible) used by both the
  CLI and the WASM bridge inside the Worker.

## Quick Start

### 1. Build the CLI

```bash
# Build the worker bundle first (one-time, or after worker code changes)
cd worker && bun install && bun run build:bundle && cd ..

# Build the CLI (embeds the worker bundle)
cargo build --release -p rs-rok-cli
```

The resulting binary is at `target/release/rs-rok.exe` (Windows) or
`target/release/rs-rok` (Linux/macOS). It is fully self-contained.

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
rs-rok config set-cf-credentials \
  --account-id <your-account-id> \
  --api-token <your-api-token>

# Deploy the embedded Worker to your account
rs-rok deploy
```

On success, the CLI prints the public Worker URL and saves it as your endpoint.
You can also pass credentials inline or via environment variables:

```bash
# Inline flags
rs-rok deploy --account-id <id> --api-token <token>

# Environment variables
CF_ACCOUNT_ID=<id> CF_API_TOKEN=<token> rs-rok deploy
```

### 4. Expose a local service

```bash
# Expose local port 8080 over HTTP
rs-rok http 8080

# Expose with a stable tunnel name
rs-rok http 8080 --name myapp

# Expose a local HTTPS service
rs-rok https 8443
```

## Configuration

Settings are stored in `~/.rs-rok/`:

| File | Purpose |
|------|---------|
| `settings.json` | Endpoint URL, auth token, default region |
| `cloudflare.json` | Cloudflare account ID and API token |

### CLI Commands

```
rs-rok http <port>              Expose a local HTTP service
rs-rok https <port>             Expose a local HTTPS service
rs-rok deploy                   Deploy the Worker to Cloudflare
rs-rok config show              Print current configuration
rs-rok config add-token <tok>   Store an auth token
rs-rok config set-endpoint <url> Set the worker endpoint URL
rs-rok config set-cf-credentials Store Cloudflare credentials
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
