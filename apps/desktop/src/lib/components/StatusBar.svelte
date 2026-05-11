<script lang="ts">
  import { backendHealth, isGenerating } from '../stores'
</script>

<footer class="status">
  <span class="left">
    {#if $backendHealth}
      <span class="dot ok"></span>
      <span class="model">{$backendHealth.model}</span>
    {:else}
      <span class="dot off"></span>
      <span class="off-text">backend offline</span>
    {/if}
  </span>
  <span class="right">
    {#if $isGenerating}
      thinking…
    {/if}
  </span>
</footer>

<style>
  .status {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 4px 24px;
    border-top: 1px solid var(--border-soft);
    background: var(--bg-soft);
    font-size: 11px;
    color: var(--fg-dim);
  }
  .left,
  .right {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    display: inline-block;
  }
  .dot.ok {
    background: var(--success);
  }
  .dot.off {
    background: var(--error);
  }
  .off-text {
    color: var(--error);
  }
  .model {
    color: var(--fg-muted);
  }
</style>
