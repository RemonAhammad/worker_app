//! Interactive chat REPL.
//!
//! Owns the rustyline editor and the active session. Each user line is sent
//! as a message; the assistant's reply is rendered with a colored prefix and
//! a usage summary.

use anyhow::Result;
use co_worker_client::{Client, Message, Role, Session};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::{Config, EditMode, Editor};
use uuid::Uuid;

pub struct ChatOptions {
    pub max_tokens: u32,
    pub temperature: f32,
    pub system_prompt: Option<String>,
    pub title: Option<String>,
    pub resume: Option<Uuid>,
}

pub async fn run(client: Client, opts: ChatOptions) -> Result<()> {
    let session: Session = match opts.resume {
        Some(id) => {
            let existing = client.get_session(id).await?;
            println!(
                "{} {} ({} messages)",
                "Resumed".green().bold(),
                existing.title.bold(),
                existing.messages.len()
            );
            for m in &existing.messages {
                render_history_entry(m);
            }
            Session {
                id: existing.id,
                title: existing.title,
                model_name: existing.model_name,
                system_prompt: existing.system_prompt,
                created_at: existing.created_at,
                updated_at: existing.updated_at,
                metadata: existing.metadata,
            }
        }
        None => {
            let title = opts
                .title
                .clone()
                .unwrap_or_else(|| chrono::Local::now().format("chat %Y-%m-%d %H:%M").to_string());
            let session = client
                .create_session(&title, opts.system_prompt.as_deref())
                .await?;
            println!(
                "{} session {} ({})",
                "New".green().bold(),
                session.id.to_string().dimmed(),
                session.title.bold()
            );
            session
        }
    };

    print_help_hint();

    let mut editor = build_editor()?;
    let history_path = history_file_path();
    if let Some(p) = history_path.as_deref() {
        let _ = editor.load_history(p);
    }

    loop {
        let prompt = format!("{} ", "›".cyan().bold());
        let line = match editor.readline(&prompt) {
            Ok(l) => l,
            Err(ReadlineError::Interrupted) => {
                println!("{}", "(interrupted; type /exit to quit)".dimmed());
                continue;
            }
            Err(ReadlineError::Eof) => {
                println!("{}", "(eof, exiting)".dimmed());
                break;
            }
            Err(e) => {
                eprintln!("{} {e}", "input error:".red());
                break;
            }
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let _ = editor.add_history_entry(trimmed);

        if let Some(stripped) = trimmed.strip_prefix('/') {
            match handle_slash_command(stripped, &session, &client).await? {
                ControlFlow::Continue => continue,
                ControlFlow::Quit => break,
            }
        }

        if let Err(e) = send_one(&client, &session, trimmed, opts.max_tokens, opts.temperature)
            .await
        {
            eprintln!("{} {e:#}", "error:".red().bold());
        }
    }

    if let Some(p) = history_path.as_deref() {
        let _ = editor.save_history(p);
    }
    Ok(())
}

async fn send_one(
    client: &Client,
    session: &Session,
    content: &str,
    max_tokens: u32,
    temperature: f32,
) -> Result<()> {
    let started = std::time::Instant::now();
    let resp = client
        .send_message(session.id, content, max_tokens, temperature)
        .await?;
    let elapsed = started.elapsed();
    println!(
        "{} {}",
        "assistant".green().bold(),
        format!(
            "({} prompt, {} completion, {:.1}s)",
            resp.usage.prompt_tokens,
            resp.usage.completion_tokens,
            elapsed.as_secs_f32()
        )
        .dimmed()
    );
    println!("{}\n", resp.message.content);
    Ok(())
}

fn render_history_entry(m: &Message) {
    match m.role {
        Role::User => {
            println!("{} {}", "›".cyan().bold(), m.content);
        }
        Role::Assistant => {
            println!("{}", "assistant".green().bold());
            println!("{}\n", m.content);
        }
        Role::System => {
            println!("{} {}", "system:".magenta().bold(), m.content.dimmed());
        }
        Role::Tool => {
            println!("{} {}", "tool:".yellow().bold(), m.content.dimmed());
        }
    }
}

enum ControlFlow {
    Continue,
    Quit,
}

async fn handle_slash_command(
    rest: &str,
    session: &Session,
    client: &Client,
) -> Result<ControlFlow> {
    let mut parts = rest.split_whitespace();
    let cmd = parts.next().unwrap_or("").to_lowercase();
    match cmd.as_str() {
        "exit" | "quit" | "q" => Ok(ControlFlow::Quit),
        "help" | "?" => {
            print_help();
            Ok(ControlFlow::Continue)
        }
        "session" | "info" => {
            println!(
                "{} {}\n{} {}\n{} {}\n{} {}",
                "id      ".dimmed(),
                session.id,
                "title   ".dimmed(),
                session.title,
                "model   ".dimmed(),
                session.model_name,
                "created ".dimmed(),
                session.created_at,
            );
            Ok(ControlFlow::Continue)
        }
        "sessions" => {
            let sessions = client.list_sessions(20, 0).await?;
            print_sessions_table(&sessions);
            Ok(ControlFlow::Continue)
        }
        "health" => {
            let h = client.health().await?;
            println!(
                "{} model={} loaded={}",
                h.status.green().bold(),
                h.model.bold(),
                h.loaded
            );
            Ok(ControlFlow::Continue)
        }
        "clear" => {
            // ANSI clear screen + cursor home.
            print!("\x1B[2J\x1B[1;1H");
            Ok(ControlFlow::Continue)
        }
        "" => Ok(ControlFlow::Continue),
        other => {
            eprintln!(
                "{} unknown command /{other} — try /help",
                "?".yellow().bold()
            );
            Ok(ControlFlow::Continue)
        }
    }
}

fn print_help_hint() {
    println!(
        "{} {} {}",
        "Type your message and press enter.".dimmed(),
        "/help".cyan(),
        "for commands. Ctrl-D exits.".dimmed()
    );
}

fn print_help() {
    println!(
        "{}\n  {}  show this help\n  {}  show current session info\n  {}  list recent sessions\n  {}  ping the backend\n  {}  clear the screen\n  {}  exit (also Ctrl-D)",
        "commands".bold(),
        "/help".cyan(),
        "/session".cyan(),
        "/sessions".cyan(),
        "/health".cyan(),
        "/clear".cyan(),
        "/exit".cyan(),
    );
}

pub fn print_sessions_table(sessions: &[Session]) {
    if sessions.is_empty() {
        println!("{}", "(no sessions)".dimmed());
        return;
    }
    for s in sessions {
        println!(
            "{}  {}  {}",
            s.id.to_string().dimmed(),
            s.updated_at.format("%Y-%m-%d %H:%M").to_string().cyan(),
            s.title.bold()
        );
    }
}

fn build_editor() -> Result<Editor<(), FileHistory>> {
    let cfg = Config::builder()
        .auto_add_history(false)
        .edit_mode(EditMode::Emacs)
        .build();
    let editor = Editor::with_config(cfg)?;
    Ok(editor)
}

fn history_file_path() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".co_worker_cli_history"))
}
