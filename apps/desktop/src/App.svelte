<script lang="ts">
  import { onMount } from 'svelte'

  import Sidebar from './lib/components/Sidebar.svelte'
  import ChatView from './lib/components/ChatView.svelte'
  import StatusBar from './lib/components/StatusBar.svelte'
  import MemoriesDrawer from './lib/components/MemoriesDrawer.svelte'
  import {
    activeSessionId,
    pingHealth,
    refreshMemories,
    refreshSessions,
    sessions,
    openSession,
    startNewSession,
  } from './lib/stores'

  let memoriesOpen = $state(false)

  onMount(() => {
    // Light polling so a backend restart is noticed.
    const t = setInterval(pingHealth, 15_000)

    // Kick off initial fetches without blocking the cleanup return.
    void (async () => {
      await pingHealth()
      const list = await refreshSessions()
      await refreshMemories()
      // Auto-open the most recent session if one exists, otherwise create one.
      if (list.length > 0) {
        await openSession(list[0]!.id)
      } else {
        await startNewSession()
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
    grid-template-rows: 1fr auto;
    min-height: 0;
    min-width: 0;
  }
</style>
