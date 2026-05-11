<script lang="ts">
  import { onMount } from 'svelte'

  import Sidebar from './lib/components/Sidebar.svelte'
  import ChatView from './lib/components/ChatView.svelte'
  import StatusBar from './lib/components/StatusBar.svelte'
  import MemoriesDrawer from './lib/components/MemoriesDrawer.svelte'
  import ModelSwitcher from './lib/components/ModelSwitcher.svelte'
  import {
    pingHealth,
    refreshMemories,
    refreshModelCatalog,
    refreshSessions,
    openSession,
    startNewConversation,
  } from './lib/stores'

  let memoriesOpen = $state(false)

  onMount(() => {
    const t = setInterval(pingHealth, 15_000)

    void (async () => {
      await pingHealth()
      const list = await refreshSessions()
      await refreshMemories()
      await refreshModelCatalog().catch(() => {})
      if (list.length > 0) {
        await openSession(list[0]!.id)
      } else {
        startNewConversation()
      }
    })()

    return () => clearInterval(t)
  })
</script>

<div class="layout">
  <aside class="sidebar">
    <Sidebar onOpenMemories={() => (memoriesOpen = true)} />
  </aside>

  <main class="main">
    <header class="top">
      <span class="brand">co_worker</span>
      <div class="top-actions">
        <ModelSwitcher />
      </div>
    </header>

    <ChatView />
    <StatusBar />
  </main>

  {#if memoriesOpen}
    <MemoriesDrawer onClose={() => (memoriesOpen = false)} />
  {/if}
</div>

<style>
  .layout {
    display: grid;
    grid-template-columns: 280px 1fr;
    height: 100vh;
    width: 100vw;
    background: var(--bg);
  }
  .sidebar {
    border-right: 1px solid var(--border-soft);
    background: var(--bg-soft);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .main {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-height: 0;
    min-width: 0;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 18px;
    border-bottom: 1px solid var(--border-soft);
    background: var(--bg-soft);
    height: 44px;
  }
  .brand {
    font-weight: 600;
    color: var(--fg-muted);
    letter-spacing: 0.02em;
    font-size: 13px;
  }
  .top-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
