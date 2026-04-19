<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let targetSessionId = '';
  export let importNodeDraft = '';
  export let loading = false;
  export let scopeScanning = false;

  const dispatch = createEventDispatcher<{
    change: void;
  }>();

  function notifyChange() {
    dispatch('change');
  }

  $: draftTrimmed = importNodeDraft.trim();
  $: importReady = Boolean(targetSessionId.trim() && draftTrimmed);
</script>

<section class="phase-card tone-import" aria-label="phase 1 import prep">
  <p class="phase-id">phase 01a</p>
  <h4>Prepare import payload</h4>
  <p class="phase-copy">Set the target session and paste one complete STTP node for direct import.</p>

  <label class="field">
    <span>target session for imported node</span>
    <input
      class="input"
      type="text"
      placeholder="import-session-id"
      bind:value={targetSessionId}
      disabled={loading || scopeScanning}
      on:input={notifyChange}
    />
  </label>

  <label class="field">
    <span>raw STTP node payload</span>
    <textarea
      class="input textarea"
      placeholder="paste one complete STTP node"
      bind:value={importNodeDraft}
      rows="8"
      disabled={loading || scopeScanning}
      on:input={notifyChange}
    ></textarea>
  </label>

  <p class="hint">
    {#if draftTrimmed}
      {draftTrimmed.length} characters staged for import.
    {:else}
      Paste one complete node to unlock phase 02.
    {/if}
  </p>

  {#if importReady}
    <p class="complete">Import payload prepared. Phase 02 is ready.</p>
  {/if}
</section>

<style>
  .phase-card {
    --accent-rgb: 126, 172, 242;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    border-radius: 12px;
    padding: 10px;
    background: linear-gradient(180deg, rgba(9, 16, 25, 0.96), rgba(8, 13, 20, 0.93));
    display: grid;
    gap: 8px;
    position: relative;
    overflow: hidden;
  }

  .phase-card::before {
    content: '';
    position: absolute;
    top: 0;
    left: 10px;
    right: 10px;
    height: 1px;
    background: linear-gradient(90deg, rgba(var(--accent-rgb), 0.1), rgba(var(--accent-rgb), 0.86), rgba(var(--accent-rgb), 0.1));
  }

  .tone-import {
    --accent-rgb: 126, 172, 242;
  }

  .phase-id {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: rgba(var(--accent-rgb), 0.86);
  }

  h4 {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 400;
    font-style: italic;
    font-size: 15px;
    color: rgba(233, 243, 255, 0.92);
  }

  .phase-copy {
    margin: 0;
    font-size: 9px;
    line-height: 1.4;
    color: rgba(192, 210, 230, 0.72);
  }

  .field {
    display: grid;
    gap: 4px;
  }

  .field > span {
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(197, 219, 243, 0.62);
  }

  .input {
    width: 100%;
    box-sizing: border-box;
    border-radius: 7px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    background: rgba(4, 10, 18, 0.86);
    color: rgba(232, 242, 255, 0.86);
    font: 10px/1.25 'Departure Mono', monospace;
    padding: 7px 8px;
    outline: none;
  }

  .input:focus {
    border-color: rgba(var(--accent-rgb), 0.66);
    box-shadow: 0 0 0 1px rgba(var(--accent-rgb), 0.18);
  }

  .textarea {
    resize: vertical;
    min-height: 146px;
    font-family: 'IBM Plex Sans', sans-serif;
    font-size: 11px;
    line-height: 1.45;
    letter-spacing: 0.01em;
  }

  .hint {
    margin: 0;
    font-size: 8px;
    line-height: 1.45;
    color: rgba(192, 220, 247, 0.76);
    text-transform: lowercase;
  }

  .complete {
    margin: 0;
    font-size: 8px;
    color: rgba(var(--accent-rgb), 0.92);
  }

  @media (max-width: 760px) {
    .phase-card {
      padding: 8px;
      gap: 6px;
    }

    h4 {
      font-size: 14px;
    }

    .phase-copy {
      font-size: 8px;
    }
  }

  @media (hover: none) and (pointer: coarse) {
    .input,
    textarea.input {
      font-size: 16px;
      line-height: 1.3;
    }
  }
</style>
