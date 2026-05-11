/**
 * Reactive stores for the desktop app.
 *
 * Most of the UI reacts to three things:
 *   - `sessions`  — the left-rail conversation list
 *   - `activeSessionId` — which one is currently open
 *   - `activeMessages` — the message log for the active session
 *
 * Plus a handful of meta-stores: backend health, available memories, etc.
 * All mutations go through helpers exported below so the wire calls live in
 * one place.
 */

import { writable, type Writable } from 'svelte/store'

import {
  createMemory,
  createSession,
  deleteMemory as apiDeleteMemory,
  deleteSession as apiDeleteSession,
  getSession,
  health,
  listMemories,
  listSessions,
  loadModel,
  modelCatalog,
  sendMessage,
  updateSession,
  type HealthResponse,
  type Memory,
  type Message,
  type ModelCatalog,
  type ModelCatalogEntry,
  type Session,
} from './api'

export interface SendOptions {
  maxTokens?: number
  temperature?: number
}

export const sessions: Writable<Session[]> = writable([])
export const activeSessionId: Writable<string | null> = writable(null)
export const activeMessages: Writable<Message[]> = writable([])
export const memories: Writable<Memory[]> = writable([])
export const backendHealth: Writable<HealthResponse | null> = writable(null)
export const isGenerating: Writable<boolean> = writable(false)
export const lastError: Writable<string | null> = writable(null)

export const modelCatalogStore: Writable<ModelCatalog | null> = writable(null)
/** True while a model is being downloaded / loaded. While set, the chat
 *  composer disables and the model switcher shows a spinner. */
export const isLoadingModel: Writable<boolean> = writable(false)

/** Refresh the conversation list. */
export async function refreshSessions(): Promise<Session[]> {
  const list = await listSessions(100, 0)
  sessions.set(list)
  return list
}

/** Refresh the memory list. */
export async function refreshMemories(): Promise<Memory[]> {
  const list = await listMemories()
  memories.set(list)
  return list
}

/** Ping the backend; updates `backendHealth`. Errors are swallowed and
 *  reflected by `backendHealth = null`. */
export async function pingHealth(): Promise<HealthResponse | null> {
  try {
    const h = await health()
    backendHealth.set(h)
    return h
  } catch {
    backendHealth.set(null)
    return null
  }
}

/** Switch focus to a session and load its messages. */
export async function openSession(id: string): Promise<void> {
  activeSessionId.set(id)
  activeMessages.set([])
  const full = await getSession(id)
  activeMessages.set(full.messages)
}

/**
 * Stage a new conversation locally — no DB row yet.
 *
 * The actual session is created on the first `sendInActive` call so its
 * title can be the user's first prompt. Until then `activeSessionId` is
 * `null` and `activeMessages` is empty.
 */
export function startNewConversation(systemPrompt?: string | null): void {
  pendingSystemPrompt = systemPrompt ?? null
  activeSessionId.set(null)
  activeMessages.set([])
  lastError.set(null)
}

let pendingSystemPrompt: string | null = null

/** Delete a session. If it's the active one, clear focus. */
export async function deleteSession(id: string): Promise<void> {
  await apiDeleteSession(id)
  activeSessionId.update((cur) => {
    if (cur === id) {
      activeMessages.set([])
      return null
    }
    return cur
  })
  await refreshSessions()
}

/** Patch a session's title (and optionally system prompt). */
export async function renameSession(id: string, title: string): Promise<void> {
  const trimmed = title.trim()
  if (trimmed.length === 0) return
  await updateSession(id, { title: trimmed })
  await refreshSessions()
}

/** Refresh the model catalog (presets + local files). */
export async function refreshModelCatalog(): Promise<ModelCatalog> {
  const c = await modelCatalog()
  modelCatalogStore.set(c)
  return c
}

/**
 * Switch the backend's active model. Blocks chat until done. Refreshes the
 * catalog and the health store on success so the UI picks up the new state.
 */
export async function switchModel(entry: ModelCatalogEntry): Promise<void> {
  if (entry.loaded) return
  isLoadingModel.set(true)
  lastError.set(null)
  try {
    await loadModel(entry.name)
    // Re-read both: catalog has the new loaded flag, health has the new name.
    await refreshModelCatalog()
    await pingHealth()
  } catch (err) {
    lastError.set(formatError(err))
    throw err
  } finally {
    isLoadingModel.set(false)
  }
}

/** Add a memory and refresh the list. */
export async function addMemory(content: string): Promise<Memory> {
  const m = await createMemory(content)
  await refreshMemories()
  return m
}

/** Delete a memory and refresh the list. */
export async function removeMemory(id: string): Promise<void> {
  await apiDeleteMemory(id)
  await refreshMemories()
}

/**
 * Send a user message in the active session. If there is no active session
 * yet (fresh app launch or "+ New chat" was just clicked), this is the
 * point we materialize one — using the first ~60 chars of `content` as the
 * title so the sidebar shows something meaningful.
 *
 * Pushes both the optimistic user message and the eventual assistant reply
 * into `activeMessages`.
 */
export async function sendInActive(content: string, opts?: SendOptions): Promise<void> {
  let sid = currentActive()

  // First message of a new conversation? Materialize the session now with
  // a title derived from the prompt.
  if (!sid) {
    const title = titleFromPrompt(content)
    const s = await createSession(title, pendingSystemPrompt)
    pendingSystemPrompt = null
    sid = s.id
    activeSessionId.set(sid)
    // Refresh the sidebar so the new row appears immediately.
    void refreshSessions()
  }

  const optimistic: Message = {
    id: crypto.randomUUID(),
    session_id: sid,
    role: 'user',
    content,
    token_count: 0,
    created_at: new Date().toISOString(),
    metadata: {},
  }
  activeMessages.update((ms) => [...ms, optimistic])
  isGenerating.set(true)
  lastError.set(null)

  try {
    const resp = await sendMessage(sid, content, opts)
    activeMessages.update((ms) => [...ms, resp.message])
    // Bump the session up the list — its updated_at moved.
    await refreshSessions()
  } catch (err) {
    const message = formatError(err)
    lastError.set(message)
    // Drop the optimistic user message back out so the user can retry.
    activeMessages.update((ms) => ms.filter((m) => m.id !== optimistic.id))
  } finally {
    isGenerating.set(false)
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function currentActive(): string | null {
  let v: string | null = null
  activeSessionId.subscribe((x) => (v = x))()
  return v
}

/** Derive a sidebar title from the user's first message. Collapses
 *  whitespace, trims to ~60 chars, appends an ellipsis if truncated. */
function titleFromPrompt(content: string): string {
  const flat = content.replace(/\s+/g, ' ').trim()
  if (flat.length === 0) return 'new chat'
  return flat.length > 60 ? flat.slice(0, 60).trimEnd() + '…' : flat
}

export function formatError(err: unknown): string {
  if (!err) return 'unknown error'
  if (typeof err === 'string') return err
  if (typeof err === 'object' && err !== null) {
    const e = err as { kind?: string; message?: string }
    if (e.kind && e.message) return `${e.kind}: ${e.message}`
    if (e.message) return e.message
  }
  return String(err)
}
