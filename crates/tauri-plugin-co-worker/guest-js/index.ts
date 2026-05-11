/**
 * TypeScript bindings for the `tauri-plugin-co-worker` Tauri plugin.
 *
 * Every export is a thin, typed wrapper around an `invoke('plugin:co-worker|<command>', ...)`
 * call. The wire shapes mirror the Rust `co_worker_client::*` types one-for-one.
 *
 * Usage:
 *
 * ```ts
 * import { health, listSessions, sendMessage } from 'tauri-plugin-co-worker'
 *
 * const h = await health()                                    // -> HealthResponse
 * const sessions = await listSessions()                       // -> Session[]
 * const reply = await sendMessage(sessions[0].id, 'hello')    // -> MessageResponse
 * ```
 */

import { invoke } from '@tauri-apps/api/core'

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

export type Role = 'system' | 'user' | 'assistant' | 'tool'

export interface Session {
  id: string
  title: string
  model_name: string
  system_prompt: string | null
  created_at: string
  updated_at: string
  metadata: unknown
}

export interface Message {
  id: string
  session_id: string
  role: Role
  content: string
  token_count: number
  created_at: string
  metadata: unknown
}

export interface SessionWithMessages extends Omit<Session, 'metadata'> {
  metadata: unknown
  messages: Message[]
}

export interface Usage {
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

export interface MessageResponse {
  message: Message
  usage: Usage
}

export interface ChatResponse {
  session_id: string
  message: Message
  usage: Usage
}

export interface HealthResponse {
  status: string
  model: string
  loaded: boolean
}

export interface ModelInfo {
  name: string
  size_bytes: number
  loaded: boolean
}

export interface ListModelsResponse {
  models: ModelInfo[]
}

export type ModelKind = 'preset' | 'local'

export interface ModelCatalogEntry {
  name: string
  kind: ModelKind
  repo: string
  filename: string
  context_length: number
  size_bytes: number | null
  min_ram_gib: number | null
  description: string | null
  present: boolean
  loaded: boolean
}

export interface ModelCatalog {
  current: string
  entries: ModelCatalogEntry[]
}

export interface Memory {
  id: string
  content: string
  source: 'manual' | 'auto' | string
  created_at: string
  updated_at: string
}

export interface DebugTurn {
  role: Role
  content: string
}

export interface DebugContext {
  session_id: string
  context_length: number
  turns: DebugTurn[]
  prompt_tokens_estimate: number
  memories_injected: number
}

/** Error payload shape returned across the bridge. */
export interface PluginError {
  kind: string
  message: string
}

// --- Filesystem tools ---

export interface ListDirEntry {
  name: string
  kind: 'dir' | 'file' | 'symlink'
  size_bytes: number | null
}

export interface ListDirResult {
  path: string
  entries: ListDirEntry[]
}

export interface ReadFileResult {
  path: string
  content: string
  truncated: boolean
  bytes_read: number
}

export interface WriteFileResult {
  path: string
  bytes_written: number
  created: boolean
}

export interface DeleteResult {
  path: string
  was_dir: boolean
}

export interface MoveResult {
  from: string
  to: string
}

export interface CreateDirResult {
  path: string
}

// --- Agent loop ---

export interface ParsedToolCall {
  id: string
  name: string
  arguments: Record<string, unknown>
}

export interface ToolResultPayload {
  id: string
  ok: boolean
  content: string
}

export type AgentResponse =
  | { kind: 'message'; message: Message; usage: Usage }
  | {
      kind: 'tool_calls'
      assistant_id: string
      calls: ParsedToolCall[]
      prose: string
      usage: Usage
    }

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

const PLUGIN = 'co-worker'

function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return invoke<T>(`plugin:${PLUGIN}|${command}`, args)
}

// ----- configuration -----

/** Replace the backend URL the plugin talks to. Returns the URL it ended up on. */
export function setBaseUrl(baseUrl: string): Promise<string> {
  return call('set_base_url', { baseUrl })
}

/** Current backend URL. */
export function getBaseUrl(): Promise<string> {
  return call('get_base_url')
}

// ----- backend metadata -----

export function health(): Promise<HealthResponse> {
  return call('health')
}

export function listModels(): Promise<ListModelsResponse> {
  return call('list_models')
}

/** Rich catalog: every preset + every local GGUF, tagged with present/loaded. */
export function modelCatalog(): Promise<ModelCatalog> {
  return call('model_catalog')
}

/**
 * Hot-swap the active model. Downloads the GGUF first if it's not on disk.
 * Takes 10-60s depending on file presence and model size; the UI should
 * disable input during the wait.
 */
export function loadModel(name: string): Promise<ModelCatalogEntry> {
  return call('load_model', { name })
}

// ----- sessions -----

export function listSessions(limit?: number, offset?: number): Promise<Session[]> {
  return call('list_sessions', { limit, offset })
}

export function getSession(id: string): Promise<SessionWithMessages> {
  return call('get_session', { id })
}

export function createSession(
  title: string,
  systemPrompt?: string | null,
): Promise<Session> {
  return call('create_session', { title, systemPrompt })
}

export function deleteSession(id: string): Promise<void> {
  return call('delete_session', { id })
}

/**
 * Patch a session's title and/or system prompt. Pass `undefined` to leave
 * a field untouched; pass empty string for `systemPrompt` to clear it.
 */
export function updateSession(
  id: string,
  patch: { title?: string; systemPrompt?: string },
): Promise<Session> {
  return call('update_session', {
    id,
    title: patch.title,
    systemPrompt: patch.systemPrompt,
  })
}

export function debugSession(id: string): Promise<DebugContext> {
  return call('debug_session', { id })
}

// ----- messages -----

export function sendMessage(
  sessionId: string,
  content: string,
  opts?: { maxTokens?: number; temperature?: number },
): Promise<MessageResponse> {
  return call('send_message', {
    sessionId,
    content,
    maxTokens: opts?.maxTokens,
    temperature: opts?.temperature,
  })
}

/** Sticky-session chat. The server reuses the most recent session. */
export function chat(
  content: string,
  opts?: { maxTokens?: number; temperature?: number; systemPrompt?: string },
): Promise<ChatResponse> {
  return call('chat', {
    content,
    maxTokens: opts?.maxTokens,
    temperature: opts?.temperature,
    systemPrompt: opts?.systemPrompt,
  })
}

// ----- memories -----

export function listMemories(): Promise<Memory[]> {
  return call('list_memories')
}

export function createMemory(content: string): Promise<Memory> {
  return call('create_memory', { content })
}

export function deleteMemory(id: string): Promise<void> {
  return call('delete_memory', { id })
}

// ----- workspace -----

/** Get the current workspace directory, or `null` if none is set. */
export function getWorkspace(): Promise<string | null> {
  return call('get_workspace')
}

/** Set or clear (`path = null`) the workspace directory. Persisted across
 *  app launches. */
export function setWorkspace(path: string | null): Promise<string | null> {
  return call('set_workspace', { path })
}

// ----- filesystem tools (sandboxed to the workspace) -----

export function toolListDir(path: string): Promise<ListDirResult> {
  return call('tool_list_dir', { path })
}

export function toolReadFile(
  path: string,
  maxBytes?: number,
): Promise<ReadFileResult> {
  return call('tool_read_file', { path, maxBytes })
}

export function toolWriteFile(
  path: string,
  content: string,
): Promise<WriteFileResult> {
  return call('tool_write_file', { path, content })
}

export function toolAppendFile(
  path: string,
  content: string,
): Promise<WriteFileResult> {
  return call('tool_append_file', { path, content })
}

export function toolDeletePath(path: string): Promise<DeleteResult> {
  return call('tool_delete_path', { path })
}

export function toolMovePath(from: string, to: string): Promise<MoveResult> {
  return call('tool_move_path', { from, to })
}

export function toolCreateDir(path: string): Promise<CreateDirResult> {
  return call('tool_create_dir', { path })
}

// ----- agent loop -----

export function agentSend(
  sessionId: string,
  content: string,
  opts?: { maxTokens?: number; temperature?: number },
): Promise<AgentResponse> {
  return call('agent_send', {
    sessionId,
    content,
    maxTokens: opts?.maxTokens,
    temperature: opts?.temperature,
  })
}

export function agentContinue(
  sessionId: string,
  assistantId: string,
  results: ToolResultPayload[],
  opts?: { maxTokens?: number; temperature?: number },
): Promise<AgentResponse> {
  return call('agent_continue', {
    sessionId,
    assistantId,
    results,
    maxTokens: opts?.maxTokens,
    temperature: opts?.temperature,
  })
}
