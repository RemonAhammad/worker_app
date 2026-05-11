# co_worker_cli

Interactive CLI client for the [co_worker_lite](../co_worker_lite) local LLM backend. User-facing docs live in [README.md](README.md); this file is the engineering map.

## Scope

Thin async HTTP client + a rustyline REPL. No persistence beyond the input-history file at `~/.co_worker_cli_history`. State (sessions, messages) lives in the lite backend — the CLI is stateless.

## Build, run, test

```sh
cargo build
cargo run --release                   # opens chat REPL against localhost:6969
cargo run --release -- ask "..."      # one-shot
CO_WORKER_URL=http://host:6969 cargo run --release
```

The CLI does not have automated tests. Smoke-test paths: `health`, `models`, `sessions list`, `ask`, `chat` (manual). All depend on a running lite backend.

## Layout

- [src/main.rs](src/main.rs) — clap parser, subcommand dispatch, preflight health check, one-shot `ask`, session management commands.
- [src/client.rs](src/client.rs) — async `reqwest::Client` wrapper. All public methods return `anyhow::Result<T>`. On non-2xx, the backend's `{"error":{...}}` body is parsed and surfaced in the message. 600-second timeout because inference can be slow.
- [src/chat.rs](src/chat.rs) — interactive REPL. Owns the rustyline `Editor`, dispatches slash commands, renders output with `colored`. Persists input history.
- [src/types.rs](src/types.rs) — wire types that match `co_worker_lite::types`. Deliberately duplicated rather than imported from the backend crate so the CLI doesn't drag in llama-cpp-2 / sqlx.

## Conventions

- **`anyhow::Result` everywhere**, including in library modules. The CLI is a single binary, so no need for the structured `thiserror` enum the backend uses.
- **`colored::Colorize` for terminal output.** Colors auto-disable when stdout isn't a TTY, so piping to files / other tools produces plain text.
- **No streaming yet.** The backend doesn't expose streaming in iteration 1; the CLI just blocks on the response. Add when the backend gains it.
- **Backend URL precedence** (highest first): `--url`, `CO_WORKER_URL` env var, `DEFAULT_URL` constant in [src/main.rs](src/main.rs:23) (currently `http://localhost:6969`).

## Wire-type drift

If [co_worker_lite/src/types.rs](../co_worker_lite/src/types.rs) changes:

- Adding fields to existing structs is safe — `serde` ignores unknown fields by default on the CLI side, and missing fields use `#[serde(default)]` where present.
- Renaming or changing a field's JSON shape requires the corresponding edit in [src/types.rs](src/types.rs). There is no compile-time link between the two crates.
- New endpoints → add a method to [src/client.rs](src/client.rs) (`get_json` / `post_json` helpers cover most shapes) and a subcommand or REPL command if it's user-facing.

## Slash command pattern

Commands are matched in [src/chat.rs](src/chat.rs) `handle_slash_command`. To add one:

1. Add the match arm.
2. Add a row in `print_help()`.
3. Document in [README.md](README.md).

Return `ControlFlow::Quit` to break the REPL loop, `ControlFlow::Continue` to keep it running.

## When making changes

- New subcommand → extend the `Command` enum in [src/main.rs](src/main.rs) and add a `cmd_*` async fn.
- New REPL slash command → see "Slash command pattern" above.
- Auth / headers (when the backend grows them) → add a builder param to `Client::new` in [src/client.rs](src/client.rs) and thread through `--token` / `CO_WORKER_TOKEN`.
- Streaming (when the backend exposes it) → switch `send_message` to consume an SSE stream; render tokens incrementally in [src/chat.rs](src/chat.rs).
