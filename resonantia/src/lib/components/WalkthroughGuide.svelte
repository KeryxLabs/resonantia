<script lang="ts">
  import { createEventDispatcher, onDestroy } from 'svelte';
  import type { WalkthroughMode, WalkthroughPhase } from '../walkthrough';

  type StepMeta = {
    title: string;
    summary: string;
    detail: string;
  };

  export let open = false;
  export let phase: WalkthroughPhase = 'intro';
  export let mode: WalkthroughMode = 'first-run';
  export let cueVisible = true;
  export let targetSelector: string | null = null;
  export let allowedSelectors: string[] = [];

  const phaseOrder: Array<Exclude<WalkthroughPhase, 'intro' | 'complete'>> = [
    'settings',
    'checkin',
    'telescope',
    'alkahest',
    'importare',
    'live',
  ];

  const phaseMeta: Record<Exclude<WalkthroughPhase, 'intro' | 'complete'>, StepMeta> = {
    settings: {
      title: 'set your foundations',
      summary: 'Pick how Resonantia thinks and syncs so the rest of your flow feels reliable.',
      detail: 'Open menu -> view settings menu -> press Next.',
    },
    checkin: {
      title: 'capture your current mode',
      summary: 'Log your current state so this session has context from the very beginning.',
      detail: 'Open menu -> view calibration modes -> press Next.',
    },
    telescope: {
      title: 'scan with telescope',
      summary: 'Use Telescope to browse sessions, inspect timelines, and spot patterns worth following.',
      detail: 'Open telescope -> look around your timeline for a moment -> press Next.',
    },
    alkahest: {
      title: 'open alkahest lab',
      summary: 'Enter Alkahest to distill memory into a clearer node or exportable artifact.',
      detail: 'Launch Alkahest -> view the chamber open -> press Next.',
    },
    importare: {
      title: 'set alkahest import mode',
      summary: 'Use Alkahest import mode to bring in external STTP material and store it directly.',
      detail: 'In Alkahest phase 02, choose import mode -> review the import fields -> press Next.',
    },
    live: {
      title: 'switch to create live',
      summary: 'Switch to Live to think, write, and evolve ideas in an active thread.',
      detail: 'Switch to create live -> confirm it opens -> press Finish.',
    },
  };

  const dispatch = createEventDispatcher<{
    start: void;
    next: void;
    dismiss: { permanently: boolean };
  }>();

  let targetRect: DOMRect | null = null;
  let targetRaf: number | null = null;
  let targetTracking = false;
  let gateAttached = false;
  let failSoftUnlocked = false;
  let unresolvedTargetSince: number | null = null;
  let acceptedPulse = false;
  let acceptedPulseTimer: ReturnType<typeof setTimeout> | null = null;
  const FAIL_SOFT_TIMEOUT_MS = 2400;

  $: guidedPhase = phase !== 'intro' && phase !== 'complete';
  $: firstRun = mode === 'first-run';
  $: stepProgressIndex = guidedPhase ? phaseOrder.indexOf(phase as Exclude<WalkthroughPhase, 'intro' | 'complete'>) : -1;
  $: stepMeta = guidedPhase ? phaseMeta[phase as Exclude<WalkthroughPhase, 'intro' | 'complete'>] : null;
  $: waitingForTarget = guidedPhase && !targetRect;

  $: spotlightStyle = targetRect
    ? `left:${Math.max(6, targetRect.left - 10)}px; top:${Math.max(6, targetRect.top - 8)}px; width:${Math.max(38, targetRect.width + 20)}px; height:${Math.max(34, targetRect.height + 16)}px;`
    : '';

  function resolveTargetElement() {
    if (typeof document === 'undefined') {
      return null;
    }

    const selectors = [
      ...(targetSelector ? [targetSelector] : []),
      ...allowedSelectors.filter(selector => selector !== targetSelector),
    ];

    for (const selector of selectors) {
      try {
        const matched = document.querySelector(selector) as HTMLElement | null;
        if (matched) {
          return matched;
        }
      } catch {
        continue;
      }
    }

    return null;
  }

  function updateTargetRect() {
    if (!open || !guidedPhase) {
      targetRect = null;
      return;
    }

    const target = resolveTargetElement();
    if (!target) {
      targetRect = null;
      return;
    }

    targetRect = target.getBoundingClientRect();
  }

  function updateFailSoftState() {
    if (!open || !guidedPhase) {
      failSoftUnlocked = false;
      unresolvedTargetSince = null;
      return;
    }

    if (targetRect) {
      failSoftUnlocked = false;
      unresolvedTargetSince = null;
      return;
    }

    const now = Date.now();
    if (unresolvedTargetSince === null) {
      unresolvedTargetSince = now;
      return;
    }

    if (now - unresolvedTargetSince >= FAIL_SOFT_TIMEOUT_MS) {
      failSoftUnlocked = true;
    }
  }

  function clearAcceptedPulseTimer() {
    if (acceptedPulseTimer !== null) {
      clearTimeout(acceptedPulseTimer);
      acceptedPulseTimer = null;
    }
  }

  function pulseAccepted() {
    acceptedPulse = true;
    clearAcceptedPulseTimer();
    acceptedPulseTimer = setTimeout(() => {
      acceptedPulse = false;
      acceptedPulseTimer = null;
    }, 220);
  }

  function startTargetTracking() {
    if (targetTracking || typeof window === 'undefined') {
      return;
    }

    targetTracking = true;

    const tick = () => {
      if (!targetTracking) {
        return;
      }

      updateTargetRect();
      updateFailSoftState();
      targetRaf = window.requestAnimationFrame(tick);
    };

    tick();
  }

  function stopTargetTracking() {
    targetTracking = false;
    failSoftUnlocked = false;
    unresolvedTargetSince = null;
    acceptedPulse = false;
    clearAcceptedPulseTimer();

    if (targetRaf !== null && typeof window !== 'undefined') {
      window.cancelAnimationFrame(targetRaf);
      targetRaf = null;
    }
  }

  function asElement(eventTarget: EventTarget | null) {
    if (eventTarget instanceof Element) {
      return eventTarget;
    }

    if (eventTarget instanceof Node) {
      return eventTarget.parentElement;
    }

    return null;
  }

  function isAllowedElement(target: Element | null) {
    if (!target) {
      return false;
    }

    if (target.closest('[data-tour-skip]') || target.closest('[data-tour-allow]')) {
      return true;
    }

    const selectors = allowedSelectors.length > 0
      ? allowedSelectors
      : (targetSelector ? [targetSelector] : []);

    if (!selectors.length) {
      return false;
    }

    for (const selector of selectors) {
      try {
        if (target.closest(selector)) {
          return true;
        }
      } catch {
        continue;
      }
    }

    return false;
  }

  function isAllowedInteraction(event: Event) {
    if (isAllowedElement(asElement(event.target))) {
      return true;
    }

    if (typeof event.composedPath !== 'function') {
      return false;
    }

    const path = event.composedPath();
    for (const node of path) {
      if (isAllowedElement(asElement(node as EventTarget))) {
        return true;
      }
    }

    return false;
  }

  function gateInteraction(event: Event) {
    if (!open || !guidedPhase) {
      return;
    }

    if (failSoftUnlocked) {
      return;
    }

    if (isAllowedInteraction(event)) {
      pulseAccepted();
      return;
    }

    event.preventDefault();
    event.stopPropagation();
    if ('stopImmediatePropagation' in event) {
      event.stopImmediatePropagation();
    }
  }

  function attachGate() {
    if (gateAttached || typeof window === 'undefined') {
      return;
    }

    window.addEventListener('pointerdown', gateInteraction, true);
    window.addEventListener('click', gateInteraction, true);
    gateAttached = true;
  }

  function detachGate() {
    if (!gateAttached || typeof window === 'undefined') {
      return;
    }

    window.removeEventListener('pointerdown', gateInteraction, true);
    window.removeEventListener('click', gateInteraction, true);
    gateAttached = false;
  }

  function beginMission() {
    dispatch('start');
  }

  function nextMissionStep() {
    dispatch('next');
  }

  function dismissWalkthrough(permanently = true) {
    dispatch('dismiss', { permanently });
  }

  $: if (open && guidedPhase) {
    startTargetTracking();
    attachGate();
  } else {
    stopTargetTracking();
    detachGate();
    targetRect = null;
  }

  onDestroy(() => {
    stopTargetTracking();
    detachGate();
    clearAcceptedPulseTimer();
  });
</script>

{#if open}
  <div class="cinematic-layer" role="dialog" aria-label="guided walkthrough">
    <div class="cinematic-scrim" aria-hidden="true"></div>

    {#if guidedPhase && targetRect}
      <div class="cinematic-spotlight" class:accepted={acceptedPulse} style={spotlightStyle} aria-hidden="true"></div>
    {/if}

    {#if phase === 'intro'}
      <section class="cinematic-center-card" data-tour-allow>
        <p class="cinematic-kicker">{firstRun ? 'guided first run' : 'demo walkthrough'}</p>
        <h2>you are in the terrain</h2>
        <p>
          Follow the glow. We will guide one action at a time and keep the rest out of your way.
        </p>
        <div class="cinematic-actions">
          <button class="tour-btn subtle" type="button" on:click={() => dismissWalkthrough(true)} data-tour-skip>
            skip
          </button>
          <button class="tour-btn launch" type="button" on:click={beginMission}>begin</button>
        </div>
      </section>
    {:else if phase === 'complete'}
      <section class="cinematic-center-card" data-tour-allow>
        <p class="cinematic-kicker">walkthrough complete</p>
        <h2>guided pass finished</h2>
        <p>Continue exploring naturally, you got more features to discover, or run the demo again whenever you want to rehearse the path.</p>
        <div class="cinematic-actions">
          <button class="tour-btn launch" type="button" on:click={() => dismissWalkthrough(true)} data-tour-skip>
            close
          </button>
        </div>
      </section>
    {:else}
      <section class="cinematic-guide-card" class:visible={cueVisible} data-tour-allow>
        <p class="coach-kicker">{firstRun ? 'guided first run' : 'demo walkthrough'}</p>
        <div class="coach-progress" style={`grid-template-columns: repeat(${phaseOrder.length}, minmax(0, 1fr));`} aria-hidden="true">
          {#each phaseOrder as _, idx}
            <span class:active={idx === stepProgressIndex} class:done={idx < stepProgressIndex}></span>
          {/each}
        </div>
        <h2>{stepMeta?.title}</h2>
        <p class="coach-summary">{stepMeta?.summary}</p>
        <p class="coach-detail">{stepMeta?.detail}</p>
        {#if waitingForTarget}
          <p class="coach-wait">waiting for required control to appear...</p>
        {:else if failSoftUnlocked}
          <p class="coach-wait">control lock relaxed while target resolves...</p>
        {/if}
        <div class="coach-actions">
          <button class="tour-btn subtle" type="button" on:click={() => dismissWalkthrough(true)} data-tour-skip>
            skip walkthrough
          </button>
          <button class="tour-btn launch" type="button" on:click={nextMissionStep} data-tour-allow>
            {phase === 'live' ? 'finish' : 'next'}
          </button>
        </div>
      </section>
    {/if}
  </div>
{/if}

<style>
  .cinematic-layer {
    position: absolute;
    inset: 0;
    z-index: 220;
    pointer-events: none;
  }

  .cinematic-scrim {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background:
      radial-gradient(circle at 15% 18%, rgba(228, 168, 78, 0.13), transparent 34%),
      radial-gradient(circle at 82% 82%, rgba(103, 182, 255, 0.12), transparent 38%),
      linear-gradient(178deg, rgba(8, 10, 15, 0.22), rgba(6, 8, 13, 0.42));
  }

  .cinematic-spotlight {
    position: fixed;
    border-radius: 14px;
    border: 1px solid rgba(245, 223, 182, 0.76);
    box-shadow:
      0 0 0 2px rgba(204, 167, 112, 0.34),
      0 0 22px rgba(200, 162, 109, 0.5);
    background: rgba(245, 221, 180, 0.03);
    pointer-events: none;
    animation: spotlightPulse 1.9s ease-in-out infinite;
  }

  .cinematic-spotlight.accepted {
    box-shadow:
      0 0 0 2px rgba(194, 247, 216, 0.66),
      0 0 26px rgba(134, 218, 171, 0.72);
  }

  .cinematic-center-card,
  .cinematic-guide-card {
    position: fixed;
    pointer-events: auto;
    width: min(296px, calc(100vw - 24px));
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.2);
    background: linear-gradient(160deg, rgba(17, 22, 34, 0.93), rgba(9, 12, 21, 0.95));
    box-shadow: 0 18px 38px rgba(0, 0, 0, 0.46);
    padding: 12px 12px 11px;
    color: rgba(235, 242, 255, 0.9);
  }

  .cinematic-center-card {
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
  }

  .cinematic-guide-card {
    left: 50%;
    top: 50%;
    --guide-shift-y: -50%;
    transform: translate(-50%, var(--guide-shift-y)) translateY(8px);
    opacity: 0;
    transition:
      opacity 0.3s ease,
      transform 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      top 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      width 0.42s cubic-bezier(0.22, 1, 0.36, 1),
      padding 0.34s ease,
      border-radius 0.34s ease,
      box-shadow 0.34s ease;
  }

  .cinematic-guide-card.visible {
    opacity: 1;
    transform: translate(-50%, var(--guide-shift-y)) translateY(0);
  }

  :global(.walkthrough-compact) .cinematic-guide-card {
    top: 95%;
    --guide-shift-y: -100%;
    width: min(248px, calc(100vw - 14px));
    padding: 8px 9px;
    border-radius: 9px;
    box-shadow: 0 14px 24px rgba(0, 0, 0, 0.4);
    background: linear-gradient(160deg, rgba(16, 21, 33, 0.95), rgba(8, 11, 20, 0.97));
  }

  :global(.walkthrough-compact) .cinematic-guide-card.visible {
    transform: translate(-50%, var(--guide-shift-y)) translateY(0);
  }

  .cinematic-kicker,
  .coach-kicker {
    margin: 0 0 8px;
    font-size: 8px;
    letter-spacing: 0.19em;
    text-transform: uppercase;
    color: rgba(232, 201, 146, 0.84);
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  h2 {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 400;
    font-style: italic;
    font-size: 19px;
    line-height: 1.18;
    color: rgba(251, 244, 230, 0.96);
  }

  p {
    margin: 6px 0 0;
    font-size: 10px;
    line-height: 1.42;
    color: rgba(225, 234, 251, 0.8);
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  .coach-progress {
    display: grid;
    gap: 4px;
    margin-bottom: 8px;
  }

  .coach-progress span {
    height: 4px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
  }

  .coach-progress span.done {
    background: rgba(255, 208, 120, 0.44);
  }

  .coach-progress span.active {
    background: rgba(255, 224, 170, 0.86);
    box-shadow: 0 0 10px rgba(255, 200, 98, 0.58);
  }

  .coach-summary {
    margin-top: 5px;
  }

  .coach-detail {
    margin-top: 4px;
    color: rgba(183, 207, 247, 0.82);
  }

  :global(.walkthrough-compact) .cinematic-guide-card h2 {
    font-size: 16px;
    line-height: 1.16;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .coach-kicker {
    margin-bottom: 5px;
    font-size: 7px;
  }

  :global(.walkthrough-compact) .cinematic-guide-card p {
    font-size: 9px;
    line-height: 1.34;
    margin-top: 4px;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .coach-progress {
    margin-bottom: 6px;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .coach-summary {
    margin-top: 3px;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .coach-detail {
    display: none;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .coach-actions {
    margin-top: 7px;
    gap: 6px;
  }

  :global(.walkthrough-compact) .cinematic-guide-card .tour-btn {
    padding: 5px 9px;
    font-size: 8px;
  }

  .coach-wait {
    margin-top: 6px;
    color: rgba(255, 183, 104, 0.9);
    text-transform: lowercase;
  }

  .cinematic-actions,
  .coach-actions {
    margin-top: 9px;
    display: flex;
    justify-content: flex-end;
    pointer-events: auto;
  }

  .tour-btn {
    border: 1px solid rgba(255, 239, 211, 0.3);
    background: rgba(248, 223, 176, 0.1);
    color: rgba(255, 245, 224, 0.92);
    border-radius: 7px;
    padding: 6px 11px;
    font-size: 9px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    cursor: pointer;
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  .tour-btn.subtle {
    border-color: rgba(186, 201, 235, 0.3);
    background: rgba(105, 123, 160, 0.14);
    color: rgba(208, 221, 252, 0.84);
  }

  .tour-btn.launch {
    margin-left: 8px;
  }

  @keyframes spotlightPulse {
    0%, 100% {
      box-shadow:
        0 0 0 2px rgba(204, 167, 112, 0.34),
        0 0 18px rgba(200, 162, 109, 0.44);
    }

    50% {
      box-shadow:
        0 0 0 2px rgba(224, 192, 136, 0.5),
        0 0 28px rgba(224, 188, 132, 0.62);
    }
  }

  @media (max-width: 760px) {
    .cinematic-center-card,
    .cinematic-guide-card {
      width: min(270px, calc(100vw - 18px));
      padding: 10px;
    }

    :global(.walkthrough-compact) .cinematic-guide-card {
      width: min(232px, calc(100vw - 12px));
      padding: 7px 8px;
      top: 96%;
    }

    h2 {
      font-size: 17px;
    }

    .cinematic-actions,
    .coach-actions {
      justify-content: stretch;
    }

    .tour-btn,
    .tour-btn.subtle,
    .tour-btn.launch {
      width: 100%;
      margin: 0;
    }

    .cinematic-actions {
      display: grid;
      grid-template-columns: 1fr;
      gap: 8px;
    }
  }
</style>
