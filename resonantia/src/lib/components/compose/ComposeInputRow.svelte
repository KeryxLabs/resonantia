<script lang="ts">
  export let draft = "";
  export let sessionId = "";
  export let loading = false;
  export let replyLoading = false;
  export let onDraftInput: () => void = () => {};
  export let sendComposeMessage: () => Promise<void> | void = () => {};
</script>

<div class="compose-entry">
  <textarea
    class="drawer-textarea compose-input"
    placeholder="message…"
    bind:value={draft}
    rows="3"
    on:input={onDraftInput}
    on:keydown={(event) => {
      if (event.key === 'Enter' && !event.shiftKey) {
        event.preventDefault();
        void sendComposeMessage();
      }
    }}
  ></textarea>
  <button
    type="button"
    class="drawer-btn submit compose-send"
    on:click={() => void sendComposeMessage()}
    disabled={loading || replyLoading || !draft.trim() || !sessionId.trim()}
  >
    {replyLoading ? 'thinking…' : 'send'}
  </button>
</div>

<style>
  .compose-entry {
    display: flex;
    gap: 8px;
    align-items: flex-end;
    flex-shrink: 0;
  }

  .drawer-textarea {
    width: 100%;
    box-sizing: border-box;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.026), rgba(255, 255, 255, 0.015));
    border: 0.5px solid rgba(167, 196, 219, 0.11);
    border-radius: 8px;
    padding: 9px 12px;
    color: rgba(200, 210, 220, 0.86);
    font-family: 'IBM Plex Sans', sans-serif;
    font-size: 12.5px;
    resize: vertical;
    margin-bottom: 0;
    outline: none;
    transition: border-color 0.2s;
    line-height: 1.45;
    box-shadow: inset 0 0 0 1px rgba(108, 144, 173, 0.07);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
  }

  .drawer-textarea::placeholder {
    color: rgba(255, 255, 255, 0.17);
    font-style: italic;
  }

  .drawer-textarea:focus {
    border-color: rgba(100, 190, 170, 0.26);
    box-shadow: inset 0 0 0 1px rgba(100, 190, 170, 0.18), 0 0 0 1px rgba(100, 190, 170, 0.05);
  }

  .compose-input {
    min-height: 40px;
    max-height: 84px;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.05em;
    padding: 9px 14px;
    border-radius: 8px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
    white-space: nowrap;
  }

  .drawer-btn.submit {
    background: linear-gradient(180deg, rgba(100, 190, 170, 0.11), rgba(78, 152, 136, 0.08));
    border: 0.5px solid rgba(100, 190, 170, 0.22);
    color: rgba(100, 190, 170, 0.74);
    box-shadow: 0 4px 10px rgba(16, 54, 48, 0.18);
  }

  .drawer-btn.submit:hover:not(:disabled) {
    background: rgba(100, 190, 170, 0.14);
    border-color: rgba(100, 190, 170, 0.28);
  }

  .drawer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .compose-send {
    min-width: 82px;
  }

  @media (max-width: 520px) {
    .compose-entry {
      flex-direction: column;
      align-items: stretch;
      gap: 6px;
    }

    .compose-send {
      width: 100%;
      min-width: 0;
    }

    .compose-input {
      min-height: 44px;
    }
  }
</style>
