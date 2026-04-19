<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  type AlkahestMode = 'export' | 'distill' | 'both' | 'import';
  type DecisionRoute = 'import' | 'export';

  export let mode: AlkahestMode = 'both';
  export let targetSessionId = '';
  export let decisionRoute: DecisionRoute = 'export';
  export let modeLocked = false;
  export let importPayloadLength = 0;
  export let loading = false;
  export let scopeScanning = false;
  export let phase2Complete = false;
  export let modeNarrative = '';

  const dispatch = createEventDispatcher<{
    change: void;
    seal: void;
  }>();

  function notifyChange() {
    dispatch('change');
  }

  function seal() {
    dispatch('seal');
  }
</script>

<section class="phase-card tone-logic" aria-label="phase 2">
  <p class="phase-id">phase 02</p>
  <h4>Bind intention</h4>
  <p class="phase-copy">Choose the transmutation mode and lock the target session for this extraction.</p>

  {#if modeLocked}
    <div class="field">
      <span>operation</span>
      <p class="mode-lock-label">import one raw STTP node</p>
    </div>
  {:else}
    <label class="field">
      <span>operation</span>
      <select class="input" bind:value={mode} disabled={loading || scopeScanning} on:change={notifyChange}>
        <option value="export">liquefy only (export json)</option>
        <option value="distill">distill only (super node)</option>
        <option value="both">liquefy + distill</option>
      </select>
    </label>
  {/if}

  <p class="mode-copy">{modeNarrative}</p>

  {#if mode !== 'export'}
    <label class="field">
      <span>{mode === 'import' ? 'target session for imported node' : 'target session for super node'}</span>
      <input
        class="input"
        type="text"
        placeholder={mode === 'import' ? 'import-session-id' : 'alkahest-monthly'}
        bind:value={targetSessionId}
        disabled={loading || scopeScanning}
        on:input={notifyChange}
      />
    </label>

    {#if decisionRoute !== 'import'}
      <p class="internal-note">distillation prompt is managed by the lab attunement layer.</p>
    {:else}
      <p class="mode-copy">{importPayloadLength > 0 ? `${importPayloadLength} payload characters staged from phase 01a.` : 'No import payload staged yet.'}</p>
    {/if}
  {/if}

  <div class="actions">
    <button class="btn" on:click={seal} disabled={loading || scopeScanning}>
      {phase2Complete ? 'intention sealed' : 'seal intention'}
    </button>
  </div>

  {#if phase2Complete}
    <p class="complete">Intention sealed. Phase 03 is ready.</p>
  {/if}
</section>

<style>
  .phase-card {
    --accent-rgb: 147, 230, 187;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    border-radius: 12px;
    padding: 10px;
    background: linear-gradient(180deg, rgba(9, 16, 24, 0.96), rgba(8, 13, 20, 0.93));
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
    background: linear-gradient(90deg, rgba(var(--accent-rgb), 0.1), rgba(var(--accent-rgb), 0.84), rgba(var(--accent-rgb), 0.1));
  }

  .tone-logic {
    --accent-rgb: 147, 230, 187;
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
    color: rgba(237, 246, 255, 0.92);
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

  .mode-lock-label {
    margin: 0;
    border-radius: 7px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    background: rgba(4, 10, 18, 0.86);
    color: rgba(232, 242, 255, 0.86);
    font: 10px/1.25 'Departure Mono', monospace;
    padding: 7px 8px;
    text-transform: lowercase;
  }

  .mode-copy {
    margin: -2px 0 0;
    font-size: 8px;
    line-height: 1.45;
    color: rgba(192, 220, 247, 0.76);
  }

  .internal-note {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(170, 206, 239, 0.56);
  }

  .actions {
    display: flex;
  }

  .btn {
    border-radius: 999px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.46);
    background: rgba(var(--accent-rgb), 0.14);
    color: rgba(219, 236, 255, 0.9);
    padding: 8px 12px;
    font: 9px/1 'Departure Mono', monospace;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .btn:disabled {
    opacity: 0.55;
    cursor: default;
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
    select.input,
    input.input {
      font-size: 16px;
      line-height: 1.3;
    }
  }
</style>
