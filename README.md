# co_worker_cli

Monorepo for the **co_worker** client: a CLI, a reusable Tauri plugin, and a
reference desktop app — all talking to the
[`co_worker_lite`](../co_worker_lite) local LLM backend.

```text
co_worker_cli/
├── crates/
│   ├── co_worker_client/           # shared async HTTP client (Rust)
│   ├── co_worker_cli/              # terminal binary (rustyline REPL + subcommands)
│   └── tauri-plugin-co-worker/     # reusable Tauri plugin (Rust + npm package)
└── apps/
    └── desktop/                    # reference Tauri 2 + Svelte 5 desktop app
        ├── src/                    #   Svelte UI (Sidebar, ChatView, …)
        └── src-tauri/              #   Tauri shell
```

## Quick start

Make sure the backend is running first (defaults to port 6969):

```sh
cd ../co_worker_lite
cargo run --release
```

Then in this repo:

```sh
npm install                 # workspaces: plugin TS bindings + desktop app
npm run plugin:build        # compile the plugin's TypeScript bindings
npm run desktop:dev         # launch the Tauri desktop app

# or use the CLI:
cargo run -p co_worker_cli  # interactive REPL
```

## Desktop app

A two-pane Tauri desktop app:

- **Left rail** — conversation list (newest first). Click `+ New chat` for a
  fresh session, click any row to switch, hover for a delete button.
- **Main area** — chat view with optimistic user messages, a "thinking…"
  indicator while the model generates, and a sticky bottom composer
  (`Enter` to send, `Shift+Enter` for newline).
- **Memories drawer** — opened from the header. Manual + auto-extracted
  facts that the backend injects into every conversation's system prompt.
- **Status bar** — model name and backend health, refreshed every 15s.

All wire calls are typed via `tauri-plugin-co-worker` (see below).

## Using the Tauri plugin in **another** project

The plugin is the modular surface. In any Tauri 2 app:

```toml
# their Cargo.toml
[dependencies]
tauri-plugin-co-worker = { path = "../co_worker_cli/crates/tauri-plugin-co-worker" }
# (or, once published: tauri-plugin-co-worker = "0.1")
```

```rust
// their src-tauri/src/lib.rs
fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_co_worker::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

```jsonc
// their src-tauri/capabilities/default.json
{
  "permissions": ["core:default", "co-worker:default"]
}
```

```ts
// their frontend
import { health, sendMessage, listSessions } from 'tauri-plugin-co-worker'

const h = await health()              // { status, model, loaded }
const ss = await listSessions()       // Session[]
const r = await sendMessage(ss[0].id, 'hello')   // { message, usage }
```

The plugin defaults to `http://localhost:6969` and can be retargeted at
runtime via `setBaseUrl('https://...')`. The `CO_WORKER_URL` env var
overrides the default at app startup.

## CLI

```sh
co_worker_cli                       # interactive REPL (default)
co_worker_cli ask "explain rust borrow checker"
co_worker_cli sessions list
co_worker_cli sessions show <id>
co_worker_cli sessions debug <id>   # what the model will actually see
co_worker_cli sessions delete <id>
co_worker_cli memories list
co_worker_cli memories add "my name is Rimon"
co_worker_cli memories delete <id>
co_worker_cli health
co_worker_cli models
```

Defaults to `http://localhost:6969`; override with `--url` or
`CO_WORKER_URL`. Input history persists at `~/.co_worker_cli_history`.

## Scripts

```sh
npm run plugin:build        # build TypeScript bindings of the plugin
npm run desktop:dev         # tauri dev (opens the desktop window)
npm run desktop:build       # production bundle (needs real icons — see below)
cargo build --workspace     # all four crates
cargo run -p co_worker_cli  # CLI
```

## Notes

- **Placeholder icons.** `apps/desktop/src-tauri/icons/icon.icns` and
  `icon.ico` are zero-byte placeholders that satisfy the bundle config for
  `tauri dev`. Before `tauri build`, regenerate full icon sets from a
  source PNG with `npm --workspace co_worker_desktop run tauri icon path/to/source.png`.
- **No streaming.** The backend doesn't stream replies yet, so the UI shows
  a thinking indicator until the full response arrives. When the backend
  gains streaming, only `sendInActive`/`chat` in
  [`apps/desktop/src/lib/stores.ts`](apps/desktop/src/lib/stores.ts) needs
  to change.
- **Shared types.** The wire shapes live in
  [`crates/co_worker_client/src/lib.rs`](crates/co_worker_client/src/lib.rs)
  on the Rust side and
  [`crates/tauri-plugin-co-worker/guest-js/index.ts`](crates/tauri-plugin-co-worker/guest-js/index.ts)
  on the TypeScript side. They must move together when the backend changes.

## License

MIT OR Apache-2.0.
