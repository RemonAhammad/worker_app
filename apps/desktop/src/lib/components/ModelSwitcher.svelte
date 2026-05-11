<script lang="ts">
  import { onMount } from 'svelte'

  import {
    activeSessionId,
    backendHealth,
    isLoadingModel,
    modelCatalogError,
    modelCatalogStore,
    pinSessionModel,
    refreshModelCatalog,
    refreshSessions,
    sessions,
    switchModel,
  } from '../stores'
  import type { ModelCatalogEntry } from '../api'

  let open = $state(false)
  let switching = $state<string | null>(null)
  let buttonEl: HTMLButtonElement | undefined = $state()
  let popoverEl: HTMLDivElement | undefined = $state()

  onMount(() => {
    void refreshModelCatalog()
  })

  // Close on outside click / Escape.
  function onDocClick(e: MouseEvent) {
    if (!open) return
    const t = e.target as Node
    if (popoverEl?.contains(t) || buttonEl?.contains(t)) return
    open = false
  }
  function onDocKey(e: KeyboardEvent) {
    if (e.key === 'Escape') open = false
  }

  $effect(() => {
    if (open) {
      document.addEventListener('mousedown', onDocClick)
      document.addEventListener('keydown', onDocKey)
      // Refresh catalog when the menu opens so freshly downloaded files appear.
      void refreshModelCatalog()
    }
    return () => {
      document.removeEventListener('mousedown', onDocClick)
      document.removeEventListener('keydown', onDocKey)
    }
  })

  async function pick(entry: ModelCatalogEntry) {
    if (entry.loaded || switching) return
    switching = entry.name
    try {
      await switchModel(entry)
      open = false
    } catch {
      // error surfaces via `lastError` store
    } finally {
      switching = null
    }
  }

  /** Pin this model to the active session — the backend will hot-swap to
   *  it on the next message if it isn't already loaded. */
  async function pinForSession(entry: ModelCatalogEntry, e: MouseEvent) {
    e.stopPropagation()
    if (!$activeSessionId) return
    try {
      await pinSessionModel(entry.filename)
      await refreshSessions()
    } catch (err) {
      console.error(err)
    }
  }

  let currentSession = $derived(
    $sessions.find((s) => s.id === $activeSessionId) ?? null,
  )
  function isPinnedToActive(entry: ModelCatalogEntry): boolean {
    return currentSession ? currentSession.model_name === entry.filename : false
  }

  function sizeLabel(bytes: number | null, kind: 'preset' | 'local'): string {
    if (bytes === null) return kind === 'preset' ? 'downloadable' : 'unknown size'
    const gib = bytes / 1024 / 1024 / 1024
    return gib >= 1 ? `${gib.toFixed(1)} GiB` : `${(bytes / 1024 / 1024).toFixed(0)} MiB`
  }

  let currentLabel = $derived($backendHealth?.model ?? 'no model loaded')
</script>

<div class="wrap">
  <button
    bind:this={buttonEl}
    class="trigger"
    class:loading={$isLoadingModel}
    onclick={() => (open = !open)}
    disabled={$isLoadingModel}
    title="Switch model"
  >
    {#if $isLoadingModel}
      <span class="spinner" aria-hidden="true"></span>
      <span class="label">loading…</span>
    {:else}
      <span class="dot"></span>
      <span class="label">{currentLabel}</span>
      <span class="chev" aria-hidden="true">▾</span>
    {/if}
  </button>

  {#if open}
    <div class="pop" bind:this={popoverEl}>
      <div class="pop-head">
        <span>Switch model</span>
        <span class="hint">
          loading takes 10–60s · downloads block until complete
        </span>
      </div>

      {#if $modelCatalogError}
        <div class="catalog-error">
          <strong>Couldn't load the catalog.</strong>
          <span>{$modelCatalogError}</span>
          <button class="retry" onclick={() => void refreshModelCatalog()}>
            Retry
          </button>
        </div>
      {:else if !$modelCatalogStore}
        <div class="loading-state">
          <span class="spinner" aria-hidden="true"></span>
          loading catalog…
        </div>
      {:else}
      <ul>
        {#each $modelCatalogStore.entries as entry (entry.name)}
          {@const isSwitchingThis = switching === entry.name}
          <li class="row-wrap">
            <button
              class="row"
              class:loaded={entry.loaded}
              disabled={!!switching || entry.loaded || $isLoadingModel}
              onclick={() => pick(entry)}
            >
              <div class="row-main">
                <div class="row-top">
                  <span class="name">{entry.name}</span>
                  {#if entry.kind === 'local'}
                    <span class="tag local">local</span>
                  {/if}
                  {#if entry.loaded}
                    <span class="tag loaded">loaded</span>
                  {:else if entry.present}
                    <span class="tag present">downloaded</span>
                  {:else}
                    <span class="tag download">download {sizeLabel(entry.size_bytes, entry.kind)}</span>
                  {/if}
                </div>
                {#if entry.description}
                  <div class="desc">{entry.description}</div>
                {/if}
                <div class="meta">
                  {#if entry.size_bytes !== null}
                    <span>{sizeLabel(entry.size_bytes, entry.kind)}</span>
                    <span class="sep">·</span>
                  {/if}
                  {#if entry.min_ram_gib}
                    <span>{entry.min_ram_gib} GB RAM</span>
                    <span class="sep">·</span>
                  {/if}
                  <span>ctx {Math.round(entry.context_length / 1024)}K</span>
                </div>
              </div>
              <span class="row-action">
                {#if isSwitchingThis}
                  <span class="spinner small"></span>
                {:else if entry.loaded}
                  ✓
                {:else if entry.present}
                  switch
                {:else}
                  download
                {/if}
              </span>
            </button>
            {#if $activeSessionId}
              <button
                class="pin"
                class:pinned={isPinnedToActive(entry)}
                onclick={(e) => pinForSession(entry, e)}
                title={isPinnedToActive(entry) ? 'Pinned to this session' : 'Pin to this session'}
                disabled={!!switching || $isLoadingModel}
              >
                📌
              </button>
            {/if}
          </li>
        {/each}
      </ul>
      {/if}
    </div>
  {/if}
</div>

<style>
  .wrap {
    position: relative;
  }

  .trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    background: var(--bg-elev);
    border-color: var(--border);
    border-radius: 6px;
    font-size: 12.5px;
    max-width: 360px;
  }
  .trigger .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--success);
    flex-shrink: 0;
  }
  .trigger .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 280px;
  }
  .trigger .chev {
    color: var(--fg-dim);
  }
  .trigger.loading {
    opacity: 0.9;
  }

  .pop {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    width: 460px;
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 14px 40px rgba(0, 0, 0, 0.45);
    z-index: 50;
    overflow: hidden;
  }
  .pop-head {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-soft);
  }
  .pop-head > span:first-child {
    font-weight: 600;
    font-size: 13px;
  }
  .pop-head .hint {
    font-size: 11px;
    color: var(--fg-dim);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 6px;
    max-height: 60vh;
    overflow-y: auto;
  }

  .row {
    all: unset;
    display: flex;
    gap: 12px;
    align-items: center;
    width: 100%;
    padding: 10px;
    border-radius: 6px;
    cursor: pointer;
  }
  .row:hover:not(:disabled):not(.loaded) {
    background: var(--bg-elev);
  }
  .row:disabled:not(.loaded) {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .row.loaded {
    background: rgba(70, 209, 138, 0.06);
    cursor: default;
  }

  .row-main {
    flex: 1;
    min-width: 0;
  }
  .row-top {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }
  .name {
    font-weight: 600;
  }
  .desc {
    color: var(--fg-muted);
    font-size: 12.5px;
    margin: 3px 0 0;
    line-height: 1.45;
  }
  .meta {
    color: var(--fg-dim);
    font-size: 11.5px;
    margin-top: 4px;
    display: flex;
    gap: 4px;
    align-items: center;
  }
  .meta .sep {
    opacity: 0.6;
  }

  .tag {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-radius: 4px;
    padding: 1px 6px;
    border: 1px solid;
  }
  .tag.local {
    color: var(--fg-muted);
    border-color: var(--border);
  }
  .tag.loaded {
    color: var(--success);
    border-color: rgba(70, 209, 138, 0.4);
  }
  .tag.present {
    color: #b8c3d6;
    border-color: rgba(184, 195, 214, 0.3);
  }
  .tag.download {
    color: #d6c46f;
    border-color: rgba(214, 196, 111, 0.4);
  }

  .row-action {
    font-size: 12px;
    color: var(--fg-muted);
    flex-shrink: 0;
  }

  .row-wrap {
    display: flex;
    align-items: stretch;
    gap: 2px;
  }
  .row-wrap > .row {
    flex: 1;
  }
  .pin {
    all: unset;
    cursor: pointer;
    width: 30px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 6px;
    color: var(--fg-dim);
    font-size: 13px;
    opacity: 0.4;
    transition: opacity 100ms ease, background 100ms ease;
  }
  .pin:hover:not(:disabled) {
    opacity: 1;
    background: var(--bg-elev);
  }
  .pin.pinned {
    opacity: 1;
    color: var(--accent);
  }
  .pin:disabled {
    cursor: not-allowed;
  }

  .spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    display: inline-block;
    animation: spin 0.8s linear infinite;
  }
  .spinner.small {
    width: 10px;
    height: 10px;
    border-width: 1.5px;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .catalog-error {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 14px;
    color: var(--fg);
    font-size: 12.5px;
    line-height: 1.5;
  }
  .catalog-error strong {
    color: var(--error);
  }
  .catalog-error .retry {
    align-self: flex-start;
    margin-top: 4px;
  }

  .loading-state {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 18px 14px;
    color: var(--fg-dim);
    font-size: 12.5px;
  }
</style>
