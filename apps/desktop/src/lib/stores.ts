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
  agentContinue,
  agentSend,
  createMemory,
  createSession,
  deleteMemory as apiDeleteMemory,
  deleteSession as apiDeleteSession,
  getAutoAllow,
  getBaseUrl,
  getSession,
  getWorkspace,
  health,
  listMemories,
  listSessions,
  loadModel,
  modelCatalog,
  sendMessage,
  setAutoAllow,
  setWorkspace as apiSetWorkspace,
  toolAppendFile,
  toolCreateDir,
  toolDeletePath,
  toolListDir,
  toolMovePath,
  toolPreviewWrite,
  toolReadFile,
  toolSearch,
  toolWriteFile,
  updateSession,
  type AgentResponse,
  type HealthResponse,
  type Memory,
  type Message,
  type ModelCatalog,
  type ModelCatalogEntry,
  type ParsedToolCall,
  type Session,
  type ToolResultPayload,
  type WritePreview,
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
/** Set to a message string when the most recent catalog refresh failed. */
export const modelCatalogError: Writable<string | null> = writable(null)
/** True while a model is being downloaded / loaded. While set, the chat
 *  composer disables and the model switcher shows a spinner. */
export const isLoadingModel: Writable<boolean> = writable(false)

/** Active workspace directory, or `null` if not set. Sync'd with the
 *  plugin's persisted state at startup. */
export const workspace: Writable<string | null> = writable(null)

/**
 * Steps of an in-flight agent turn shown inline in the chat alongside the
 * regular message bubbles. Cleared when the loop terminates (either with a
 * final assistant message or because the user cancelled).
 */
export type AgentStep =
  | { kind: 'assistant_prose'; id: string; content: string }
  | {
      kind: 'tool_call'
      id: string
      call: ParsedToolCall
      status: 'pending' | 'running' | 'done' | 'denied' | 'error'
      result?: string
      error?: string
    }

export const agentSteps: Writable<AgentStep[]> = writable([])
/** When set, the UI shows an Allow/Deny card. `resolve` is called on click. */
export const pendingApproval: Writable<{
  call: ParsedToolCall
  resolve: (allowed: boolean) => void
} | null> = writable(null)
/** Auto-allow all mutating tool calls for the remainder of this turn. The
 *  user can flip this on a permission prompt to lower friction. */
export const autoApprove: Writable<boolean> = writable(false)

/** Persisted per-tool "always allow" — set of tool names that bypass the
 *  approval card across sessions. Loaded from disk at startup. */
export const persistentAllow: Writable<Set<string>> = writable(new Set())

/** Diff preview attached to a pending approval card for write_file/
 *  append_file. Computed lazily on demand. */
export const approvalDiff: Writable<WritePreview | null> = writable(null)

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

/** Refresh the model catalog (presets + local files). Errors are recorded
 *  in `modelCatalogError` so the UI can show them rather than rendering an
 *  empty popover. */
export async function refreshModelCatalog(): Promise<ModelCatalog | null> {
  try {
    const c = await modelCatalog()
    modelCatalogStore.set(c)
    modelCatalogError.set(null)
    return c
  } catch (err) {
    const msg = formatError(err)
    // The common cause of 404 here is "backend hasn't been rebuilt since
    // the catalog endpoint was added". Spell that out so the user knows
    // what to do.
    if (/404|not.?found/i.test(msg)) {
      modelCatalogError.set(
        'The backend does not expose /v1/models/catalog. Restart the lite backend (cargo run --release) to pick up the latest build.',
      )
    } else {
      modelCatalogError.set(msg)
    }
    modelCatalogStore.set(null)
    return null
  }
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

/** Re-read the workspace from the plugin. */
export async function refreshWorkspace(): Promise<string | null> {
  const w = await getWorkspace()
  workspace.set(w)
  return w
}

/** Set or clear the workspace. `null` disables agentic mode. */
export async function setWorkspaceRoot(path: string | null): Promise<void> {
  const updated = await apiSetWorkspace(path)
  workspace.set(updated)
}

// ---------------------------------------------------------------------------
// Persistent per-tool allow-list.
// ---------------------------------------------------------------------------

export async function refreshPersistentAllow(): Promise<Set<string>> {
  const list = await getAutoAllow()
  const s = new Set(list)
  persistentAllow.set(s)
  return s
}

export async function togglePersistentAllow(tool: string): Promise<void> {
  let next: Set<string> = new Set()
  persistentAllow.update((cur) => {
    next = new Set(cur)
    if (next.has(tool)) next.delete(tool)
    else next.add(tool)
    return next
  })
  await setAutoAllow(Array.from(next))
}

// ---------------------------------------------------------------------------
// Per-session model pin.
// ---------------------------------------------------------------------------

/**
 * Pin (or clear) a preferred model on the active session. Stores the GGUF
 * filename on `sessions.model_name`; the backend auto-swaps on the next
 * send if the loaded model differs.
 */
export async function pinSessionModel(filename: string): Promise<void> {
  const sid = currentActive()
  if (!sid) return
  await updateSession(sid, { /* leave title alone */ })
  // updateSession TS binding only handles title/systemPrompt; call the
  // plugin's update_session command directly via base URL? No — we have
  // the underlying TS API, but it doesn't expose model_name. We patch via
  // a direct PATCH to the backend.
  const base = await getBaseUrl()
  const resp = await fetch(`${base}/v1/sessions/${sid}`, {
    method: 'PATCH',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ model_name: filename }),
  })
  if (!resp.ok) {
    throw new Error(`pin model failed: ${resp.status} ${await resp.text()}`)
  }
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
 * Send a user message in the active session. If there is no active session
 * yet (fresh app launch or "+ New chat" was just clicked), this is the
 * point we materialize one — using the first ~60 chars of `content` as the
 * title so the sidebar shows something meaningful.
 *
 * Pushes both the optimistic user message and the eventual assistant reply
 * into `activeMessages`.
 *
 * When a workspace is configured, this runs the agent loop instead of a
 * plain message round-trip — tool calls bubble up via `agentSteps` and
 * `pendingApproval`.
 */
export async function sendInActive(content: string, opts?: SendOptions): Promise<void> {
  let sid = currentActive()
  const hasWorkspace = currentWorkspace() !== null

  // First message of a new conversation? Materialize the session now with
  // a title derived from the prompt.
  if (!sid) {
    const title = titleFromPrompt(content)
    const s = await createSession(title, pendingSystemPrompt)
    pendingSystemPrompt = null
    sid = s.id
    activeSessionId.set(sid)
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
  agentSteps.set([])
  autoApprove.set(false)

  try {
    if (hasWorkspace) {
      await runAgentLoop(sid, content, opts)
    } else {
      await runStreamingChat(sid, content, opts)
    }
    await refreshSessions()
  } catch (err) {
    const message = formatError(err)
    lastError.set(message)
    activeMessages.update((ms) => ms.filter((m) => m.id !== optimistic.id))
  } finally {
    isGenerating.set(false)
    agentSteps.set([])
    pendingApproval.set(null)
  }
}

const MAX_TOOL_ROUNDTRIPS = 10

/**
 * Stream a non-agent assistant reply via the backend's SSE endpoint.
 *
 * Pushes a placeholder assistant message into `activeMessages` immediately
 * and appends incoming tokens to it as they arrive, so the user sees text
 * flow in real time. On the `done` event we swap the placeholder for the
 * persisted backend row (more accurate id/timestamps). On failure or
 * exception we fall back to the non-streaming `sendMessage` so a degraded
 * backend (no SSE) still works.
 */
async function runStreamingChat(
  sessionId: string,
  content: string,
  opts?: SendOptions,
): Promise<void> {
  const base = await getBaseUrl()
  const placeholderId = crypto.randomUUID()
  const placeholder: Message = {
    id: placeholderId,
    session_id: sessionId,
    role: 'assistant',
    content: '',
    token_count: 0,
    created_at: new Date().toISOString(),
    metadata: {},
  }
  activeMessages.update((ms) => [...ms, placeholder])

  let resp: Response
  try {
    resp = await fetch(
      `${base}/v1/sessions/${sessionId}/messages/stream`,
      {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          Accept: 'text/event-stream',
        },
        body: JSON.stringify({
          content,
          max_tokens: opts?.maxTokens ?? 1024,
          temperature: opts?.temperature ?? 0.7,
        }),
      },
    )
  } catch (err) {
    activeMessages.update((ms) => ms.filter((m) => m.id !== placeholderId))
    // Non-streaming fallback so older backends still work.
    const fallback = await sendMessage(sessionId, content, opts)
    activeMessages.update((ms) => [...ms, fallback.message])
    return
  }

  if (!resp.ok || !resp.body) {
    activeMessages.update((ms) => ms.filter((m) => m.id !== placeholderId))
    const body = await resp.text().catch(() => '')
    throw new Error(`stream failed: ${resp.status} ${body}`)
  }

  const reader = resp.body
    .pipeThrough(new TextDecoderStream())
    .getReader()
  let bufferedLine = ''
  // SSE frames are terminated by a blank line. Each frame is one event,
  // composed of one or more `data:` lines we concatenate.
  let eventBuffer = ''

  const finishEvent = async () => {
    if (!eventBuffer) return
    let parsed: unknown
    try {
      parsed = JSON.parse(eventBuffer)
    } catch {
      return
    }
    eventBuffer = ''
    if (typeof parsed !== 'object' || parsed === null) return
    const ev = parsed as { type?: string; text?: string; message?: Message; error?: string }
    if (ev.type === 'token' && typeof ev.text === 'string') {
      const token = ev.text
      activeMessages.update((ms) =>
        ms.map((m) =>
          m.id === placeholderId ? { ...m, content: m.content + token } : m,
        ),
      )
    } else if (ev.type === 'done' && ev.message) {
      activeMessages.update((ms) =>
        ms.map((m) => (m.id === placeholderId ? ev.message! : m)),
      )
    } else if (ev.type === 'error') {
      activeMessages.update((ms) => ms.filter((m) => m.id !== placeholderId))
      throw new Error(`stream error: ${ev.error ?? 'unknown'}`)
    }
  }

  while (true) {
    const { value, done } = await reader.read()
    if (done) break
    bufferedLine += value
    // Split incoming text into complete lines; keep the trailing partial.
    let nl
    while ((nl = bufferedLine.indexOf('\n')) >= 0) {
      const line = bufferedLine.slice(0, nl).replace(/\r$/, '')
      bufferedLine = bufferedLine.slice(nl + 1)
      if (line === '') {
        await finishEvent()
      } else if (line.startsWith('data:')) {
        eventBuffer += line.slice(5).trimStart()
      }
      // SSE has other prefixes (event:, id:, retry:) we ignore.
    }
  }
  // Drain any final event that didn't end with a blank line.
  await finishEvent()
}

/** Walk the agent loop until the model produces a final assistant message
 *  or we exceed the round-trip cap. Tool execution and permission prompts
 *  happen inline. */
async function runAgentLoop(
  sessionId: string,
  content: string,
  opts?: SendOptions,
): Promise<void> {
  let response: AgentResponse = await agentSend(sessionId, content, opts)
  for (let round = 0; round < MAX_TOOL_ROUNDTRIPS; round++) {
    if (response.kind === 'message') {
      const final = response
      activeMessages.update((ms) => [...ms, final.message])
      return
    }
    // tool_calls branch: show prose (if any), then for each call: prompt
    // for approval if mutating, execute, stash the result.
    const turn = response
    const prose = turn.prose.trim()
    if (prose.length > 0) {
      agentSteps.update((s) => [
        ...s,
        { kind: 'assistant_prose', id: crypto.randomUUID(), content: prose },
      ])
    }

    const results: ToolResultPayload[] = []
    for (const call of turn.calls) {
      const stepId = crypto.randomUUID()
      agentSteps.update((s) => [
        ...s,
        { kind: 'tool_call', id: stepId, call, status: 'pending' },
      ])

      let allowed = true
      if (isMutating(call.name)) {
        // Persistent allow-list lets the user pre-approve specific tools.
        const persistent = currentPersistent()
        if (currentAutoApprove() || persistent.has(call.name)) {
          allowed = true
        } else {
          // For file-writing tools, compute a diff preview so the approval
          // card can show what will change.
          if (call.name === 'write_file' || call.name === 'append_file') {
            const path = typeof call.arguments.path === 'string'
              ? call.arguments.path
              : ''
            const content = typeof call.arguments.content === 'string'
              ? call.arguments.content
              : ''
            try {
              const preview = await toolPreviewWrite(path, content)
              approvalDiff.set(preview)
            } catch {
              approvalDiff.set(null)
            }
          } else {
            approvalDiff.set(null)
          }
          allowed = await askApproval(call)
          approvalDiff.set(null)
        }
      }

      if (!allowed) {
        updateStep(stepId, { status: 'denied', error: 'user denied' })
        results.push({
          id: call.id,
          ok: false,
          content: 'The user denied this tool call.',
        })
        continue
      }

      updateStep(stepId, { status: 'running' })
      try {
        const out = await runTool(call)
        updateStep(stepId, { status: 'done', result: out })
        results.push({ id: call.id, ok: true, content: out })
      } catch (err) {
        const msg = formatError(err)
        updateStep(stepId, { status: 'error', error: msg })
        results.push({ id: call.id, ok: false, content: msg })
      }
    }

    response = await agentContinue(sessionId, turn.assistant_id, results, opts)
  }
  // Hit the cap.
  agentSteps.update((s) => [
    ...s,
    {
      kind: 'assistant_prose',
      id: crypto.randomUUID(),
      content: `(stopped after ${MAX_TOOL_ROUNDTRIPS} tool round-trips)`,
    },
  ])
}

function isMutating(name: string): boolean {
  return (
    name === 'write_file' ||
    name === 'append_file' ||
    name === 'delete_path' ||
    name === 'move_path' ||
    name === 'create_dir'
  )
}

function updateStep(id: string, patch: Partial<Extract<AgentStep, { kind: 'tool_call' }>>) {
  agentSteps.update((steps) =>
    steps.map((s) => (s.kind === 'tool_call' && s.id === id ? { ...s, ...patch } : s)),
  )
}

function askApproval(call: ParsedToolCall): Promise<boolean> {
  return new Promise((resolve) => {
    pendingApproval.set({
      call,
      resolve: (allowed) => {
        pendingApproval.set(null)
        resolve(allowed)
      },
    })
  })
}

/** Dispatch a single tool call to the plugin's sandboxed implementations. */
async function runTool(call: ParsedToolCall): Promise<string> {
  const a = call.arguments as Record<string, unknown>
  switch (call.name) {
    case 'list_dir': {
      const r = await toolListDir(asString(a.path, '.'))
      return JSON.stringify(r)
    }
    case 'read_file': {
      const r = await toolReadFile(asString(a.path), asNumber(a.max_bytes))
      return JSON.stringify(r)
    }
    case 'write_file': {
      const r = await toolWriteFile(asString(a.path), asString(a.content, ''))
      return JSON.stringify(r)
    }
    case 'append_file': {
      const r = await toolAppendFile(asString(a.path), asString(a.content, ''))
      return JSON.stringify(r)
    }
    case 'delete_path': {
      const r = await toolDeletePath(asString(a.path))
      return JSON.stringify(r)
    }
    case 'move_path': {
      const r = await toolMovePath(asString(a.from), asString(a.to))
      return JSON.stringify(r)
    }
    case 'create_dir': {
      const r = await toolCreateDir(asString(a.path))
      return JSON.stringify(r)
    }
    case 'search': {
      const r = await toolSearch(asString(a.query), {
        path: typeof a.path === 'string' ? a.path : undefined,
        maxResults: asNumber(a.max_results),
        caseInsensitive: a.case_insensitive === true,
      })
      return JSON.stringify(r)
    }
    default:
      throw new Error(`unknown tool: ${call.name}`)
  }
}

function asString(v: unknown, fallback?: string): string {
  if (typeof v === 'string') return v
  if (fallback !== undefined) return fallback
  throw new Error(`missing required string argument`)
}

function asNumber(v: unknown): number | undefined {
  return typeof v === 'number' ? v : undefined
}

function currentWorkspace(): string | null {
  let v: string | null = null
  workspace.subscribe((x) => (v = x))()
  return v
}

function currentAutoApprove(): boolean {
  let v = false
  autoApprove.subscribe((x) => (v = x))()
  return v
}

function currentPersistent(): Set<string> {
  let v: Set<string> = new Set()
  persistentAllow.subscribe((x) => (v = x))()
  return v
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
