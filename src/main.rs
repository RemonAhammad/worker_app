//! co_worker_cli — interactive client for the co_worker_lite backend.
//!
//! Default subcommand is `chat`, which opens a REPL backed by a freshly
//! created session. Other subcommands cover one-shot prompts, session
//! management, and a health check. The backend URL defaults to the address
//! the lite project listens on locally; override with `--url` or
//! `CO_WORKER_URL`.

mod chat;
mod client;
mod types;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::Colorize;
use uuid::Uuid;

use crate::chat::{ChatOptions, run as run_chat};
use crate::client::Client;

/// Default backend address; matches the co_worker_lite default.
const DEFAULT_URL: &str = "http://localhost:6969";

#[derive(Parser, Debug)]
#[command(
    name = "co_worker_cli",
    version,
    about = "CLI client for the co_worker_lite backend",
    disable_help_subcommand = true
)]
struct Cli {
    /// Base URL of the co_worker_lite backend.
    #[arg(long, env = "CO_WORKER_URL", default_value = DEFAULT_URL, global = true)]
    url: String,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Open an interactive chat REPL (default).
    Chat {
        /// Resume an existing session by id instead of creating a new one.
        #[arg(long, value_name = "UUID")]
        resume: Option<Uuid>,
        /// Set the system prompt on a new session.
        #[arg(long, value_name = "TEXT")]
        system: Option<String>,
        /// Set the title of the new session.
        #[arg(long, value_name = "TEXT")]
        title: Option<String>,
        /// Cap on tokens generated per reply.
        #[arg(long, default_value_t = 1024)]
        max_tokens: u32,
        /// Sampling temperature; 0.0 = greedy.
        #[arg(long, default_value_t = 0.7)]
        temperature: f32,
    },
    /// Send a single prompt and print the reply, then exit.
    Ask {
        /// Prompt to send. If omitted, read from stdin.
        prompt: Option<String>,
        /// Reuse an existing session instead of creating a one-off.
        #[arg(long, value_name = "UUID")]
        session: Option<Uuid>,
        /// System prompt for the one-off session.
        #[arg(long, value_name = "TEXT")]
        system: Option<String>,
        #[arg(long, default_value_t = 1024)]
        max_tokens: u32,
        #[arg(long, default_value_t = 0.7)]
        temperature: f32,
    },
    /// Manage chat sessions.
    Sessions {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Ping the backend and print the loaded model.
    Health,
    /// List models present in the backend's models directory.
    Models,
}

#[derive(Subcommand, Debug)]
enum SessionAction {
    /// List recent sessions.
    List {
        #[arg(long, default_value_t = 20)]
        limit: i64,
        #[arg(long, default_value_t = 0)]
        offset: i64,
    },
    /// Show a session and its full message history.
    Show { id: Uuid },
    /// Delete a session.
    Delete { id: Uuid },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new(&cli.url)?;

    match cli.command.unwrap_or(Command::Chat {
        resume: None,
        system: None,
        title: None,
        max_tokens: 1024,
        temperature: 0.7,
    }) {
        Command::Chat {
            resume,
            system,
            title,
            max_tokens,
            temperature,
        } => {
            preflight(&client).await?;
            run_chat(
                client,
                ChatOptions {
                    resume,
                    system_prompt: system,
                    title,
                    max_tokens,
                    temperature,
                },
            )
            .await
        }
        Command::Ask {
            prompt,
            session,
            system,
            max_tokens,
            temperature,
        } => {
            let prompt = match prompt {
                Some(p) => p,
                None => read_stdin().context("reading prompt from stdin")?,
            };
            cmd_ask(client, prompt, session, system, max_tokens, temperature).await
        }
        Command::Sessions { action } => match action {
            SessionAction::List { limit, offset } => cmd_list_sessions(client, limit, offset).await,
            SessionAction::Show { id } => cmd_show_session(client, id).await,
            SessionAction::Delete { id } => cmd_delete_session(client, id).await,
        },
        Command::Health => cmd_health(client).await,
        Command::Models => cmd_models(client).await,
    }
}

async fn preflight(client: &Client) -> Result<()> {
    match client.health().await {
        Ok(h) => {
            println!(
                "{} {} {} {}",
                "Connected".green().bold(),
                client.base_url().dimmed(),
                "•".dimmed(),
                h.model.bold()
            );
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "{} could not reach backend at {}: {e:#}",
                "error:".red().bold(),
                client.base_url()
            );
            eprintln!(
                "{} is the lite backend running? try `cargo run --release` in co_worker_lite",
                "hint:".yellow().bold()
            );
            Err(e)
        }
    }
}

async fn cmd_ask(
    client: Client,
    prompt: String,
    session: Option<Uuid>,
    system: Option<String>,
    max_tokens: u32,
    temperature: f32,
) -> Result<()> {
    preflight(&client).await?;
    let session_id = match session {
        Some(id) => id,
        None => {
            let s = client
                .create_session(
                    &format!("ask {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")),
                    system.as_deref(),
                )
                .await?;
            s.id
        }
    };

    let resp = client
        .send_message(session_id, &prompt, max_tokens, temperature)
        .await?;
    println!("{}", resp.message.content);
    eprintln!(
        "{}",
        format!(
            "({} prompt + {} completion = {} total tokens)",
            resp.usage.prompt_tokens, resp.usage.completion_tokens, resp.usage.total_tokens
        )
        .dimmed()
    );
    Ok(())
}

async fn cmd_list_sessions(client: Client, limit: i64, offset: i64) -> Result<()> {
    let sessions = client.list_sessions(limit, offset).await?;
    chat::print_sessions_table(&sessions);
    Ok(())
}

async fn cmd_show_session(client: Client, id: Uuid) -> Result<()> {
    let s = client.get_session(id).await?;
    println!(
        "{} {}\n{} {}\n{} {}",
        "id     ".dimmed(),
        s.session.id,
        "title  ".dimmed(),
        s.session.title,
        "model  ".dimmed(),
        s.session.model_name
    );
    if let Some(prompt) = &s.session.system_prompt {
        println!("{} {}", "system ".dimmed(), prompt);
    }
    println!("{} {}", "msgs   ".dimmed(), s.messages.len());
    println!();
    for m in &s.messages {
        let label = match m.role {
            types::Role::System => "system".magenta().bold(),
            types::Role::User => "user".cyan().bold(),
            types::Role::Assistant => "assistant".green().bold(),
            types::Role::Tool => "tool".yellow().bold(),
        };
        println!("{label} {}", format!("({}t)", m.token_count).dimmed());
        println!("{}\n", m.content);
    }
    Ok(())
}

async fn cmd_delete_session(client: Client, id: Uuid) -> Result<()> {
    client.delete_session(id).await?;
    println!("{} {id}", "deleted".green().bold());
    Ok(())
}

async fn cmd_health(client: Client) -> Result<()> {
    let h = client.health().await?;
    println!(
        "{} model={} loaded={}",
        h.status.green().bold(),
        h.model.bold(),
        h.loaded
    );
    Ok(())
}

async fn cmd_models(client: Client) -> Result<()> {
    let resp = client.list_models().await?;
    if resp.models.is_empty() {
        println!("{}", "(no models present)".dimmed());
        return Ok(());
    }
    for m in resp.models {
        let marker = if m.loaded { "●".green() } else { "○".dimmed() };
        let size_mb = m.size_bytes as f64 / 1024.0 / 1024.0;
        println!(
            "{marker} {} {}",
            m.name.bold(),
            format!("({size_mb:.1} MiB)").dimmed()
        );
    }
    Ok(())
}

fn read_stdin() -> Result<String> {
    use std::io::Read;
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s)?;
    Ok(s)
}
