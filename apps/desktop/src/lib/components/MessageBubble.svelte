<script lang="ts">
  import type { Message } from '../api'

  let { message } = $props<{ message: Message }>()
</script>

<div class="bubble" class:user={message.role === 'user'} class:assistant={message.role === 'assistant'} class:system={message.role === 'system'} class:tool={message.role === 'tool'}>
  <div class="role">{message.role}</div>
  <pre class="body">{message.content}</pre>
</div>

<style>
  .bubble {
    border-radius: 10px;
    padding: 10px 14px;
    max-width: 80%;
  }
  .bubble.user {
    background: var(--user-bubble);
    align-self: flex-end;
  }
  .bubble.assistant {
    align-self: flex-start;
    border: 1px solid var(--border-soft);
  }
  .bubble.system,
  .bubble.tool {
    align-self: stretch;
    max-width: none;
    background: transparent;
    border: 1px dashed var(--border-soft);
    color: var(--fg-muted);
  }
  .role {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--fg-dim);
    margin-bottom: 4px;
  }
  .bubble.assistant .role {
    color: var(--success);
  }
  .bubble.user .role {
    color: var(--accent);
  }
  .body {
    margin: 0;
    background: transparent;
    border: none;
    padding: 0;
    white-space: pre-wrap;
    word-wrap: break-word;
    font-family: inherit;
    font-size: 14px;
    color: var(--fg);
  }
</style>
