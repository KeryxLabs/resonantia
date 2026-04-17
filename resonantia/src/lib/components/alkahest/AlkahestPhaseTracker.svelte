<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let activePhase = 1;
  export let selectedPhase = 1;
  export let completionPct = 0;
  export let phase1Complete = false;
  export let phase2Complete = false;
  export let phase3Complete = false;
  export let phase2Unlocked = false;
  export let phase3Unlocked = false;

  const dispatch = createEventDispatcher<{
    select: { phase: number };
  }>();

  function selectPhase(phase: number, unlocked: boolean) {
    if (!unlocked) return;
    dispatch('select', { phase });
  }
</script>

<section class="tracker" aria-label="ritual progression">
  <div class="tracker-head">
    <span>ritual arc</span>
    <small>step {activePhase} of 3 - {completionPct}%</small>
  </div>
  <div class="tracker-bar" aria-hidden="true">
    <i style={`width:${completionPct}%`}></i>
  </div>
  <ol class="tracker-steps">
    <li class:active={selectedPhase === 1} class:complete={phase1Complete}>
      <button class="tracker-step-btn" type="button" on:click={() => selectPhase(1, true)}>
        stabilize waveform
      </button>
    </li>
    <li class:active={selectedPhase === 2} class:complete={phase2Complete} class:locked={!phase2Unlocked}>
      <button class="tracker-step-btn" type="button" disabled={!phase2Unlocked} on:click={() => selectPhase(2, phase2Unlocked)}>
        bind intention
      </button>
    </li>
    <li class:active={selectedPhase === 3} class:complete={phase3Complete} class:locked={!phase3Unlocked}>
      <button class="tracker-step-btn" type="button" disabled={!phase3Unlocked} on:click={() => selectPhase(3, phase3Unlocked)}>
        extract quintessence
      </button>
    </li>
  </ol>
</section>

<style>
  .tracker {
    border: 0.5px solid rgba(168, 206, 244, 0.2);
    border-radius: 12px;
    background: rgba(12, 20, 34, 0.5);
    padding: 8px;
    display: grid;
    gap: 6px;
  }

  .tracker-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .tracker-head span {
    font-size: 8px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(188, 220, 252, 0.72);
  }

  .tracker-head small {
    font-size: 8px;
    color: rgba(216, 232, 250, 0.62);
  }

  .tracker-bar {
    height: 5px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .tracker-bar i {
    display: block;
    height: 100%;
    border-radius: inherit;
    background: linear-gradient(90deg, rgba(119, 184, 248, 0.78), rgba(169, 230, 200, 0.9));
    box-shadow: 0 0 10px rgba(132, 190, 239, 0.24);
    transition: width 0.24s ease;
  }

  .tracker-steps {
    list-style: none;
    margin: 0;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 6px;
  }

  .tracker-steps li {
    --step-rgb: 127, 194, 248;
    border-radius: 8px;
    border: 0.5px solid rgba(var(--step-rgb), 0.22);
    background: rgba(var(--step-rgb), 0.08);
    padding: 5px;
  }

  .tracker-steps li:nth-child(1) {
    --step-rgb: 127, 194, 248;
  }

  .tracker-steps li:nth-child(2) {
    --step-rgb: 147, 230, 187;
  }

  .tracker-steps li:nth-child(3) {
    --step-rgb: 230, 200, 82;
  }

  .tracker-step-btn {
    width: 100%;
    border: 0;
    background: transparent;
    text-align: center;
    font-size: 7px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(220, 233, 248, 0.82);
    padding: 0;
    cursor: pointer;
    font-family: inherit;
  }

  .tracker-step-btn:disabled {
    cursor: default;
    color: rgba(170, 186, 206, 0.42);
  }

  .tracker-steps li.locked {
    border-color: rgba(132, 154, 178, 0.22);
    background: rgba(48, 63, 82, 0.2);
  }

  .tracker-steps li.active {
    border-color: rgba(var(--step-rgb), 0.72);
    background: linear-gradient(180deg, rgba(var(--step-rgb), 0.28), rgba(var(--step-rgb), 0.12));
    box-shadow: 0 0 0 1px rgba(var(--step-rgb), 0.18) inset;
  }

  .tracker-steps li.complete {
    border-color: rgba(var(--step-rgb), 0.62);
    background: rgba(var(--step-rgb), 0.16);
  }

  @media (max-width: 760px) {
    .tracker {
      padding: 7px;
      gap: 5px;
    }

    .tracker-head small {
      font-size: 7px;
    }

    .tracker-steps {
      grid-template-columns: 1fr;
    }

    .tracker-steps li {
      font-size: 6px;
      padding: 4px;
    }
  }
</style>
