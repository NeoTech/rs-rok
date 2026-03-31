# Tunnel Quickstart

Run each command in its own terminal from the project root.

## Terminal 1 — Mock Service

    cd /c/Users/andre/Projects/rs-rok
    ./target/release/mock-service --port 3000

## Terminal 2 — Traefik

    cd /c/Users/andre/Projects/rs-rok
    ./traefik.exe --configFile=traefik.yml

## Terminal 3 — Tunnel

    cd /c/Users/andre/Projects/rs-rok
    ./target/release/rs-rok http 9000 --name myapp

Wait until you see:

    Tunnel:     https://rs-rok-worker.andreas-016.workers.dev/tunnel/myapp
    Forwarding: http://localhost:9000

## Terminal 4 — Test

    curl https://rs-rok-worker.andreas-016.workers.dev/tunnel/myapp/health

Expected response: ok

    curl -X POST https://rs-rok-worker.andreas-016.workers.dev/tunnel/myapp/echo -H "Content-Type: application/json" -d "{\"hello\":\"world\"}"

Expected response: JSON echoing the body back.

## Notes

- The URL https://rs-rok-worker.andreas-016.workers.dev/tunnel/myapp is stable — same every restart.
- Edit traefik-routes.yml to add more services / path rules. Traefik hot-reloads it.
