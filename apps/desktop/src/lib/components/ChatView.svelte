<script lang="ts">
  import { tick } from 'svelte'

  import AgentSteps from './AgentSteps.svelte'
  import MessageBubble from './MessageBubble.svelte'
  import MessageInput from './MessageInput.svelte'
  import {
    activeMessages,
    activeSessionId,
    agentSteps,
    deleteSession,
    isGenerating,
    isLoadingModel,
    lastError,
    pendingApproval,
    renameSession,
    sendInActive,
    sessions,
    workspace,
  } from '../stores'

  let scrollEl: HTMLDivElement

  $effect(() => {
    void $activeMessages
    void $isGenerating
    void $agentSteps
    void $pendingApproval
    tick().then(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight
    })
  })

  let currentSession = $derived(
    $sessions.find((s) => s.id === $activeSessionId) ?? null,
  )

  // Inline title editing.
  let editing = $state(false)
  let titleDraft = $state('')
  let titleInput: HTMLInputElement | undefined = $state()

  function startRename() {
    if (!currentSession) return
    titleDraft = currentSession.title
    editing = true
    tick().then(() => titleInput?.select())
  }

  async function commitRename() {
    if (!currentSession || !editing) return
    editing = false
    const next = titleDraft.trim()
    if (!next || next === currentSession.title) return
    try {
      await renameSession(currentSession.id, next)
    } catch (e) {
      lastError.set(`rename failed: ${e}`)
    }
  }

  function cancelRename() {
    editing = false
  }

  function onTitleKey(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      e.preventDefault()
      void commitRename()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      cancelRename()
    }
  }

  async function onDelete() {
    if (!currentSession) return
    if (!confirm(`Delete conversation "${currentSession.title}"?`)) return
    await deleteSession(currentSession.id)
  }

  async function onSend(content: string) {
    await sendInActive(content)
  }
</script>

<div class="chat">
  <header class="head">
    <div class="head-text">
      {#if currentSession}
        {#if editing}
          <input
            bind:this={titleInput}
            bind:value={titleDraft}
            class="title-input"
            onkeydown={onTitleKey}
            onblur={() => void commitRename()}
          />
        {:else}
          <button class="title-btn" onclick={startRename} title="Click to rename">
            {currentSession.title}
            <span class="edit-hint">✎</span>
          </button>
        {/if}
        <div class="sub">
          {currentSession.model_name}
          {#if currentSession.system_prompt}
            · system: {currentSession.system_prompt.slice(0, 60)}{currentSession.system_prompt.length > 60 ? '…' : ''}
          {/if}
        </div>
      {:else}
        <div class="title placeholder">new conversation</div>
        <div class="sub">your first message becomes the title</div>
      {/if}
    </div>

    {#if currentSession}
      <div class="head-actions">
        <button class="ghost icon" title="Rename" onclick={startRename}>✎</button>
        <button class="ghost icon danger" title="Delete conversation" onclick={onDelete}>🗑</button>
      </div>
    {/if}
  </header>

  <div class="scroll" bind:this={scrollEl}>
    {#if $activeMessages.length === 0 && !$isGenerating}
      <div class="empty">
        <h3>start a conversation</h3>
        <p>type a message below — the model picks up your memories automatically.</p>
      </div>
    {/if}

    {#each $activeMessages as m (m.id)}
      <MessageBubble message={m} />
    {/each}

    <AgentSteps />

    {#if $isGenerating && $agentSteps.length === 0}
      <div class="thinking">
        <span class="dot"></span><span class="dot"></span><span class="dot"></span>
      </div>
    {/if}

    {#if $lastError}
      <div class="error">⚠ {$lastError}</div>
    {/if}
  </div>

  <MessageInput
    disabled={$isGenerating || $isLoadingModel}
    placeholder={
      $isLoadingModel
        ? 'loading model…'
        : $workspace
          ? 'Ask anything — the assistant can read & edit files in your workspace…'
          : 'Type a message… (Shift+Enter for newline)'
    }
    on:send={(e) => onSend(e.detail)}
  />
</div>

<style>
  .chat {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 0;
    min-width: 0;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 14px 24px;
    border-bottom: 1px solid var(--border-soft);
  }
  .head-text {
    min-width: 0;
    flex: 1;
  }
  .title-btn {
    all: unset;
    cursor: pointer;
    font-size: 15px;
    font-weight: 600;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    border-radius: 4px;
    padding: 1px 4px;
    margin: -1px -4px;
  }
  .title-btn:hover .edit-hint {
    opacity: 0.9;
  }
  .edit-hint {
    color: var(--fg-dim);
    font-size: 12px;
    opacity: 0;
    transition: opacity 100ms ease;
  }
  .title-input {
    font-size: 15px;
    font-weight: 600;
    padding: 2px 6px;
    width: 100%;
    max-width: 520px;
  }
  .title.placeholder {
    color: var(--fg-dim);
    font-size: 15px;
    font-weight: 600;
  }
  .sub {
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 4px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .head-actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
  .head-actions .icon {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    border-radius: 6px;
    border-color: transparent;
    color: var(--fg-muted);
  }
  .head-actions .icon:hover {
    color: var(--fg);
    background: var(--bg-elev);
  }
  .head-actions .icon.danger:hover {
    color: var(--error);
  }

  .scroll {
    overflow-y: auto;
    padding: 18px 24px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }

  .empty {
    margin: auto;
    text-align: center;
    color: var(--fg-dim);
  }
  .empty h3 {
    margin: 0 0 6px 0;
    color: var(--fg-muted);
    font-weight: 500;
  }
  .empty p {
    margin: 0;
  }

  .thinking {
    display: flex;
    gap: 4px;
    padding: 8px 14px;
    align-self: flex-start;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-muted);
    animation: pulse 1.2s infinite ease-in-out;
  }
  .dot:nth-child(2) { animation-delay: 0.15s; }
  .dot:nth-child(3) { animation-delay: 0.3s; }
  @keyframes pulse {
    0%, 80%, 100% { opacity: 0.3; transform: translateY(0); }
    40% { opacity: 1; transform: translateY(-2px); }
  }

  .error {
    color: var(--error);
    background: rgba(240, 87, 122, 0.08);
    border: 1px solid rgba(240, 87, 122, 0.25);
    border-radius: 6px;
    padding: 8px 12px;
    font-size: 13px;
    align-self: flex-start;
  }

  .title-btn:hover .edit-hint,
  .title-btn:focus-visible .edit-hint {
    opacity: 0.7;
  }
</style>
