<script lang="ts">
  import { addMemory, memories, removeMemory } from '../stores'

  let { onClose } = $props<{ onClose: () => void }>()

  let draft = $state('')
  let saving = $state(false)
  let errorMsg = $state<string | null>(null)

  async function save() {
    const v = draft.trim()
    if (!v) return
    saving = true
    errorMsg = null
    try {
      await addMemory(v)
      draft = ''
    } catch (e) {
      errorMsg = String(e)
    } finally {
      saving = false
    }
  }

  async function remove(id: string) {
    if (!confirm('Delete this memory?')) return
    await removeMemory(id)
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Enter' && (e.metaKey || e.ctrlKey)) {
      e.preventDefault()
      save()
    }
    if (e.key === 'Escape') onClose()
  }
</script>

<div class="scrim" onclick={onClose} role="presentation"></div>

<div class="drawer" role="dialog" aria-modal="true" aria-label="Memories">
  <header>
    <h2>Memories</h2>
    <p>Facts injected into every conversation's system prompt.</p>
    <button class="ghost close" onclick={onClose} aria-label="Close">×</button>
  </header>

  <div class="add">
    <textarea
      placeholder='e.g. "my name is Rimon" or "I prefer Rust over Go"'
      bind:value={draft}
      onkeydown={onKey}
      rows="2"
    ></textarea>
    <button class="primary" onclick={save} disabled={saving || !draft.trim()}>
      {saving ? 'saving…' : 'add'}
    </button>
  </div>

  {#if errorMsg}
    <div class="err">⚠ {errorMsg}</div>
  {/if}

  <ul class="list">
    {#if $memories.length === 0}
      <li class="empty">no memories yet</li>
    {/if}
    {#each $memories as m (m.id)}
      <li>
        <span class="tag" class:auto={m.source === 'auto'}>{m.source}</span>
        <span class="body">{m.content}</span>
        <button class="del" onclick={() => remove(m.id)} aria-label="Delete">×</button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    z-index: 9;
  }
  .drawer {
    position: fixed;
    right: 0;
    top: 0;
    bottom: 0;
    width: 420px;
    background: var(--bg);
    border-left: 1px solid var(--border);
    z-index: 10;
    display: flex;
    flex-direction: column;
    box-shadow: -8px 0 24px rgba(0, 0, 0, 0.35);
  }
  header {
    padding: 18px 20px 14px;
    border-bottom: 1px solid var(--border-soft);
    position: relative;
  }
  header h2 {
    margin: 0;
    font-size: 16px;
  }
  header p {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--fg-dim);
  }
  .close {
    position: absolute;
    top: 12px;
    right: 12px;
    width: 28px;
    height: 28px;
    padding: 0;
    border-radius: 6px;
    font-size: 18px;
    line-height: 1;
  }

  .add {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    align-items: end;
    padding: 14px 20px;
    border-bottom: 1px solid var(--border-soft);
  }
  .add textarea {
    min-height: 56px;
  }

  .err {
    color: var(--error);
    font-size: 12px;
    padding: 8px 20px;
  }

  .list {
    list-style: none;
    margin: 0;
    padding: 8px 12px;
    overflow-y: auto;
    flex: 1;
  }
  .list li {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 6px;
  }
  .list li:hover {
    background: var(--bg-elev);
  }
  .list .empty {
    color: var(--fg-dim);
    text-align: center;
    display: block;
    padding: 30px 0;
  }
  .tag {
    font-size: 11px;
    text-transform: uppercase;
    color: var(--fg-muted);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 6px;
  }
  .tag.auto {
    color: #d6c46f;
    border-color: rgba(214, 196, 111, 0.4);
  }
  .body {
    font-size: 13px;
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
    cursor: pointer;
  }
  .del:hover {
    color: var(--error);
    background: rgba(240, 87, 122, 0.1);
  }
</style>
