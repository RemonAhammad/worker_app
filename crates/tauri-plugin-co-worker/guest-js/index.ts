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
