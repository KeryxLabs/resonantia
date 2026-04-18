<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let loading = false;
  export let scopeScanning = false;
  export let runLabel = 'begin extraction';
  export let providerLabel = 'unknown';
  export let preflightWindowLabel = '';
  export let preflightNodeCount = 0;
  export let preflightSessionCount = 0;
  export let completionPct = 0;
  export let superNodePreview = '';

  const dispatch = createEventDispatcher<{
    run: void;
    copy: void;
  }>();

  $: nodeScore = Math.min(100, Math.round((preflightNodeCount / 240) * 100));
  $: sessionScore = Math.min(100, Math.round((preflightSessionCount / 24) * 100));
  $: chargeScore = Math.min(100, Math.max(18, completionPct));

  const statRows = [
    { label: 'node density', value: () => nodeScore, raw: () => `${preflightNodeCount} nodes` },
    { label: 'session spread', value: () => sessionScore, raw: () => `${preflightSessionCount} sessions` },
    { label: 'ritual charge', value: () => chargeScore, raw: () => `${completionPct}%` },
  ];

  function run() {
    dispatch('run');
  }

  function copy() {
    dispatch('copy');
  }
</script>

<section class="phase-card tone-autonomy" aria-label="phase 3">
  <p class="phase-id">phase 03</p>
  <h4>Extract quintessence</h4>
  <p class="phase-copy">Read the final telemetry signature, then complete the extraction ritual.</p>

  <div class="manifest">
    <p><span>provider</span>{providerLabel}</p>
    <p><span>window</span>{preflightWindowLabel}</p>
  </div>

  <section class="stats" aria-label="ritual telemetry">
    <p class="stats-title">ritual telemetry</p>
    {#each statRows as row}
      <div class="stat-row">
        <div class="stat-labels">
          <span>{row.label}</span>
          <small>{row.raw()}</small>
        </div>
        <div class="stat-track" aria-hidden="true">
          <i style={`width:${row.value()}%`}></i>
        </div>
      </div>
    {/each}
  </section>

  <div class="actions">
    <button class="btn primary" on:click={run} disabled={loading || scopeScanning}>
      {runLabel}
    </button>
  </div>

  {#if superNodePreview.trim()}
    <section class="preview" aria-label="super node preview">
      <div class="preview-head">
        <span>super node preview</span>
        <button class="link" on:click={copy} disabled={loading || scopeScanning}>copy</button>
      </div>
      <pre>{superNodePreview}</pre>
    </section>
  {/if}
</section>

<style>
  .phase-card {
    --accent-rgb: 230, 200, 82;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    border-radius: 12px;
    padding: 10px;
    background: linear-gradient(180deg, rgba(11, 14, 21, 0.96), rgba(8, 12, 18, 0.94));
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
    background: linear-gradient(90deg, rgba(var(--accent-rgb), 0.08), rgba(var(--accent-rgb), 0.82), rgba(var(--accent-rgb), 0.08));
  }

  .tone-autonomy {
    --accent-rgb: 230, 200, 82;
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
    color: rgba(204, 212, 224, 0.72);
  }

  .manifest {
    border-radius: 9px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.18);
    background: rgba(15, 17, 24, 0.64);
    padding: 7px;
    display: grid;
    gap: 3px;
  }

  .manifest p {
    margin: 0;
    display: grid;
    grid-template-columns: 56px 1fr;
    align-items: center;
    gap: 6px;
    font-size: 8px;
    color: rgba(208, 225, 245, 0.82);
  }

  .manifest span {
    font-size: 8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(188, 220, 251, 0.72);
  }

  .stats {
    border-radius: 9px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.18);
    background: rgba(15, 17, 24, 0.64);
    padding: 7px;
    display: grid;
    gap: 6px;
  }

  .stats-title {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.13em;
    text-transform: uppercase;
    color: rgba(188, 220, 251, 0.84);
  }

  .stat-row {
    display: grid;
    gap: 3px;
  }

  .stat-labels {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .stat-labels span {
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(200, 224, 246, 0.72);
  }

  .stat-labels small {
    font-size: 8px;
    color: rgba(216, 232, 249, 0.62);
  }

  .stat-track {
    height: 5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .stat-track i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, rgba(118, 183, 247, 0.82), rgba(var(--accent-rgb), 0.88));
    box-shadow: 0 0 10px rgba(190, 168, 84, 0.24);
    transition: width 0.24s ease;
  }

  .actions {
    display: flex;
  }

  .btn {
    border-radius: 999px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.4);
    background: rgba(var(--accent-rgb), 0.12);
    color: rgba(219, 236, 255, 0.9);
    padding: 8px 12px;
    font: 9px/1 'Departure Mono', monospace;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .btn.primary {
    background: linear-gradient(90deg, rgba(96, 146, 206, 0.3), rgba(142, 120, 49, 0.3));
    border-color: rgba(var(--accent-rgb), 0.54);
  }

  .btn:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .preview {
    border-top: 0.5px solid rgba(170, 205, 240, 0.2);
    margin-top: 2px;
    padding-top: 9px;
    display: grid;
    gap: 7px;
  }

  .preview-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .preview-head span {
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(202, 226, 247, 0.82);
  }

  .link {
    border: none;
    background: transparent;
    color: rgba(176, 217, 255, 0.88);
    font: 9px/1 'Departure Mono', monospace;
    cursor: pointer;
    padding: 0;
  }

  pre {
    margin: 0;
    max-height: 160px;
    overflow: auto;
    border-radius: 8px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.16);
    background: rgba(7, 10, 16, 0.86);
    color: rgba(234, 244, 255, 0.88);
    font: 9px/1.4 'Departure Mono', monospace;
    padding: 7px;
    white-space: pre-wrap;
    word-break: break-word;
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

    pre {
      max-height: 130px;
    }
  }

</style>
