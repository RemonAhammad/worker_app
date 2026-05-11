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
  sendMessage,
  type HealthResponse,
  type Memory,
  type Message,
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

/** Create a new session and switch to it. */
export async function startNewSession(systemPrompt?: string | null): Promise<Session> {
  const title = defaultTitle()
  const s = await createSession(title, systemPrompt ?? null)
  await refreshSessions()
  await openSession(s.id)
  return s
}

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
 * Send a user message in the active session. Pushes both the optimistic
 * user message and the eventual assistant reply into `activeMessages`.
 */
export async function sendInActive(content: string, opts?: SendOptions): Promise<void> {
  const sid = currentActive()
  if (!sid) throw new Error('no active session')

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

function defaultTitle(): string {
  const d = new Date()
  const pad = (n: number) => String(n).padStart(2, '0')
  return `chat ${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
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
