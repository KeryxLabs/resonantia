<script lang="ts">
  export let sessionId = "";
  export let pasteNodeDraft = "";
  export let pasteNodeLoading = false;
  export let pastePrettyView = false;
  export let pasteNodePreviewHtml = "";
  export let pasteInputEl: HTMLTextAreaElement | null = null;
  export let pastePreviewEl: HTMLDivElement | null = null;
  export let togglePastePrettyView: () => void = () => {};
  export let syncPasteEditorScroll: () => void = () => {};
  export let toggleComposePasteNode: () => void = () => {};
  export let saveComposePastedNode: () => Promise<void> | void = () => {};
</script>

<div class="compose-paste-panel">
  <p class="compose-paste-intro">paste a complete STTP node and save it directly.</p>
  <div class="compose-paste-toolbar">
    <button class="compose-link-btn" type="button" on:click={togglePastePrettyView}>
      {pastePrettyView ? 'pretty view on' : 'pretty view off'}
    </button>
    {#if pastePrettyView}
      <span class="compose-paste-mode-note">visual only</span>
    {/if}
  </div>
  <div class="compose-paste-editor" class:pretty={pastePrettyView}>
    <div class="compose-paste-preview-wrap" bind:this={pastePreviewEl} aria-hidden="true">
      <pre class="compose-paste-preview">{@html pasteNodePreviewHtml}</pre>
    </div>
    {#if !pastePrettyView}
      <textarea
        class="drawer-textarea compose-paste-input compose-paste-input-highlighted"
        placeholder="paste one full STTP node"
        bind:this={pasteInputEl}
        bind:value={pasteNodeDraft}
        rows="9"
        wrap="soft"
        on:input={syncPasteEditorScroll}
        on:scroll={syncPasteEditorScroll}
      ></textarea>
    {/if}
  </div>
  <div class="compose-paste-actions">
    <button class="drawer-btn cancel" on:click={toggleComposePasteNode} disabled={pasteNodeLoading}>cancel paste</button>
    <button class="drawer-btn submit" on:click={saveComposePastedNode} disabled={pasteNodeLoading || !pasteNodeDraft.trim() || !sessionId.trim()}>
      {pasteNodeLoading ? 'saving…' : 'save pasted node'}
    </button>
  </div>
</div>

<style>
  .compose-link-btn {
    border: 0.5px solid rgba(126, 173, 198, 0.24);
    background: rgba(80, 119, 143, 0.09);
    padding: 4px 9px;
    margin: 0;
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    border-radius: 999px;
    color: rgba(191, 223, 242, 0.72);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .compose-paste-panel {
    margin-top: 7px;
    padding: 10px;
    border-radius: 10px;
    border: 0.5px dashed rgba(255, 255, 255, 0.11);
    background: rgba(255, 255, 255, 0.012);
  }

  .compose-paste-intro {
    margin: 0 0 8px;
    font-size: 9px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.48);
    text-transform: lowercase;
  }

  .compose-paste-input {
    min-height: var(--compose-paste-height, 184px);
    height: var(--compose-paste-height, 184px);
    margin-bottom: 0;
    resize: none;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
    overflow-x: hidden;
  }

  .compose-paste-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 7px;
  }

  .compose-paste-mode-note {
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    color: rgba(168, 188, 230, 0.74);
  }

  .compose-paste-editor {
    position: relative;
    min-height: var(--compose-paste-height, 184px);
    height: var(--compose-paste-height, 184px);
    margin-bottom: 0;
    min-width: 0;
  }

  .compose-paste-editor.pretty {
    border: 0.5px solid rgba(170, 193, 240, 0.24);
    border-radius: 6px;
    box-shadow: inset 0 0 0 1px rgba(112, 142, 204, 0.12);
  }

  .compose-paste-preview-wrap {
    position: absolute;
    inset: 0;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    background: rgba(7, 8, 12, 0.9);
    overflow: auto;
    pointer-events: none;
  }

  .compose-paste-preview {
    margin: 0;
    padding: 8px 10px;
    font-size: 11px;
    line-height: 1.45;
    color: rgba(233, 235, 242, 0.88);
    white-space: pre-wrap;
    word-break: break-word;
    overflow-wrap: anywhere;
    min-height: 100%;
    max-width: 100%;
    box-sizing: border-box;
  }

  .compose-paste-preview :global(span) {
    overflow-wrap: anywhere;
    word-break: break-word;
    max-width: 100%;
  }

  .compose-paste-preview :global(.sttp-empty) {
    color: rgba(255, 255, 255, 0.38);
    font-style: italic;
  }

  .compose-paste-preview :global(.sttp-marker) {
    color: #f7c97b;
  }

  .compose-paste-preview :global(.sttp-brace) {
    color: rgba(214, 221, 255, 0.82);
  }

  .compose-paste-preview :global(.sttp-key) {
    color: #7cc6ff;
  }

  .compose-paste-preview :global(.sttp-confidence) {
    color: #ffd68f;
  }

  .compose-paste-preview :global(.sttp-number) {
    color: #8be6a8;
  }

  .compose-paste-preview :global(.sttp-keyword) {
    color: #efc995;
  }

  .drawer-textarea {
    width: 100%;
    box-sizing: border-box;
    background: rgba(255, 255, 255, 0.04);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 10px;
    color: rgba(255, 255, 255, 0.7);
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    resize: vertical;
    margin-bottom: 10px;
    outline: none;
    transition: border-color 0.2s;
  }

  .compose-paste-input-highlighted {
    position: relative;
    z-index: 1;
    background: transparent;
    border-color: rgba(255, 255, 255, 0.14);
    color: transparent;
    -webkit-text-fill-color: transparent;
    caret-color: rgba(244, 247, 255, 0.92);
    overflow-x: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .compose-paste-input-highlighted::selection {
    background: rgba(143, 183, 255, 0.28);
  }

  .compose-paste-input-highlighted::placeholder {
    color: transparent;
  }

  .compose-paste-editor.pretty .compose-paste-preview-wrap {
    position: relative;
    border: none;
    border-radius: 6px;
  }

  .compose-paste-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 6px 14px;
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
    background: rgba(255, 255, 255, 0.06);
    border: 0.5px solid rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.8);
  }

  .drawer-btn.submit:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.35);
  }

  .drawer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  @media (max-width: 520px) {
    .compose-paste-actions {
      flex-direction: row;
      align-items: center;
      justify-content: flex-end;
      gap: 6px;
    }

    .compose-paste-actions .drawer-btn {
      width: auto;
      text-align: center;
      padding: 5px 10px;
    }
  }
</style>
