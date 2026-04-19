<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import type { GraphSessionDto, ModelProvider } from '@resonantia/core';
  import AlkahestStone from './alkahest/AlkahestStone.svelte';
  import AlkahestPhaseTracker from './alkahest/AlkahestPhaseTracker.svelte';
  import AlkahestPhaseZeroCard from './alkahest/AlkahestPhaseZeroCard.svelte';
  import AlkahestPhaseOneCard from './alkahest/AlkahestPhaseOneCard.svelte';
  import AlkahestPhaseOneImportCard from './alkahest/AlkahestPhaseOneImportCard.svelte';
  import AlkahestPhaseTwoCard from './alkahest/AlkahestPhaseTwoCard.svelte';
  import AlkahestPhaseThreeCard from './alkahest/AlkahestPhaseThreeCard.svelte';

  type AlkahestScopeKind = 'session' | 'sessions' | 'timeline' | 'resonance';
  type AlkahestMode = 'export' | 'distill' | 'both' | 'import';
  type ResonanceDim = 'stability' | 'friction' | 'logic' | 'autonomy';

  type TimelineOption = {
    label: string;
    days: number;
  };

  const INTERNAL_DISTILL_PROMPT = [
    'You are the Alkahest memory distiller for STTP.',
    'Read all source nodes and produce one durable super node that preserves chronology, unresolved threads,',
    'decision context, and confidence-weighted meaning.',
    'Return exactly one valid STTP node and nothing else.',
  ].join('\n');

  const STAGE_COPY = 'The chamber remains steady while the ritual control card advances.';

  export let hidden = false;
  export let open = false;
  export let cameraEngaged = false;
  export let phase: 'idle' | 'entering' | 'exiting' = 'idle';

  export let loading = false;
  export let scopeScanning = false;
  export let error: string | null = null;
  export let status: string | null = null;

  export let scope: AlkahestScopeKind = 'session';
  export let mode: AlkahestMode = 'both';
  export let sessionId = '';
  export let sessionIds: string[] = [];
  export let sessions: GraphSessionDto[] = [];

  export let timelineDays = 30;
  export let timelineOptions: TimelineOption[] = [];

  export let resonanceDim: ResonanceDim = 'stability';
  export let psiMin = 0;

  export let prompt = '';
  export let targetSessionId = 'alkahest-monthly';
  export let importNodeDraft = '';
  export let storeDistilledNode = true;

  export let preflightNodeCount = 0;
  export let preflightWindowLabel = '';
  export let preflightSessionCount = 0;
  export let preflightClipped = false;
  export let preflightLastScannedAt: string | null = null;

  export let modelProvider: ModelProvider = 'managed-gateway';
  export let superNodePreview = '';

  export let toggleOpen: () => void = () => {};
  export let closePanel: () => void = () => {};
  export let cancelAndReset: () => void | Promise<void> = () => {};
  export let scanScope: () => void | Promise<void> = () => {};
  export let runAlkahest: () => void | Promise<void> = () => {};
  export let copySuperNode: () => void | Promise<void> = () => {};

  let wasOpen = false;
  let lastObservedScanStamp = '';
  let scannedScopeKey = '';
  let decisionCommitted = false;
  let phase2Committed = false;
  let committedPhase2Key = '';
  let selectedPhase = 0;
  let previousMaxVisiblePhase = 0;
  let decisionRoute: 'import' | 'export' = 'export';

  function currentScopeKeyFor(): string {
    if (scope === 'session') {
      return `session:${sessionId}`;
    }
    if (scope === 'sessions') {
      const normalizedSessionIds = [...sessionIds]
        .map((id) => id.trim())
        .filter(Boolean)
        .sort();
      return `sessions:${normalizedSessionIds.join(',')}`;
    }
    if (scope === 'timeline') {
      return `timeline:${timelineDays}`;
    }
    return `resonance:${resonanceDim}:${psiMin}`;
  }

  function currentPhase2KeyFor(): string {
    const normalizedTarget = targetSessionId.trim();
    if (mode === 'import') {
      const normalizedImportDraft = importNodeDraft.trim();
      const fingerprint = normalizedImportDraft.slice(0, 96);
      return `${mode}|${normalizedTarget}|${normalizedImportDraft.length}|${fingerprint}`;
    }

    return `${mode}|${normalizedTarget}|${storeDistilledNode ? 'store' : 'skip'}`;
  }

  function clearPhase2Commit() {
    phase2Committed = false;
    committedPhase2Key = '';
  }

  function handleDecisionSelect(event: CustomEvent<{ route: 'import' | 'export' }>) {
    decisionCommitted = true;

    if (event.detail.route === 'import') {
      mode = 'import';
      selectedPhase = 1;
    } else {
      if (mode === 'import') {
        mode = 'both';
      }
      selectedPhase = 1;
    }

    clearPhase2Commit();
  }

  function handleScopeChange() {
    clearPhase2Commit();
  }

  function handleImportPrepChange() {
    clearPhase2Commit();
  }

  function handlePhase2Change() {
    clearPhase2Commit();
  }

  function sealPhase2() {
    phase2Committed = true;
    committedPhase2Key = currentPhase2Key;
  }

  function handleClosePointerDown(event: PointerEvent) {
    if (event.target === event.currentTarget) {
      closePanel();
    }
  }

  function handleCancel() {
    scannedScopeKey = '';
    lastObservedScanStamp = '';
    wasOpen = false;
    decisionCommitted = false;
    clearPhase2Commit();
    cancelAndReset();
  }

  function handleTrackerSelect(event: CustomEvent<{ phase: number }>) {
    selectedPhase = event.detail.phase;
  }

  $: if (!prompt.trim()) {
    prompt = INTERNAL_DISTILL_PROMPT;
  }

  $: storeDistilledNode = mode === 'distill' || mode === 'both';

  $: currentScopeKey = currentScopeKeyFor();
  $: currentPhase2Key = currentPhase2KeyFor();
  $: scanStamp = preflightLastScannedAt ?? '';

  $: if (open && !wasOpen) {
    wasOpen = true;
    decisionCommitted = false;
    selectedPhase = 0;
    previousMaxVisiblePhase = 0;
    lastObservedScanStamp = scanStamp;
    if (scanStamp) {
      scannedScopeKey = currentScopeKey;
    }
  }

  $: if (!open && wasOpen) {
    wasOpen = false;
    lastObservedScanStamp = scanStamp;
  }

  $: if (open && scanStamp && scanStamp !== lastObservedScanStamp) {
    lastObservedScanStamp = scanStamp;
    scannedScopeKey = currentScopeKey;
    clearPhase2Commit();
  }

  $: importMode = mode === 'import';
  $: decisionRoute = importMode ? 'import' : 'export';
  $: importPrepared = Boolean(targetSessionId.trim() && importNodeDraft.trim());
  $: decisionComplete = decisionCommitted;
  $: scopeFresh = scannedScopeKey !== '' && scannedScopeKey === currentScopeKey;
  $: scopeStale = scannedScopeKey !== '' && scannedScopeKey !== currentScopeKey;

  $: phase1Complete = decisionComplete && (importMode ? importPrepared : scopeFresh);
  $: phase2Complete = phase1Complete && phase2Committed && committedPhase2Key === currentPhase2Key;
  $: phase3Complete = phase2Complete && /distilled|exported|stored|imported|complete|finished|sealed/i.test(status ?? '');
  $: maxVisiblePhase = !decisionComplete ? 0 : phase2Complete ? 3 : phase1Complete ? 2 : 1;
  $: {
    if (!open) {
      selectedPhase = 0;
      previousMaxVisiblePhase = 0;
    } else {
      if (maxVisiblePhase > previousMaxVisiblePhase && maxVisiblePhase === 3) {
        selectedPhase = maxVisiblePhase;
      }
      if (selectedPhase > maxVisiblePhase) {
        selectedPhase = maxVisiblePhase;
      }
      previousMaxVisiblePhase = maxVisiblePhase;
    }
  }

  $: completedPhaseCount = Number(decisionComplete) + Number(phase1Complete) + Number(phase2Complete) + Number(phase3Complete);
  $: completionPct = Math.round((completedPhaseCount / 4) * 100);
  $: activePhase = !decisionComplete ? 0 : !phase1Complete ? 1 : !phase2Complete ? 2 : 3;

  $: modeNarrative =
    mode === 'export'
      ? 'Liquefy the chosen memory cluster into a portable packet.'
      : mode === 'import'
        ? 'Import one externally prepared STTP node directly into memory.'
      : mode === 'distill'
        ? 'Distill one super node and anchor it into STTP.'
        : 'Liquefy and distill in one aligned extraction pass.';

  $: runLabel =
    loading
      ? 'transmuting...'
      : mode === 'export'
        ? 'extract packet'
        : mode === 'import'
          ? 'import node'
        : mode === 'distill'
          ? 'distill super node'
          : 'extract + distill';

  $: stageIntentLine = !decisionComplete
    ? 'Choose import or export route to unlock feedstock prep.'
    : importMode
      ? 'Import route: prepare target session + payload in phase 01a.'
      : scopeFresh
        ? `${preflightNodeCount} nodes across ${preflightSessionCount} sessions stabilized.`
        : 'Export route: stabilize the selected scope to reveal phase 02.';

  $: providerLabel = String(modelProvider ?? 'unknown');
</script>

<div class="alkahest-shell" class:hidden={hidden || cameraEngaged}>
  <button class="alkahest-launcher" data-tour-target="alkahest" on:click={toggleOpen} aria-label="open alkahest lab">
    <AlkahestStone size={28} glow="soft" animated={false} />
  </button>
</div>

<div
  class="alkahest-stage"
  class:visible={cameraEngaged}
  class:closing={phase === 'exiting'}
  aria-hidden={!cameraEngaged}
>
  {#if open}
    <button class="alkahest-backdrop" on:pointerdown={handleClosePointerDown} aria-label="close alkahest lab"></button>
  {/if}

  <aside
    class="alkahest-lab"
    class:open={open || phase === 'exiting'}
    class:interactive={open}
    class:closing={phase === 'exiting'}
    aria-label="The Alkahest"
  >
    <section class="visual-pane">
      <div class="visual-stone">
        <AlkahestStone size={196} glow="strong" animated={false} />
      </div>
      <p class="visual-kicker">memory refinery</p>
      <h3 class="visual-title">The Alkahest Lab</h3>
      <p class="visual-copy">{STAGE_COPY}</p>
    </section>

    <section class="control-pane">
      <header class="control-head">
        <div>
          <p class="control-kicker">ritual control</p>
          <h4>Sequential extraction</h4>
        </div>
        <div class="control-actions">
          <button class="alkahest-cancel" on:click={handleCancel}>cancel</button>
          <button class="alkahest-close" on:click={closePanel} aria-label="close alkahest">x</button>
        </div>
      </header>

      <div class="tracker-dock">
        <AlkahestPhaseTracker
          activePhase={activePhase}
          selectedPhase={selectedPhase}
          completionPct={completionPct}
          phase0Complete={decisionComplete}
          phase1Complete={phase1Complete}
          phase2Complete={phase2Complete}
          phase3Complete={phase3Complete}
          phase1Unlocked={decisionComplete}
          phase2Unlocked={phase1Complete}
          phase3Unlocked={phase2Complete}
          on:select={handleTrackerSelect}
        />

        <div class="phase-popover" data-phase={selectedPhase}>
          {#key selectedPhase}
            <div class="card-motion" in:fly={{ y: 8, duration: 150, opacity: 0.14 }} out:fade={{ duration: 90 }}>
              {#if selectedPhase === 0}
                <AlkahestPhaseZeroCard
                  selectedRoute={decisionComplete ? decisionRoute : null}
                  {loading}
                  {scopeScanning}
                  on:select={handleDecisionSelect}
                />
              {:else if selectedPhase === 1}
                {#if importMode}
                  <AlkahestPhaseOneImportCard
                    bind:targetSessionId
                    bind:importNodeDraft
                    {loading}
                    {scopeScanning}
                    on:change={handleImportPrepChange}
                  />
                {:else}
                  <AlkahestPhaseOneCard
                    bind:scope
                    bind:sessionId
                    bind:sessionIds
                    bind:timelineDays
                    bind:resonanceDim
                    bind:psiMin
                    {sessions}
                    {timelineOptions}
                    {loading}
                    {scopeScanning}
                    {preflightNodeCount}
                    {preflightWindowLabel}
                    {preflightSessionCount}
                    {preflightClipped}
                    {preflightLastScannedAt}
                    scopeFresh={phase1Complete}
                    {scopeStale}
                    {stageIntentLine}
                    on:scopeChange={handleScopeChange}
                    on:scan={scanScope}
                  />
                {/if}
              {:else if selectedPhase === 2}
                <AlkahestPhaseTwoCard
                  bind:mode
                  bind:targetSessionId
                  decisionRoute={decisionRoute}
                  modeLocked={importMode}
                  importPayloadLength={importNodeDraft.trim().length}
                  {loading}
                  {scopeScanning}
                  {phase2Complete}
                  {modeNarrative}
                  on:change={handlePhase2Change}
                  on:seal={sealPhase2}
                />
              {:else}
                <AlkahestPhaseThreeCard
                  {loading}
                  {scopeScanning}
                  {runLabel}
                  {providerLabel}
                  {preflightWindowLabel}
                  {preflightNodeCount}
                  {preflightSessionCount}
                  {completionPct}
                  {superNodePreview}
                  on:run={runAlkahest}
                  on:copy={copySuperNode}
                />
              {/if}
            </div>
          {/key}
        </div>
      </div>

      {#if error}
        <p class="alkahest-error">{error}</p>
      {/if}

      {#if status}
        <p class="alkahest-success">{status}</p>
      {/if}
    </section>
  </aside>
</div>

<style>
  .alkahest-shell {
    position: absolute;
    left: max(14px, calc(var(--safe-left) + 8px));
    bottom: max(66px, calc(var(--safe-bottom) + 58px));
    z-index: 18;
    pointer-events: none;
    transition: opacity 0.2s ease, transform 0.2s ease;
  }

  .alkahest-shell.hidden {
    opacity: 0;
    transform: translateY(8px);
  }

  .alkahest-launcher {
    pointer-events: auto;
    width: 38px;
    height: 38px;
    border-radius: 999px;
    border: 0.5px solid rgba(160, 198, 239, 0.38);
    background: rgba(13, 20, 32, 0.9);
    display: grid;
    place-items: center;
    padding: 0;
    cursor: pointer;
    box-shadow: 0 0 14px rgba(86, 144, 213, 0.16);
    transition: border-color 0.16s ease, background 0.16s ease;
  }

  .alkahest-launcher:hover {
    border-color: rgba(182, 218, 255, 0.62);
    background: rgba(33, 50, 78, 0.9);
  }

  .alkahest-stage {
    position: fixed;
    inset: 0;
    z-index: 170;
    pointer-events: none;
    opacity: 0;
    overflow: hidden;
    transition: opacity 0.26s ease;
  }

  .alkahest-stage.visible {
    opacity: 1;
    pointer-events: auto;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-overflow-scrolling: touch;
  }

  .alkahest-backdrop {
    position: absolute;
    inset: 0;
    border: none;
    margin: 0;
    padding: 0;
    background:
      radial-gradient(circle at 35% 50%, rgba(44, 74, 112, 0.14), rgba(3, 9, 16, 0.84) 72%),
      linear-gradient(125deg, rgba(8, 12, 18, 0.44), rgba(4, 7, 12, 0.88));
    backdrop-filter: blur(2px);
    cursor: pointer;
  }

  .alkahest-lab {
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%) scale(0.985);
    width: min(760px, calc(100vw - 52px));
    max-height: min(620px, calc(100dvh - 52px));
    display: grid;
    grid-template-columns: minmax(180px, 0.64fr) minmax(280px, 360px);
    gap: clamp(18px, 5vw, 46px);
    border: none;
    background: transparent;
    box-shadow: none;
    padding: 0;
    overflow: hidden;
    opacity: 0;
    pointer-events: none;
    transition: transform 0.26s cubic-bezier(0.2, 0.9, 0.2, 1), opacity 0.22s ease;
  }

  .alkahest-lab.open {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1);
  }

  .alkahest-lab.interactive {
    pointer-events: auto;
  }

  .visual-pane {
    border: none;
    background: transparent;
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 6px;
    padding: 0;
    overflow: visible;
    position: relative;
  }

  .visual-pane::before {
    content: '';
    position: absolute;
    width: 250px;
    height: 250px;
    border-radius: 50%;
    background: radial-gradient(circle at 50% 42%, rgba(88, 142, 206, 0.2), rgba(8, 16, 28, 0) 70%);
    pointer-events: none;
    z-index: -1;
  }

  .visual-pane::after {
    content: '';
    position: absolute;
    width: 300px;
    height: 300px;
    border-radius: 50%;
    border: 0.5px solid rgba(164, 204, 247, 0.14);
    opacity: 0.3;
    pointer-events: none;
    z-index: -1;
  }

  .visual-stone {
    animation: stone-idle 24s ease-in-out infinite;
    will-change: transform;
  }

  .visual-kicker {
    margin: 0;
    font: 9px/1.2 'Departure Mono', monospace;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(175, 208, 242, 0.68);
  }

  .visual-title {
    margin: 0;
    font: italic 400 28px/1.1 'Fraunces', serif;
    color: rgba(237, 246, 255, 0.95);
    text-align: center;
  }

  .visual-copy {
    margin: 0;
    max-width: 230px;
    text-align: center;
    font: 9px/1.45 'Departure Mono', monospace;
    letter-spacing: 0.05em;
    color: rgba(200, 220, 243, 0.68);
  }

  .control-pane {
    border-radius: 11px;
    border: 0.5px solid rgba(171, 206, 241, 0.18);
    background: linear-gradient(170deg, rgba(11, 22, 37, 0.76), rgba(6, 12, 22, 0.92));
    padding: 10px;
    display: grid;
    align-content: start;
    gap: 8px;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }

  .control-head {
    display: flex;
    justify-content: space-between;
    align-items: start;
    gap: 10px;
  }

  .control-kicker {
    margin: 0;
    font: 8px/1.2 'Departure Mono', monospace;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(183, 216, 248, 0.68);
  }

  .control-head h4 {
    margin: 2px 0 0;
    font: italic 400 16px/1.15 'Fraunces', serif;
    color: rgba(236, 246, 255, 0.93);
  }

  .control-actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .alkahest-cancel {
    border-radius: 999px;
    border: 0.5px solid rgba(169, 206, 245, 0.4);
    background: rgba(16, 28, 46, 0.7);
    color: rgba(211, 231, 252, 0.88);
    font: 700 8px/1 'Departure Mono', monospace;
    letter-spacing: 0.11em;
    text-transform: uppercase;
    padding: 7px 9px;
    cursor: pointer;
  }

  .alkahest-cancel:hover {
    border-color: rgba(196, 228, 255, 0.68);
    background: rgba(25, 41, 63, 0.78);
  }

  .alkahest-close {
    width: 26px;
    height: 26px;
    border-radius: 999px;
    border: 0.5px solid rgba(173, 210, 247, 0.36);
    background: rgba(15, 27, 44, 0.75);
    color: rgba(226, 239, 255, 0.9);
    font: 700 13px/1 'Departure Mono', monospace;
    cursor: pointer;
  }

  .tracker-dock {
    display: grid;
    gap: 10px;
  }

  .phase-popover {
    --phase-anchor: 50%;
    position: relative;
    margin-top: 2px;
    padding-top: 8px;
  }

  .phase-popover[data-phase='1'] {
    --phase-anchor: 16.6667%;
  }

  .phase-popover[data-phase='2'] {
    --phase-anchor: 50%;
  }

  .phase-popover[data-phase='3'] {
    --phase-anchor: 83.3333%;
  }

  .phase-popover::before {
    content: '';
    position: absolute;
    top: 0;
    left: var(--phase-anchor, 50%);
    width: 12px;
    height: 12px;
    transform: translate(-50%, -50%) rotate(45deg);
    background: rgba(10, 18, 30, 0.96);
    border-left: 0.5px solid rgba(154, 196, 239, 0.2);
    border-top: 0.5px solid rgba(154, 196, 239, 0.2);
  }

  .card-motion {
    width: min(100%, 336px);
    max-width: 328px;
    transform-origin: var(--phase-anchor, 50%) top;
  }

  .phase-popover[data-phase='1'] .card-motion {
    margin: 0 auto 0 0;
  }

  .phase-popover[data-phase='2'] .card-motion {
    margin: 0 auto;
  }

  .phase-popover[data-phase='3'] .card-motion {
    margin: 0 0 0 auto;
  }

  .alkahest-error,
  .alkahest-success {
    margin: 0;
    padding: 8px 9px;
    border-radius: 9px;
    font: 9px/1.45 'Departure Mono', monospace;
  }

  .alkahest-error {
    border: 0.5px solid rgba(239, 146, 146, 0.42);
    background: rgba(88, 27, 34, 0.5);
    color: rgba(255, 214, 214, 0.92);
  }

  .alkahest-success {
    border: 0.5px solid rgba(149, 221, 186, 0.38);
    background: rgba(26, 71, 58, 0.42);
    color: rgba(217, 248, 233, 0.95);
  }

  @media (max-width: 980px) {
    .alkahest-lab {
      position: relative;
      top: 10px;
      left: 50%;
      grid-template-columns: 1fr;
      grid-template-rows: auto minmax(0, 1fr);
      justify-items: center;
      transform: translateX(-50%);
      max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 20px);
      max-height: calc(100svh - var(--safe-top) - var(--safe-bottom) - 20px);
      width: min(400px, calc(100vw - 14px));
      gap: 10px;
      padding-bottom: max(8px, calc(var(--safe-bottom) + 4px));
    }

    .alkahest-lab.open {
      transform: translateX(-50%);
    }

    .visual-pane {
      min-height: 154px;
    }

    .visual-title {
      font-size: 22px;
    }

    .control-pane {
      width: 100%;
      padding: 8px;
      gap: 6px;
      min-height: 0;
      overflow-y: auto;
      overscroll-behavior: contain;
      -webkit-overflow-scrolling: touch;
    }

    .card-motion {
      width: 100%;
      max-width: 100%;
      margin: 0;
    }
  }

  @media (max-width: 760px) {
    .alkahest-lab {
      top: max(12px, calc(var(--safe-top) + 8px));
      max-height: calc(100dvh - var(--safe-top) - var(--safe-bottom) - 24px);
      max-height: calc(100svh - var(--safe-top) - var(--safe-bottom) - 24px);
      width: min(380px, calc(100vw - 24px));
    }

    .control-pane {
      padding-top: 10px;
    }

    .alkahest-shell {
      left: max(4px, calc(var(--safe-left) + 0px));
      bottom: max(58px, calc(var(--safe-bottom) + 48px));
    }

    .alkahest-launcher {
      width: 34px;
      height: 34px;
      box-shadow: 0 0 8px rgba(86, 144, 213, 0.1);
    }

    .phase-popover {
      --phase-anchor: 50%;
    }

    .phase-popover[data-phase='1'] .card-motion,
    .phase-popover[data-phase='2'] .card-motion,
    .phase-popover[data-phase='3'] .card-motion {
      margin: 0;
    }
  }

  @keyframes stone-idle {
    0%,
    100% {
      transform: translate(0, 0) rotate(-0.2deg);
    }
    50% {
      transform: translate(-1px, -1px) rotate(0.15deg);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .visual-stone {
      animation: none;
    }
  }
</style>
