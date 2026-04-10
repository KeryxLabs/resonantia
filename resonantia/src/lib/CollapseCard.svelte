<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { AVEC_HEX, avecColor, formatTimestamp } from './avec';
  import type { CollapseCardData } from './types';

  export let data: CollapseCardData | null = null;
  export let visible = false;

  const dispatch = createEventDispatcher<{
    close: void;
    navigate: { sessionId: string };
  }>();

  $: avec = data?.nodeDto?.userAvec ?? null;

  $: rho   = data?.nodeDto?.rho   ?? null;
  $: kappa = data?.nodeDto?.kappa ?? null;
  $: psi   = data?.nodeDto?.userAvec.psi ?? data?.node.psi ?? null;

  $: sessionLabel = data?.node.sessionId.replace(/_/g, ' ') ?? '—';
  $: timestamp    = data?.node.timestamp ? formatTimestamp(data.node.timestamp) : '—';
  $: tier         = data?.node.tier ?? '—';

  function bar(val: number | null) {
    return `${((val ?? 0) * 100).toFixed(0)}%`;
  }
</script>

<div
  class="card"
  class:visible
  role="complementary"
  aria-label="Collapse detail"
>
  <div class="card-header">
    <div class="left">
      <span class="session-label">{sessionLabel}</span>
      <span class="tier-pill">{tier}</span>
    </div>
    <button class="close-btn" on:click={() => dispatch('close')} aria-label="Close">✕</button>
  </div>

  <div class="timestamp">{timestamp}</div>

  {#if avec}
    <div class="avec-grid">
      {#each [
        { key: 'stability' as const, val: avec.stability },
        { key: 'friction'  as const, val: avec.friction  },
        { key: 'logic'     as const, val: avec.logic      },
        { key: 'autonomy'  as const, val: avec.autonomy   },
      ] as dim}
        <div class="dim">
          <span class="dim-label">{dim.key}</span>
          <div class="bar-track">
            <div
              class="bar-fill"
              style="width:{bar(dim.val)};background:{AVEC_HEX[dim.key]}"
            ></div>
          </div>
          <span class="dim-val">{dim.val.toFixed(2)}</span>
        </div>
      {/each}
    </div>
  {/if}

  <div class="divider"></div>

  <div class="metrics">
    <div class="metric">
      <span class="metric-label">Ψ</span>
      <span class="metric-val">{psi != null ? psi.toFixed(4) : '—'}</span>
    </div>
    <div class="metric">
      <span class="metric-label">ρ signal</span>
      <span class="metric-val">{rho != null ? rho.toFixed(2) : '—'}</span>
    </div>
    <div class="metric">
      <span class="metric-label">κ coherence</span>
      <span class="metric-val">{kappa != null ? kappa.toFixed(2) : '—'}</span>
    </div>
  </div>

  {#if data?.relatedSessions?.length}
    <div class="threads">
      {#each data.relatedSessions.slice(0, 4) as s}
        <button
          class="thread-tag"
          on:click={() => dispatch('navigate', { sessionId: s.id })}
        >
          ⟶ {s.label.split('_').slice(0, 2).join(' ')}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .card {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%) translateY(calc(100% + 32px));
    opacity: 0;
    width: 340px;
    background: rgba(12, 14, 20, 0.96);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 20px;
    z-index: 30;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    transition:
      transform 0.5s cubic-bezier(0.16, 1, 0.3, 1),
      opacity   0.4s ease;
    pointer-events: none;
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  .card.visible {
    transform: translateX(-50%) translateY(0);
    opacity: 1;
    pointer-events: all;
  }

  .card-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .left {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .session-label {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 14px;
    color: rgba(255, 255, 255, 0.75);
    line-height: 1.3;
    max-width: 220px;
  }

  .tier-pill {
    display: inline-block;
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.25);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 99px;
    padding: 2px 7px;
    align-self: flex-start;
  }

  .close-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.3);
    font-size: 14px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.2s;
  }
  .close-btn:hover { color: rgba(255, 255, 255, 0.7); }

  .timestamp {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.25);
    letter-spacing: 0.06em;
    margin-bottom: 16px;
  }

  .avec-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    margin-bottom: 16px;
  }

  .dim {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .dim-label {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.3);
  }

  .bar-track {
    height: 2px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 2px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.7s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .dim-val {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
  }

  .divider {
    height: 0.5px;
    background: rgba(255, 255, 255, 0.06);
    margin: 4px 0 14px;
  }

  .metrics {
    display: flex;
    gap: 20px;
  }

  .metric {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .metric-label {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.25);
  }

  .metric-val {
    font-size: 17px;
    color: rgba(255, 255, 255, 0.8);
    letter-spacing: 0.02em;
  }

  .threads {
    margin-top: 14px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .thread-tag {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.06em;
    padding: 3px 9px;
    border-radius: 99px;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: transparent;
    color: rgba(255, 255, 255, 0.3);
    cursor: pointer;
    transition: all 0.2s;
  }
  .thread-tag:hover {
    border-color: rgba(255, 255, 255, 0.25);
    color: rgba(255, 255, 255, 0.7);
  }
</style>
