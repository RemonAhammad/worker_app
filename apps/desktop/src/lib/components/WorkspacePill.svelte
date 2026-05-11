<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'

  import { lastError, refreshWorkspace, setWorkspaceRoot, workspace } from '../stores'

  async function pick() {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Choose workspace folder',
      })
      if (typeof selected === 'string') {
        await setWorkspaceRoot(selected)
      }
    } catch (e) {
      lastError.set(`workspace pick failed: ${e}`)
    }
  }

  async function clear() {
    if (!confirm('Disable filesystem tools? The agent will lose access to files.')) return
    await setWorkspaceRoot(null)
  }

  function basename(path: string): string {
    const segs = path.split(/[\\/]/).filter((s) => s.length > 0)
    return segs[segs.length - 1] ?? path
  }

  // Best-effort sync on mount in case App.svelte hasn't fired yet.
  void refreshWorkspace()
</script>

{#if $workspace}
  <div class="pill set" title={$workspace}>
    <span class="folder" aria-hidden="true">📁</span>
    <button class="name" onclick={pick}>{basename($workspace)}</button>
    <button class="x" onclick={clear} title="Disable workspace">×</button>
  </div>
{:else}
  <button class="pill empty" onclick={pick} title="Pick a workspace to enable file tools">
    <span class="folder" aria-hidden="true">📁</span>
    Pick workspace
  </button>
{/if}

<style>
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg-elev);
    font-size: 12.5px;
    color: var(--fg);
    height: 30px;
    line-height: 1;
  }
  .pill.empty {
    cursor: pointer;
    color: var(--fg-muted);
  }
  .pill.empty:hover {
    background: var(--bg-elev);
    color: var(--fg);
  }
  .folder {
    font-size: 13px;
  }
  .name {
    all: unset;
    cursor: pointer;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .name:hover {
    text-decoration: underline;
  }
  .x {
    all: unset;
    color: var(--fg-dim);
    cursor: pointer;
    padding: 0 4px;
    line-height: 1;
    font-size: 14px;
  }
  .x:hover {
    color: var(--error);
  }
</style>
