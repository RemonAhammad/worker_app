<script lang="ts">
  import {
    activeSessionId,
    deleteSession,
    openSession,
    sessions,
    startNewSession,
  } from '../stores'

  let { onOpenMemories } = $props<{ onOpenMemories: () => void }>()

  function fmt(iso: string): string {
    const d = new Date(iso)
    return `${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
  }
  const pad = (n: number) => String(n).padStart(2, '0')

  async function onNew() {
    await startNewSession()
  }

  async function onSelect(id: string) {
    if ($activeSessionId === id) return
    await openSession(id)
  }

  async function onDelete(e: MouseEvent, id: string) {
    e.stopPropagation()
    if (!confirm('Delete this conversation?')) return
    await deleteSession(id)
  }
</script>

<header class="sidebar-header">
  <button class="primary new" onclick={onNew}>+ New chat</button>
  <button class="ghost icon" onclick={onOpenMemories} title="Memories">
    <span class="brain">⟁</span>
  </button>
</header>

<div class="list">
  {#if $sessions.length === 0}
    <p class="empty">no conversations yet</p>
  {:else}
    {#each $sessions as s (s.id)}
      <div
        role="button"
        tabindex="0"
        class="row"
        class:active={$activeSessionId === s.id}
        onclick={() => onSelect(s.id)}
        onkeydown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault()
            onSelect(s.id)
          }
        }}
      >
        <div class="row-main">
          <div class="title">{s.title}</div>
          <div class="meta">
            <span class="time">{fmt(s.updated_at)}</span>
            <span class="model">{s.model_name.split('-').slice(0, 3).join('-')}</span>
          </div>
        </div>
        <button
          type="button"
          class="del"
          aria-label="Delete conversation"
          onclick={(e) => onDelete(e, s.id)}
        >×</button>
      </div>
    {/each}
  {/if}
</div>

<style>
  .sidebar-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px;
    border-bottom: 1px solid var(--border-soft);
  }
  .new {
    flex: 1;
  }
  .icon {
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    padding: 0;
  }
  .brain {
    font-size: 18px;
    color: var(--fg-muted);
  }

  .list {
    flex: 1;
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .empty {
    color: var(--fg-dim);
    text-align: center;
    margin-top: 30px;
    font-size: 13px;
  }

  .row {
    display: flex;
    align-items: stretch;
    gap: 8px;
    padding: 8px 10px;
    border-radius: 6px;
    cursor: pointer;
    border: 1px solid transparent;
    outline: none;
  }
  .row:focus-visible {
    border-color: var(--accent);
  }
  .row:hover {
    background: var(--bg-elev);
  }
  .row.active {
    background: var(--bg-elev);
    border-color: var(--border);
  }
  .row-main {
    flex: 1;
    min-width: 0;
  }
  .title {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    gap: 8px;
    color: var(--fg-dim);
    font-size: 12px;
    margin-top: 2px;
  }
  .model {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .del {
    all: unset;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--fg-dim);
    border-radius: 4px;
    align-self: center;
    opacity: 0;
    transition: opacity 100ms ease, background 100ms ease;
    cursor: pointer;
  }
  .row:hover .del {
    opacity: 1;
  }
  .del:hover {
    color: var(--error);
    background: rgba(240, 87, 122, 0.1);
  }
</style>
