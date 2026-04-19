<script lang="ts">
  import type { ComposeCalibrationAvec, ComposeProviderUsage, ComposeTokenUsage } from "./types";

  export let sessionId = "";
  export let tokenUsage: ComposeTokenUsage = {
    contextTokens: 0,
    draftTokens: 0,
    projectedTurnTokens: 0,
    contextWindowTokens: 1,
    usagePercent: 0,
    thresholdPercent: 72,
    thresholdTokens: 1,
    remainingTokens: 0,
  };

  export let providerUsage: ComposeProviderUsage = {
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
    responseCount: 0,
    provider: "",
    model: "",
    hasUsageData: false,
  };

  export let calibrationAvec: ComposeCalibrationAvec = {
    stability: 0.5,
    friction: 0.2,
    logic: 0.8,
    autonomy: 0.9,
    psi: 2.4,
  };

  export let autoEncodeEnabled = false;
  export let autoEncodeThresholdPercent = 72;

  let rowEl: HTMLDivElement | null = null;
  let popoverOpen = false;

  function clampPercent(value: number) {
    return Math.max(0, Math.min(100, value));
  }

  function formatTokenCount(value: number) {
    return Math.max(0, Math.round(value)).toLocaleString();
  }

  function formatAvecValue(value: number) {
    return Math.max(0, Math.min(1, value)).toFixed(2);
  }

  function formatContextHint(usagePercent: number, thresholdPercent: number) {
    if (usagePercent >= 90) {
      return "context critical - encode now";
    }

    if (usagePercent >= thresholdPercent) {
      return `encode threshold reached - ${Math.round(thresholdPercent)}%`;
    }

    if (usagePercent > 35) {
      return `${Math.max(0, 100 - Math.round(usagePercent))}% remaining`;
    }

    return "context pacing nominal";
  }

  function orbFill(valuePercent: number) {
    return `${clampPercent(valuePercent)}%`;
  }

  function togglePopover() {
    popoverOpen = !popoverOpen;
  }

  function handleWindowPointerDown(event: PointerEvent) {
    if (!rowEl) {
      return;
    }

    if (rowEl.contains(event.target as Node)) {
      return;
    }

    popoverOpen = false;
  }

  $: usagePercent = clampPercent(tokenUsage.usagePercent);
  $: thresholdPercent = clampPercent(autoEncodeThresholdPercent);
  $: providerSpentKnown = providerUsage.hasUsageData && providerUsage.totalTokens > 0;
  $: providerModelLabel = [providerUsage.provider, providerUsage.model]
    .map((value) => value.trim())
    .filter(Boolean)
    .join(" · ");
  $: contextHint = formatContextHint(usagePercent, thresholdPercent);
  $: contextReadout = `context ${Math.round(usagePercent)}% · threshold ${Math.round(thresholdPercent)}% · ${formatTokenCount(tokenUsage.remainingTokens)} left`;
  $: contextCritical = usagePercent >= 90;
  $: contextWarm = usagePercent >= thresholdPercent && usagePercent < 90;
</script>

<svelte:window on:pointerdown={handleWindowPointerDown} />

<div class="compose-status-bar">
  <div class="compose-status-copy">
    <p class="compose-encode-hint visible" class:warm={contextWarm} class:critical={contextCritical}>{contextHint}</p>
    <p class="compose-context-readout">{contextReadout}</p>
  </div>

  <div class="compose-avec-row" bind:this={rowEl}>
    <button
      class="compose-avec-orb-btn"
      type="button"
      aria-expanded={popoverOpen}
      aria-label="toggle session avec metrics"
      on:click={togglePopover}
    >
      <span class="compose-avec-orb context" class:warm={contextWarm} class:critical={contextCritical}>
        <span class="compose-avec-orb-fill" style={`height:${orbFill(usagePercent)};`}></span>
        <span class="compose-avec-dot"></span>
      </span>
      <span class="compose-avec-orb stability">
        <span class="compose-avec-orb-fill" style={`height:${orbFill(calibrationAvec.stability * 100)};`}></span>
        <span class="compose-avec-dot"></span>
      </span>
      <span class="compose-avec-orb friction">
        <span class="compose-avec-orb-fill" style={`height:${orbFill(calibrationAvec.friction * 100)};`}></span>
        <span class="compose-avec-dot"></span>
      </span>
      <span class="compose-avec-orb logic">
        <span class="compose-avec-orb-fill" style={`height:${orbFill(calibrationAvec.logic * 100)};`}></span>
        <span class="compose-avec-dot"></span>
      </span>
      <span class="compose-avec-orb autonomy">
        <span class="compose-avec-orb-fill" style={`height:${orbFill(calibrationAvec.autonomy * 100)};`}></span>
        <span class="compose-avec-dot"></span>
      </span>
    </button>

    <div class="compose-avec-popover" class:visible={popoverOpen} role="dialog" aria-label="session avec metrics">
      <div class="compose-pop-title">session avec · live</div>

      <div class="compose-pop-row">
        <span class="compose-pop-swatch context"></span>
        <span class="compose-pop-label">context</span>
        <span class="compose-pop-track"><span class="compose-pop-fill context" style={`width:${orbFill(usagePercent)};`}></span></span>
        <span class="compose-pop-val">{Math.round(usagePercent)}%</span>
      </div>

      <div class="compose-pop-row">
        <span class="compose-pop-swatch stability"></span>
        <span class="compose-pop-label">stability</span>
        <span class="compose-pop-track"><span class="compose-pop-fill stability" style={`width:${orbFill(calibrationAvec.stability * 100)};`}></span></span>
        <span class="compose-pop-val">{formatAvecValue(calibrationAvec.stability)}</span>
      </div>

      <div class="compose-pop-row">
        <span class="compose-pop-swatch friction"></span>
        <span class="compose-pop-label">friction</span>
        <span class="compose-pop-track"><span class="compose-pop-fill friction" style={`width:${orbFill(calibrationAvec.friction * 100)};`}></span></span>
        <span class="compose-pop-val">{formatAvecValue(calibrationAvec.friction)}</span>
      </div>

      <div class="compose-pop-row">
        <span class="compose-pop-swatch logic"></span>
        <span class="compose-pop-label">logic</span>
        <span class="compose-pop-track"><span class="compose-pop-fill logic" style={`width:${orbFill(calibrationAvec.logic * 100)};`}></span></span>
        <span class="compose-pop-val">{formatAvecValue(calibrationAvec.logic)}</span>
      </div>

      <div class="compose-pop-row">
        <span class="compose-pop-swatch autonomy"></span>
        <span class="compose-pop-label">autonomy</span>
        <span class="compose-pop-track"><span class="compose-pop-fill autonomy" style={`width:${orbFill(calibrationAvec.autonomy * 100)};`}></span></span>
        <span class="compose-pop-val">{formatAvecValue(calibrationAvec.autonomy)}</span>
      </div>

      <hr class="compose-pop-divider" />

      <div class="compose-pop-footer">
        Ψ {calibrationAvec.psi.toFixed(2)}
        {#if providerModelLabel}
          · {providerModelLabel}
        {/if}
      </div>
      <div class="compose-pop-footer muted">
        {#if providerSpentKnown}
          provider spent {formatTokenCount(providerUsage.totalTokens)} tokens
        {:else}
          provider usage pending
        {/if}
        {#if sessionId.trim()}
          · {sessionId.trim()}
        {/if}
        {#if autoEncodeEnabled}
          · auto {Math.round(thresholdPercent)}%
        {/if}
      </div>
    </div>
  </div>
  </div>

<style>
  .compose-status-bar {
    margin-top: 5px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 10px 10px;
    border-top: 0.5px solid rgba(255, 255, 255, 0.05);
  }

  .compose-status-copy {
    min-width: 0;
    display: grid;
    gap: 2px;
  }

  .compose-encode-hint {
    margin: 0;
    font-size: 9px;
    font-style: italic;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.14);
    transition: color 0.35s ease;
    text-transform: lowercase;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compose-encode-hint.visible {
    color: rgba(208, 222, 236, 0.7);
  }

  .compose-encode-hint.warm {
    color: rgba(201, 166, 94, 0.82);
  }

  .compose-encode-hint.critical {
    color: rgba(219, 117, 93, 0.86);
  }

  .compose-context-readout {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    color: rgba(173, 198, 220, 0.62);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compose-avec-row {
    position: relative;
  }

  .compose-avec-orb-btn {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    padding: 0;
    margin: 0;
    cursor: pointer;
  }

  .compose-avec-orb {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    position: relative;
    overflow: hidden;
    border: 0.5px solid rgba(255, 255, 255, 0.12);
    transition: transform 0.16s ease, border-color 0.2s ease;
  }

  .compose-avec-orb-btn:hover .compose-avec-orb {
    transform: scale(1.09);
  }

  .compose-avec-orb-fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 0;
    transition: height 0.45s ease, background 0.25s ease;
  }

  .compose-avec-orb.context {
    border-color: rgba(100, 190, 170, 0.28);
  }

  .compose-avec-orb.context .compose-avec-orb-fill {
    background: rgba(100, 190, 170, 0.58);
  }

  .compose-avec-orb.context.warm {
    border-color: rgba(204, 160, 88, 0.4);
  }

  .compose-avec-orb.context.warm .compose-avec-orb-fill {
    background: rgba(201, 155, 70, 0.66);
  }

  .compose-avec-orb.context.critical {
    border-color: rgba(220, 112, 86, 0.48);
  }

  .compose-avec-orb.context.critical .compose-avec-orb-fill {
    background: rgba(210, 90, 70, 0.68);
  }

  .compose-avec-orb.stability {
    border-color: rgba(100, 160, 220, 0.24);
  }

  .compose-avec-orb.stability .compose-avec-orb-fill {
    background: rgba(100, 160, 220, 0.58);
  }

  .compose-avec-orb.friction {
    border-color: rgba(200, 130, 80, 0.24);
  }

  .compose-avec-orb.friction .compose-avec-orb-fill {
    background: rgba(200, 130, 80, 0.58);
  }

  .compose-avec-orb.logic {
    border-color: rgba(160, 120, 220, 0.24);
  }

  .compose-avec-orb.logic .compose-avec-orb-fill {
    background: rgba(160, 120, 220, 0.58);
  }

  .compose-avec-orb.autonomy {
    border-color: rgba(100, 190, 120, 0.24);
  }

  .compose-avec-orb.autonomy .compose-avec-orb-fill {
    background: rgba(100, 190, 120, 0.58);
  }

  .compose-avec-dot {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
    background: rgba(255, 255, 255, 0.52);
  }

  .compose-avec-popover {
    display: none;
    position: absolute;
    right: 0;
    bottom: 26px;
    width: 228px;
    border-radius: 10px;
    border: 0.5px solid rgba(100, 190, 170, 0.18);
    background: rgba(12, 18, 28, 0.97);
    padding: 12px;
    box-shadow: 0 12px 30px rgba(2, 6, 12, 0.46);
    z-index: 6;
  }

  .compose-avec-popover.visible {
    display: block;
  }

  .compose-pop-title {
    margin: 0 0 12px;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.28);
  }

  .compose-pop-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 9px;
  }

  .compose-pop-swatch {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .compose-pop-swatch.context {
    background: rgba(100, 190, 170, 0.82);
  }

  .compose-pop-swatch.stability {
    background: rgba(100, 160, 220, 0.82);
  }

  .compose-pop-swatch.friction {
    background: rgba(200, 130, 80, 0.82);
  }

  .compose-pop-swatch.logic {
    background: rgba(160, 120, 220, 0.82);
  }

  .compose-pop-swatch.autonomy {
    background: rgba(100, 190, 120, 0.82);
  }

  .compose-pop-label {
    flex: 1;
    font-size: 10px;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.38);
    text-transform: lowercase;
  }

  .compose-pop-track {
    width: 70px;
    height: 2px;
    border-radius: 2px;
    background: rgba(255, 255, 255, 0.08);
    overflow: hidden;
  }

  .compose-pop-fill {
    display: block;
    height: 100%;
    border-radius: 2px;
    transition: width 0.35s ease;
  }

  .compose-pop-fill.context {
    background: rgba(100, 190, 170, 0.72);
  }

  .compose-pop-fill.stability {
    background: rgba(100, 160, 220, 0.72);
  }

  .compose-pop-fill.friction {
    background: rgba(200, 130, 80, 0.72);
  }

  .compose-pop-fill.logic {
    background: rgba(160, 120, 220, 0.72);
  }

  .compose-pop-fill.autonomy {
    background: rgba(100, 190, 120, 0.72);
  }

  .compose-pop-val {
    min-width: 30px;
    text-align: right;
    font-size: 10px;
    letter-spacing: 0.02em;
    color: rgba(255, 255, 255, 0.42);
  }

  .compose-pop-divider {
    border: none;
    border-top: 0.5px solid rgba(255, 255, 255, 0.06);
    margin: 10px 0;
  }

  .compose-pop-footer {
    margin: 0;
    font-size: 9px;
    text-align: center;
    color: rgba(255, 255, 255, 0.3);
    line-height: 1.5;
    text-transform: lowercase;
  }

  .compose-pop-footer.muted {
    color: rgba(255, 255, 255, 0.22);
  }

  @media (max-width: 520px) {
    .compose-status-bar {
      padding: 7px 0 10px;
    }

    .compose-avec-popover {
      width: min(228px, calc(100vw - 52px));
      right: -8px;
    }
  }
</style>
