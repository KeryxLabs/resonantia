<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  type DecisionRoute = 'import' | 'export';

  export let selectedRoute: DecisionRoute | null = null;
  export let loading = false;
  export let scopeScanning = false;

  const dispatch = createEventDispatcher<{
    select: { route: DecisionRoute };
  }>();

  function choose(route: DecisionRoute) {
    dispatch('select', { route });
  }
</script>

<section class="phase-card tone-decision" aria-label="phase 0">
  <p class="phase-id">phase 00</p>
  <h4>Choose route</h4>
  <p class="phase-copy">Decide which ritual path to run before preparing feedstock.</p>

  <div class="route-grid">
    <button
      class="route-btn"
      class:active={selectedRoute === 'import'}
      data-tour-target="alkahest-import"
      on:click={() => choose('import')}
      disabled={loading || scopeScanning}
      type="button"
    >
      <span class="route-title">import</span>
      <span class="route-meta">Bring in one external STTP node and store it directly.</span>
    </button>

    <button
      class="route-btn"
      class:active={selectedRoute === 'export'}
      on:click={() => choose('export')}
      disabled={loading || scopeScanning}
      type="button"
    >
      <span class="route-title">export / distill</span>
      <span class="route-meta">Scan scoped memory, then export, distill, or both.</span>
    </button>
  </div>

  {#if selectedRoute}
    <p class="complete">Route committed. Proceed to phase 01.</p>
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

  .tone-decision {
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

  .route-grid {
    display: grid;
    gap: 8px;
  }

  .route-btn {
    border-radius: 10px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.26);
    background: rgba(16, 25, 38, 0.74);
    color: rgba(216, 232, 248, 0.86);
    padding: 9px 10px;
    text-align: left;
    display: grid;
    gap: 3px;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }

  .route-btn:hover:not(:disabled) {
    border-color: rgba(var(--accent-rgb), 0.62);
    background: rgba(32, 47, 70, 0.72);
  }

  .route-btn.active {
    border-color: rgba(var(--accent-rgb), 0.74);
    background: linear-gradient(170deg, rgba(65, 102, 151, 0.34), rgba(35, 57, 88, 0.28));
  }

  .route-btn:disabled {
    opacity: 0.56;
    cursor: default;
  }

  .route-title {
    font: 10px/1.2 'Departure Mono', monospace;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .route-meta {
    font-size: 8px;
    line-height: 1.45;
    color: rgba(198, 216, 236, 0.78);
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
</style>
