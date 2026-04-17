<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import type { GraphSessionDto } from '@resonantia/core';

  type ResonanceDim = 'stability' | 'friction' | 'logic' | 'autonomy';

  type TimelineOption = {
    label: string;
    days: number;
  };

  export let scope: 'session' | 'sessions' | 'timeline' | 'resonance' = 'session';
  export let sessionId = '';
  export let sessionIds: string[] = [];
  export let sessions: GraphSessionDto[] = [];
  export let timelineDays = 30;
  export let timelineOptions: TimelineOption[] = [];
  export let resonanceDim: ResonanceDim = 'stability';
  export let psiMin = 0;

  export let loading = false;
  export let scopeScanning = false;

  export let preflightNodeCount = 0;
  export let preflightWindowLabel = '';
  export let preflightSessionCount = 0;
  export let preflightClipped = false;
  export let preflightLastScannedAt: string | null = null;

  export let scopeFresh = false;
  export let scopeStale = false;
  export let stageIntentLine = '';

  const dispatch = createEventDispatcher<{
    scopeChange: void;
    scan: void;
  }>();

  function formatScanTime(value: string | null): string {
    if (!value) return 'not scanned yet';
    const parsed = new Date(value);
    if (Number.isNaN(parsed.getTime())) return 'scan timestamp unavailable';
    return parsed.toLocaleString();
  }

  function sessionTitle(session: GraphSessionDto): string {
    const label = session.label?.trim() || 'untitled wave';
    return `${label} - ${session.nodeCount} nodes`;
  }

  function notifyScopeChange() {
    dispatch('scopeChange');
  }

  function isSessionSelected(sessionKey: string): boolean {
    return sessionIds.includes(sessionKey);
  }

  function toggleSession(sessionKey: string, checked: boolean) {
    const selected = new Set(sessionIds.map((id) => id.trim()).filter(Boolean));
    if (checked) {
      selected.add(sessionKey);
    } else {
      selected.delete(sessionKey);
    }
    sessionIds = [...selected];
    notifyScopeChange();
  }

  function selectAllSessions() {
    sessionIds = sessions.map((session) => session.id);
    notifyScopeChange();
  }

  function clearSelectedSessions() {
    if (sessionIds.length === 0) return;
    sessionIds = [];
    notifyScopeChange();
  }

  function handleScan() {
    dispatch('scan');
  }

  $: selectedSessionCount = sessionIds.length;
</script>

<section class="phase-card tone-stability" aria-label="phase 1">
  <p class="phase-id">phase 01</p>
  <h4>Gather feedstock</h4>
  <p class="phase-copy">Choose one scope and stabilize it before any other action appears.</p>

  <label class="field">
    <span>scope</span>
    <select class="input" bind:value={scope} disabled={loading || scopeScanning} on:change={notifyScopeChange}>
      <option value="session">one session</option>
      <option value="sessions">multiple sessions</option>
      <option value="timeline">one timeline window</option>
      <option value="resonance">one resonance frequency</option>
    </select>
  </label>

  {#if scope === 'session'}
    <label class="field">
      <span>session id</span>
      <select class="input" bind:value={sessionId} disabled={loading || scopeScanning} on:change={notifyScopeChange}>
        <option value="">select a wave</option>
        {#each sessions as session}
          <option value={session.id}>{sessionTitle(session)}</option>
        {/each}
      </select>
    </label>
  {:else if scope === 'sessions'}
    <div class="field">
      <div class="session-picker-head">
        <span>session set</span>
        <small>{selectedSessionCount} selected</small>
      </div>

      <div class="session-picker-actions">
        <button class="mini-btn" type="button" on:click={selectAllSessions} disabled={loading || scopeScanning || sessions.length === 0}>
          select all
        </button>
        <button class="mini-btn" type="button" on:click={clearSelectedSessions} disabled={loading || scopeScanning || sessionIds.length === 0}>
          clear
        </button>
      </div>

      <div class="session-picker" role="group" aria-label="session set">
        {#if sessions.length === 0}
          <p class="session-empty">no sessions available</p>
        {:else}
          {#each sessions as session}
            <label class="session-item">
              <input
                type="checkbox"
                checked={isSessionSelected(session.id)}
                disabled={loading || scopeScanning}
                on:change={(event) => toggleSession(session.id, (event.target as HTMLInputElement).checked)}
              />
              <span>{sessionTitle(session)}</span>
            </label>
          {/each}
        {/if}
      </div>
    </div>
  {:else if scope === 'timeline'}
    <label class="field">
      <span>timeline window</span>
      <select class="input" bind:value={timelineDays} disabled={loading || scopeScanning} on:change={notifyScopeChange}>
        {#each timelineOptions as option}
          <option value={option.days}>{option.label}</option>
        {/each}
      </select>
    </label>
  {:else}
    <div class="grid">
      <label class="field">
        <span>dominant signal</span>
        <select class="input" bind:value={resonanceDim} disabled={loading || scopeScanning} on:change={notifyScopeChange}>
          <option value="stability">stability</option>
          <option value="friction">friction</option>
          <option value="logic">logic</option>
          <option value="autonomy">autonomy</option>
        </select>
      </label>
      <label class="field">
        <span>minimum psi</span>
        <input class="input" type="number" min="0" max="4" step="0.1" bind:value={psiMin} disabled={loading || scopeScanning} on:input={notifyScopeChange} />
      </label>
    </div>
  {/if}

  <div class="actions">
    <button class="btn" on:click={handleScan} disabled={loading || scopeScanning}>
      {scopeScanning ? 'stabilizing...' : 'stabilize waveform'}
    </button>
  </div>

  <div class="preflight">
    <div class="preflight-head">
      <span>waveform state</span>
      <small>{formatScanTime(preflightLastScannedAt)}</small>
    </div>
    <p>{preflightNodeCount} nodes across {preflightSessionCount} sessions</p>
    <p>{stageIntentLine}</p>
    <p>{preflightWindowLabel}</p>
    {#if preflightClipped}
      <p class="warn">scope clipped for model context safety</p>
    {/if}
  </div>

  {#if scopeFresh}
    <p class="complete">Waveform stabilized. Phase 02 is ready.</p>
  {:else if scopeStale}
    <p class="lock">Scope changed after last scan. Stabilize again to continue.</p>
  {/if}
</section>

<style>
  .phase-card {
    --accent-rgb: 127, 194, 248;
    border: 0.5px solid rgba(var(--accent-rgb), 0.24);
    border-radius: 12px;
    padding: 10px;
    background: linear-gradient(180deg, rgba(9, 15, 25, 0.96), rgba(8, 12, 20, 0.93));
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

  .tone-stability {
    --accent-rgb: 127, 194, 248;
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

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .session-picker-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .session-picker-head span {
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(197, 219, 243, 0.62);
  }

  .session-picker-head small {
    font-size: 8px;
    color: rgba(216, 232, 249, 0.62);
  }

  .session-picker-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .mini-btn {
    border-radius: 999px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.32);
    background: rgba(var(--accent-rgb), 0.1);
    color: rgba(211, 230, 250, 0.88);
    padding: 5px 8px;
    font: 8px/1 'Departure Mono', monospace;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }

  .mini-btn:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .session-picker {
    max-height: 142px;
    overflow: auto;
    border-radius: 8px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.2);
    background: rgba(10, 17, 30, 0.62);
    padding: 6px;
    display: grid;
    gap: 4px;
  }

  .session-item {
    display: grid;
    grid-template-columns: auto 1fr;
    align-items: start;
    gap: 6px;
    font-size: 8px;
    color: rgba(209, 227, 246, 0.82);
  }

  .session-item input {
    margin: 1px 0 0;
    accent-color: rgba(var(--accent-rgb), 0.92);
  }

  .session-empty {
    margin: 0;
    font-size: 8px;
    color: rgba(177, 197, 220, 0.74);
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

  .preflight {
    border-radius: 9px;
    border: 0.5px solid rgba(var(--accent-rgb), 0.2);
    background: rgba(10, 17, 30, 0.62);
    padding: 7px;
    display: grid;
    gap: 2px;
  }

  .preflight-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    margin-bottom: 1px;
  }

  .preflight-head span {
    font-size: 8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(188, 220, 251, 0.8);
  }

  .preflight-head small {
    font-size: 8px;
    color: rgba(224, 236, 250, 0.54);
  }

  .preflight p {
    margin: 0;
    font-size: 8px;
    color: rgba(201, 220, 243, 0.7);
  }

  .warn {
    color: rgba(249, 214, 161, 0.92) !important;
  }

  .complete,
  .lock {
    margin: 0;
    font-size: 8px;
  }

  .complete {
    color: rgba(var(--accent-rgb), 0.92);
  }

  .lock {
    color: rgba(192, 220, 248, 0.62);
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

    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
