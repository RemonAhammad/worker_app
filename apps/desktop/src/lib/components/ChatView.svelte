<script lang="ts">
  import { tick } from 'svelte'

  import MessageBubble from './MessageBubble.svelte'
  import MessageInput from './MessageInput.svelte'
  import {
    activeMessages,
    activeSessionId,
    isGenerating,
    lastError,
    sendInActive,
    sessions,
  } from '../stores'

  let scrollEl: HTMLDivElement

  // Auto-scroll to the bottom whenever the message list or generating state
  // changes — simulates the natural chat behavior of staying pinned.
  $effect(() => {
    void $activeMessages
    void $isGenerating
    tick().then(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight
    })
  })

  let currentSession = $derived(
    $sessions.find((s) => s.id === $activeSessionId) ?? null,
  )

  async function onSend(content: string) {
    await sendInActive(content)
  }
</script>

<div class="chat">
  <header class="head">
    {#if currentSession}
      <div class="title">{currentSession.title}</div>
      <div class="sub">
        {currentSession.model_name}
        {#if currentSession.system_prompt}
          · system: {currentSession.system_prompt.slice(0, 60)}{currentSession.system_prompt.length > 60 ? '…' : ''}
        {/if}
      </div>
    {:else}
      <div class="title placeholder">no conversation selected</div>
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

    {#if $isGenerating}
      <div class="thinking">
        <span class="dot"></span><span class="dot"></span><span class="dot"></span>
      </div>
    {/if}

    {#if $lastError}
      <div class="error">⚠ {$lastError}</div>
    {/if}
  </div>

  <MessageInput disabled={$isGenerating || !$activeSessionId} on:send={(e) => onSend(e.detail)} />
</div>

<style>
  .chat {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 0;
    min-width: 0;
  }

  .head {
    padding: 14px 24px;
    border-bottom: 1px solid var(--border-soft);
  }
  .title {
    font-size: 15px;
    font-weight: 600;
  }
  .title.placeholder {
    color: var(--fg-dim);
  }
  .sub {
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .dot:nth-child(2) {
    animation-delay: 0.15s;
  }
  .dot:nth-child(3) {
    animation-delay: 0.3s;
  }
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
</style>
