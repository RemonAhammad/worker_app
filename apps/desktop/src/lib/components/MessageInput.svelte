<script lang="ts">
  import { createEventDispatcher } from 'svelte'

  let { disabled = false, placeholder = 'Type a message… (Shift+Enter for newline)' } =
    $props<{ disabled?: boolean; placeholder?: string }>()
  const dispatch = createEventDispatcher<{ send: string }>()

  let value = $state('')
  let textarea: HTMLTextAreaElement

  function autosize() {
    if (!textarea) return
    textarea.style.height = 'auto'
    textarea.style.height = Math.min(textarea.scrollHeight, 240) + 'px'
  }

  $effect(() => {
    void value
    autosize()
  })

  function submit() {
    const v = value.trim()
    if (!v || disabled) return
    dispatch('send', v)
    value = ''
  }

  function onKey(e: KeyboardEvent) {
    // Enter → send, Shift+Enter → newline.
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault()
      submit()
    }
  }
</script>

<div class="composer">
  <textarea
    bind:this={textarea}
    bind:value
    {placeholder}
    onkeydown={onKey}
    rows="1"
    {disabled}
  ></textarea>
  <button class="primary send" onclick={submit} disabled={disabled || !value.trim()}>
    Send
  </button>
</div>

<style>
  .composer {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 8px;
    align-items: end;
    padding: 12px 24px 18px;
    border-top: 1px solid var(--border-soft);
    background: var(--bg);
  }
  textarea {
    min-height: 42px;
    max-height: 240px;
    padding: 10px 12px;
    font-size: 14px;
    line-height: 1.45;
    overflow-y: auto;
  }
  .send {
    height: 42px;
    padding: 0 16px;
  }
</style>
