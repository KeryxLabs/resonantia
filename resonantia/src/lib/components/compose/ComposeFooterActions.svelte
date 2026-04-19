<script lang="ts">
  import type { ComposeMode } from "./types";

  export let mode: ComposeMode = "live";
  export let loading = false;
  export let replyLoading = false;
  export let messagesCount = 0;
  export let sessionId = "";
  export let onClose: () => void = () => {};
  export let submitCompose: () => Promise<void> | void = () => {};
</script>

<div class="drawer-actions compose-actions">
  <button class="drawer-btn cancel" on:click={onClose}>cancel</button>
  {#if mode === 'live'}
    <button class="drawer-btn submit" on:click={submitCompose} disabled={loading || replyLoading || messagesCount === 0 || !sessionId.trim()}>
      {loading ? 'encoding…' : 'encode + save + continue'}
    </button>
  {/if}
</div>

<style>
  .drawer-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 8px 12px 12px;
    padding-top: 8px;
    border-top: 0.5px solid rgba(255, 255, 255, 0.04);
  }

  .compose-actions {
    justify-content: flex-end;
    align-items: center;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.06em;
    padding: 7px 13px;
    border-radius: 999px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .drawer-btn.cancel {
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.3);
  }

  .drawer-btn.cancel:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.22);
    color: rgba(255, 255, 255, 0.62);
    background: rgba(255, 255, 255, 0.04);
  }

  .drawer-btn.submit {
    background: rgba(100, 190, 170, 0.1);
    border: 0.5px solid rgba(100, 190, 170, 0.24);
    color: rgba(100, 190, 170, 0.82);
  }

  .drawer-btn.submit:hover:not(:disabled) {
    background: rgba(100, 190, 170, 0.18);
    border-color: rgba(100, 190, 170, 0.36);
  }

  .drawer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  @media (max-width: 520px) {
    .drawer-actions {
      margin: 8px 10px 10px;
    }
  }
</style>
