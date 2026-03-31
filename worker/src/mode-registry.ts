import type { Env } from "./index";

type Mode = "root" | "named";

/**
 * Singleton Durable Object that tracks whether the worker is in root mode
 * (one tunnel at /) or named mode (multiple tunnels at /:name/).
 *
 * Accessed via idFromName("__singleton__").
 *
 * Routes:
 *   POST /register   { mode }  → 200 { ok } | 409 { error }
 *   POST /unregister { mode }  → 200 { ok }
 *   GET  /mode                 → 200 { mode: "root" | "named" | null }
 */
export class ModeRegistry implements DurableObject {
  private state: DurableObjectState;

  constructor(state: DurableObjectState, _env: Env) {
    this.state = state;
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);

    if (url.pathname === "/mode") {
      const mode = (await this.state.storage.get<Mode>("mode")) ?? null;
      return Response.json({ mode });
    }

    if (url.pathname === "/register" && request.method === "POST") {
      const { mode } = await request.json<{ mode: Mode }>();
      return this.register(mode);
    }

    if (url.pathname === "/unregister" && request.method === "POST") {
      const { mode } = await request.json<{ mode: Mode }>();
      return this.unregister(mode);
    }

    return new Response("Not found", { status: 404 });
  }

  private async register(requested: Mode): Promise<Response> {
    const current = (await this.state.storage.get<Mode>("mode")) ?? null;
    const count = (await this.state.storage.get<number>("count")) ?? 0;

    if (current && current !== requested) {
      const error =
        current === "root"
          ? "Worker is in root mode — disconnect the root tunnel before using named tunnels."
          : "Worker is in named mode — disconnect all named tunnels before using root mode.";
      return Response.json({ error }, { status: 409 });
    }

    await this.state.storage.put("mode", requested);
    await this.state.storage.put("count", count + 1);
    return Response.json({ ok: true });
  }

  private async unregister(mode: Mode): Promise<Response> {
    const count = Math.max(0, ((await this.state.storage.get<number>("count")) ?? 1) - 1);
    if (count === 0) {
      await this.state.storage.delete("mode");
      await this.state.storage.delete("count");
    } else {
      await this.state.storage.put("count", count);
    }
    return Response.json({ ok: true });
  }
}
