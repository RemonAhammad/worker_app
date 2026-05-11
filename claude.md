# co_worker_cli — engineering map

Monorepo for every **client** that talks to the [`co_worker_lite`](../co_worker_lite) backend. Holds a CLI binary, a reusable Tauri plugin (Rust + npm package), and a reference desktop app built on that plugin. User-facing docs live in [README.md](README.md).

## Workspace layout

```text
co_worker_cli/
├── Cargo.toml                          # cargo workspace
├── package.json                        # npm workspaces
├── crates/
│   ├── co_worker_client/               # shared async HTTP client + wire types
│   ├── co_worker_cli/                  # terminal binary (clap + rustyline + colored)
│   └── tauri-plugin-co-worker/         # reusable Tauri plugin
│       ├── src/                        #   Rust: lib.rs, commands.rs, error.rs,
│       │                               #         workspace.rs, tools.rs
│       ├── guest-js/                   #   TS bindings (published as npm package)
│       └── permissions/default.toml    #   Tauri 2 permissions surface
└── apps/
    └── desktop/                        # Tauri 2 + Svelte 5 reference app
        ├── src/                        #   Svelte UI
        └── src-tauri/                  #   Tauri shell — plugin(init()) + dialog plugin
```

## Build, run

```sh
npm install                  # one-time, sets up workspaces
npm run plugin:build         # tsc on the plugin's guest-js → dist-js
npm run desktop:dev          # tauri dev (opens the window)
cargo run -p co_worker_cli   # terminal client
cargo build --workspace      # all four crates
cargo test --workspace       # plugin sandbox + workspace tests
```

## Architecture conventions

- **One Rust client, many consumers.** [`crates/co_worker_client`](crates/co_worker_client/src/lib.rs) is the single source of truth for the HTTP surface and wire types. Both the CLI and the Tauri plugin depend on it.
- **Tauri plugin = the reusable surface.** [`crates/tauri-plugin-co-worker`](crates/tauri-plugin-co-worker) bundles Rust commands and matching TypeScript bindings (`guest-js/index.ts`) into one publishable unit. Three sub-surfaces:
  1. **Chat passthroughs** — `health`, `model_catalog`, `load_model`, `list_sessions`, `create_session`, `update_session`, `chat`, `send_message`, … each a thin wrapper over `co_worker_client`.
  2. **Workspace + sandboxed FS tools** — `set_workspace`, `get_workspace`, plus seven tool commands (`tool_list_dir`, `tool_read_file`, `tool_write_file`, `tool_append_file`, `tool_delete_path`, `tool_move_path`, `tool_create_dir`, `tool_search`) and a helper `tool_preview_write` that produces the diff shown in the approval card.
  3. **Agent loop + persistent allow-list** — `agent_send`, `agent_continue`, `get_auto_allow`, `set_auto_allow`. The plugin holds in-memory `WorkspaceState` and `AllowListState` and persists both to the Tauri `app_data_dir`.
- **Path sandbox is the trust boundary.** Every FS tool routes through [`workspace::resolve`](crates/tauri-plugin-co-worker/src/workspace.rs). Inputs are canonicalized against the workspace root; paths that escape (via `..` or absolute paths leading elsewhere) hit `Error::ToolOutsideWorkspace` and never touch disk. The workspace root itself is protected from deletion.
- **Commands hold `Arc<PluginState>`.** Each `#[tauri::command]` is `pub async fn` (no `R: Runtime` generic — that breaks `generate_handler!`). They briefly lock the client mutex, clone the cheap reqwest client (internal `Arc`s), release, then `.await` — never holding the lock across an HTTP call.
- **Error type crosses the bridge as `{ kind, message }`.** [`tauri-plugin-co-worker/src/error.rs`](crates/tauri-plugin-co-worker/src/error.rs) hand-implements `Serialize` so the TS side gets a stable shape. The frontend's `formatError` in [`stores.ts`](apps/desktop/src/lib/stores.ts) parses it.
- **Frontend is component + stores, no router.** Single-window app. [`src/lib/stores.ts`](apps/desktop/src/lib/stores.ts) holds every reactive piece of state and every mutation helper; components only read stores and call helpers.

## Agent loop on the client side

1. User sends a message. [`sendInActive`](apps/desktop/src/lib/stores.ts) routes to **`runAgentLoop`** when a workspace is set, **`runStreamingChat`** otherwise.
2. `runAgentLoop` calls `agent_send`. If the response is `kind: "message"`, push it and return.
3. If `kind: "tool_calls"`, for each call:
   - **Read-only tools (`list_dir`, `read_file`, `search`)** auto-execute.
   - **Mutating tools (`write_file`, `append_file`, `delete_path`, `move_path`, `create_dir`)** check the persistent allow-list and the per-turn `autoApprove` flag; if neither pre-approves, push an inline Allow/Deny card. For `write_file`/`append_file`, fetch a diff preview via `tool_preview_write` first so the card shows what will change.
   - Run the tool through the plugin's sandboxed command. Stash the `ToolResultPayload`.
4. POST `agent_continue` with the result batch. Loop until `kind: "message"` or the **10-round cap** kicks in.

`runStreamingChat` opens an SSE stream against the backend's `/v1/sessions/:id/messages/stream`, appends incoming tokens to a placeholder assistant bubble in real time, and swaps the placeholder for the persisted row on the `done` event. If the backend doesn't speak SSE (returns 404 or network error), it falls back to the non-streaming `sendMessage`.

## Adding a new endpoint

1. **Backend** — add route + handler in `co_worker_lite/src/api/`.
2. **Shared Rust client** — add the method on `Client` in [`crates/co_worker_client/src/lib.rs`](crates/co_worker_client/src/lib.rs) plus any new request/response types (derive both `Serialize` and `Deserialize`).
3. **Tauri plugin Rust** — add a `#[tauri::command]` wrapper in [`crates/tauri-plugin-co-worker/src/commands.rs`](crates/tauri-plugin-co-worker/src/commands.rs) and list it in `tauri::generate_handler!` in [`lib.rs`](crates/tauri-plugin-co-worker/src/lib.rs).
4. **Tauri plugin build.rs** — append the command name to the `COMMANDS` array in [`build.rs`](crates/tauri-plugin-co-worker/build.rs).
5. **Tauri plugin permissions** — append `allow-<command_name>` to [`permissions/default.toml`](crates/tauri-plugin-co-worker/permissions/default.toml).
6. **TS bindings** — add the typed wrapper in [`guest-js/index.ts`](crates/tauri-plugin-co-worker/guest-js/index.ts). Mirror the Rust shape one-for-one.
7. **(Optional) CLI** — add a subcommand in [`crates/co_worker_cli/src/main.rs`](crates/co_worker_cli/src/main.rs).
8. **(Optional) Desktop UI** — add a store helper in [`apps/desktop/src/lib/stores.ts`](apps/desktop/src/lib/stores.ts) and wire it into a component.

## Adding a new agent tool

1. **Backend tool declaration** — append a `ToolDefinition` to `filesystem_tools()` in [`co_worker_lite/src/tools/mod.rs`](../co_worker_lite/src/tools/mod.rs). Backend never executes tools; it only describes them to the model.
2. **Plugin Rust impl** — add to [`crates/tauri-plugin-co-worker/src/tools.rs`](crates/tauri-plugin-co-worker/src/tools.rs). Always call `workspace::resolve` first.
3. **Plugin Tauri command** — wrap it in [`commands.rs`](crates/tauri-plugin-co-worker/src/commands.rs) and register everywhere (lib.rs handler list, build.rs COMMANDS, permissions/default.toml).
4. **TS binding** — add to [`guest-js/index.ts`](crates/tauri-plugin-co-worker/guest-js/index.ts).
5. **Desktop dispatcher** — add a `case` in `runTool` inside [`apps/desktop/src/lib/stores.ts`](apps/desktop/src/lib/stores.ts).
6. **Permission policy** — decide read-only vs mutating in `isMutating` (same file). Mutating tools get the approval card automatically.

## Tauri 2 specifics

- Commands are plain `pub async fn` — **no `R: Runtime` generic**. `generate_handler!` can't infer generics. The exception is two commands that need the `AppHandle` for persistence (`set_workspace`, `set_auto_allow`) — those *do* take `R: Runtime` because `AppHandle<R>` is the parameter.
- Permissions live in [`permissions/default.toml`](crates/tauri-plugin-co-worker/permissions/default.toml). `build.rs` runs `tauri_plugin::Builder::new(COMMANDS).build()` which auto-generates per-command `allow-*` / `deny-*` permissions under `permissions/autogenerated/`. The `default` bundle then references them.
- The desktop app grants the bundles via [`apps/desktop/src-tauri/capabilities/default.json`](apps/desktop/src-tauri/capabilities/default.json) → `"permissions": ["core:default", "co-worker:default", "dialog:default", "dialog:allow-open"]`. The dialog plugin is registered in [`apps/desktop/src-tauri/src/lib.rs`](apps/desktop/src-tauri/src/lib.rs) so the workspace picker can open a folder chooser.
- **Placeholder icons.** `icon.icns` is a real ~54 KB file (regenerated via `iconutil`). For Windows builds you'll need `icon.ico` — run `npm --workspace co_worker_desktop run tauri icon <source.png>` to regenerate the full set.

## Persistent state files

The plugin writes JSON in the Tauri app-data dir (`$HOME/Library/Application Support/dev.co-worker.desktop/`):

- `co_worker_workspace.json` — currently selected workspace root.
- `co_worker_autoallow.json` — list of tool names the user has marked "always allow".

Both are best-effort: the in-memory state is authoritative; corrupt files are ignored at startup.

## Streaming on the client side

The desktop uses **plain `fetch` against the backend's SSE endpoint** rather than going through the plugin. The flow:

1. UI calls `runStreamingChat`.
2. We call the plugin's `getBaseUrl` to learn the backend URL.
3. POST to `${base}/v1/sessions/:id/messages/stream` with `Accept: text/event-stream`.
4. Parse the SSE frames (each event terminated by a blank line, lines starting `data:` carry JSON).
5. `{type:"token",text}` events append to a placeholder bubble; `{type:"done",message,usage}` swaps the placeholder for the persisted row; `{type:"error",...}` raises.

This keeps the plugin command surface clean (no command needs to return a stream).

## Svelte 5 specifics

- Components use the **runes API** (`$props`, `$state`, `$derived`, `$effect`) — not `export let` or `$:`.
- `onMount` callbacks that need cleanup must be **synchronous** with the cleanup returned directly. The "kick off async then return cleanup" pattern lives in [`App.svelte`](apps/desktop/src/App.svelte) `onMount`.
- The list row in [`Sidebar.svelte`](apps/desktop/src/lib/components/Sidebar.svelte) is a `<div role="button" tabindex="0">`, **not** a `<button>`, because it contains a nested delete button.
- Inside a discriminated-union narrow, store the narrowed value in a local `const` before using it inside a closure — TypeScript loses the narrowing across closure boundaries (see `runAgentLoop` in [`stores.ts`](apps/desktop/src/lib/stores.ts)).

## Trade-offs worth knowing

- **No streaming for the agent loop.** Tool calls become visible only after that turn's generation finishes. The non-agent path *is* streamed. When/if the backend exposes agent streaming, only `runAgentLoop` in [`stores.ts`](apps/desktop/src/lib/stores.ts) and the SSE-event union shape change.
- **Diff preview is hand-rolled LCS.** Good enough for the small/medium files the model writes; falls back to a summary line for >2M-cell tables.
- **Search is plain substring, not regex.** Walks the workspace tree, skips hidden / `node_modules` / `target` / `dist` / `dist-js`, caps at ~1000 matches and 4 MB per file, refuses files containing NUL in the first 8 KiB.
- **Optimistic UI.** The user message is pushed into `activeMessages` before the request returns; on failure it's removed and the error surfaces in `lastError`. Streaming uses the same pattern with a placeholder assistant bubble.
- **Wire types are duplicated by hand on the TS side.** The Rust types in `co_worker_client` and the TS interfaces in `guest-js/index.ts` are not generated from each other. A code-gen step (e.g. `ts-rs`) would close that gap but adds toolchain weight.

## Pinning a model to a session

`PATCH /v1/sessions/:id { model_name }` on the backend stores the preference. The desktop's `pinSessionModel` calls that endpoint directly via `fetch` (the plugin's `update_session` TS binding doesn't currently expose `model_name` — adding it is a one-line change). On the next message/chat/agent send, the backend's handler calls `ensure_model_loaded(&state, &session.model_name)`, which hot-swaps the engine if the pinned filename differs from what's loaded.

## Testing

- Plugin: 4 unit tests in [`crates/tauri-plugin-co-worker/src/workspace.rs`](crates/tauri-plugin-co-worker/src/workspace.rs) cover path-sandbox escape attempts.
- Frontend: no automated UI tests yet. svelte-check is the typecheck floor.
- CLI: no automated tests — relies on the backend's `tests/api_test.rs` for the wire surface.
