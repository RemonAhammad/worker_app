# co_worker_cli

Interactive terminal client for the [co_worker_lite](../co_worker_lite) local
LLM backend. Open a chat REPL, send one-shot prompts, manage sessions.

## Build & run

```sh
cargo run --release
```

Defaults to `http://localhost:6969` (the lite backend's default). Override
with `--url` or the `CO_WORKER_URL` env var:

```sh
CO_WORKER_URL=http://192.168.1.10:6969 co_worker_cli
co_worker_cli --url http://localhost:9090 chat
```

## Subcommands

```
co_worker_cli                       open interactive chat (default)
co_worker_cli chat                  same, with options
co_worker_cli ask "prompt"          one-shot — send and exit
co_worker_cli sessions list         list recent sessions
co_worker_cli sessions show <id>    print full message history
co_worker_cli sessions delete <id>  delete a session
co_worker_cli health                ping the backend
co_worker_cli models                list models in the backend's models dir
```

`co_worker_cli --help` and `co_worker_cli <subcommand> --help` show every flag.

## Interactive chat

```sh
co_worker_cli chat --system "You are a helpful coding assistant." \
                   --temperature 0.7 --max-tokens 1024
```

Once inside:

```
› your message
assistant (42 prompt, 88 completion, 1.4s)
... reply ...
```

Slash commands inside the REPL:

| Command       | Action                                          |
| ------------- | ----------------------------------------------- |
| `/help`       | show command list                               |
| `/session`    | show current session id, title, model, created  |
| `/sessions`   | list recent sessions                            |
| `/health`     | ping the backend                                |
| `/clear`      | clear the screen                                |
| `/exit`       | quit (also Ctrl-D)                              |

History (your past prompts) is persisted to `~/.co_worker_cli_history`
across runs — use ↑/↓ to recall.

## One-shot

```sh
co_worker_cli ask "Explain Rust's borrow checker in two sentences."

# pipe a long prompt
cat prompt.txt | co_worker_cli ask

# reuse a session for context
co_worker_cli ask --session <uuid> "and what about Send/Sync?"
```

## Resuming a session

```sh
co_worker_cli sessions list
co_worker_cli chat --resume <uuid>      # past history is replayed
```

## Project layout

```
co_worker_cli/
├── Cargo.toml
├── claude.md
└── src/
    ├── main.rs    # clap parsing + subcommand dispatch
    ├── client.rs  # async HTTP client
    ├── chat.rs    # interactive REPL (rustyline + colored)
    └── types.rs   # wire types matching the backend API
```

## Notes

- Inference can take a while; the HTTP client uses a 600-second timeout.
- Color output auto-disables when stdout isn't a TTY, so piping into other
  tools produces clean, plain text.

## License

MIT OR Apache-2.0.
