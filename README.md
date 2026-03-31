# rs-rok

A Cloudflare-backed reverse tunnel tool. Expose local services behind firewalls
to the public internet via HTTPS, similar to ngrok.

## Architecture

- **CLI** (`rs-rok`): Rust binary that opens a WebSocket tunnel to Cloudflare and
  proxies traffic to a local port.
- **Worker**: Cloudflare Worker (TypeScript + Rust WASM) that brokers connections
  between internet clients and CLI tunnels via a Durable Object.
- **Protocol**: Shared binary framing crate (`no_std`-compatible) used by both the
  CLI and the WASM bridge inside the Worker.

## Quick Start

```bash
# Build the CLI
cargo build --release -p rs-rok-cli

# Configure
rs-rok config add-token <your-token>

# Expose local port 8080
rs-rok http 8080
```

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
```

## License

MIT
