<script lang="ts">
  import { agentSteps, autoApprove, pendingApproval } from '../stores'

  function shortArgs(args: Record<string, unknown>): string {
    // Show the most identifying fields first.
    if (typeof args.path === 'string') return args.path
    if (typeof args.from === 'string') return `${args.from} → ${args.to ?? '?'}`
    return JSON.stringify(args).slice(0, 80)
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

  function icon(name: string): string {
    switch (name) {
      case 'list_dir': return '📂'
      case 'read_file': return '📄'
      case 'write_file': return '✍️'
      case 'append_file': return '➕'
      case 'delete_path': return '🗑'
      case 'move_path': return '↔️'
      case 'create_dir': return '📁'
      default: return '🔧'
    }
  }

  let expanded = $state<Record<string, boolean>>({})
  function toggle(id: string) {
    expanded[id] = !expanded[id]
  }
</script>

{#each $agentSteps as step (step.id)}
  {#if step.kind === 'assistant_prose'}
    <div class="prose">{step.content}</div>
  {:else}
    {@const c = step.call}
    <div class="card status-{step.status}">
      <button class="card-head" onclick={() => toggle(step.id)} title="Toggle details">
        <span class="ico" aria-hidden="true">{icon(c.name)}</span>
        <span class="name">{c.name}</span>
        <span class="arg" title={JSON.stringify(c.arguments)}>{shortArgs(c.arguments as Record<string, unknown>)}</span>
        <span class="badge {step.status}">
          {#if step.status === 'pending'}awaiting…{/if}
          {#if step.status === 'running'}running…{/if}
          {#if step.status === 'done'}ok{/if}
          {#if step.status === 'denied'}denied{/if}
          {#if step.status === 'error'}error{/if}
        </span>
      </button>
      {#if expanded[step.id]}
        <div class="card-body">
          <pre class="args">{JSON.stringify(c.arguments, null, 2)}</pre>
          {#if step.result}
            <div class="label">result</div>
            <pre class="result">{step.result}</pre>
          {/if}
          {#if step.error}
            <div class="label err">error</div>
            <pre class="result err">{step.error}</pre>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
{/each}

{#if $pendingApproval}
  {@const c = $pendingApproval.call}
  {@const r = $pendingApproval.resolve}
  <div class="approval">
    <div class="approval-head">
      <span class="ico" aria-hidden="true">{icon(c.name)}</span>
      <strong>The assistant wants to run</strong>
      <code>{c.name}</code>
    </div>
    <pre class="args">{JSON.stringify(c.arguments, null, 2)}</pre>
    {#if isMutating(c.name)}
      <label class="auto">
        <input type="checkbox" bind:checked={$autoApprove} />
        Allow all writes for the rest of this turn
      </label>
    {/if}
    <div class="approval-actions">
      <button class="primary" onclick={() => r(true)}>Allow</button>
      <button class="ghost" onclick={() => r(false)}>Deny</button>
    </div>
  </div>
{/if}

<style>
  .prose {
    align-self: flex-start;
    color: var(--fg);
    padding: 6px 0 0;
    white-space: pre-wrap;
  }

  .card {
    align-self: flex-start;
    width: 100%;
    max-width: 720px;
    border: 1px solid var(--border-soft);
    border-radius: 8px;
    background: var(--bg-soft);
    overflow: hidden;
  }
  .card.status-running { border-color: var(--accent); }
  .card.status-done    { border-color: rgba(70, 209, 138, 0.4); }
  .card.status-denied  { border-color: rgba(214, 196, 111, 0.4); }
  .card.status-error   { border-color: rgba(240, 87, 122, 0.4); }

  .card-head {
    all: unset;
    display: grid;
    grid-template-columns: 24px auto 1fr auto;
    gap: 10px;
    align-items: center;
    padding: 8px 12px;
    width: 100%;
    cursor: pointer;
    font-size: 13px;
  }
  .card-head:hover {
    background: var(--bg-elev);
  }
  .ico {
    font-size: 14px;
  }
  .name {
    font-weight: 600;
  }
  .arg {
    color: var(--fg-muted);
    font-family: "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .badge {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--fg-dim);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 1px 6px;
  }
  .badge.running { color: var(--accent); border-color: var(--accent); }
  .badge.done    { color: var(--success); border-color: rgba(70, 209, 138, 0.4); }
  .badge.denied  { color: #d6c46f; border-color: rgba(214, 196, 111, 0.4); }
  .badge.error   { color: var(--error); border-color: rgba(240, 87, 122, 0.4); }

  .card-body {
    border-top: 1px solid var(--border-soft);
    padding: 8px 12px;
  }
  .label {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--fg-dim);
    margin: 6px 0 4px;
  }
  .label.err { color: var(--error); }
  .args, .result {
    background: var(--bg);
    border: 1px solid var(--border-soft);
    border-radius: 6px;
    padding: 8px 10px;
    margin: 0;
    font-size: 12px;
    max-height: 260px;
    overflow: auto;
    white-space: pre-wrap;
    word-break: break-all;
  }
  .result.err {
    border-color: rgba(240, 87, 122, 0.4);
  }

  .approval {
    align-self: flex-start;
    width: 100%;
    max-width: 720px;
    border: 1px solid var(--accent);
    border-radius: 8px;
    background: rgba(108, 123, 245, 0.05);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .approval-head {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
  }
  .approval-actions {
    display: flex;
    gap: 8px;
    margin-top: 4px;
  }
  .auto {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--fg-muted);
  }
</style>
