<script lang="ts">
  import type { ComposeProviderUsage, ComposeTokenUsage } from "./types";

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

  function formatTokenCount(value: number) {
    return Math.max(0, Math.round(value)).toLocaleString();
  }

  function formatUsagePercent(value: number) {
    return `${Math.max(0, value).toFixed(1)}%`;
  }

  $: providerSpentKnown = providerUsage.hasUsageData && providerUsage.totalTokens > 0;
</script>

<div class="compose-budget-panel" aria-label="context usage budget">
  <div class="compose-budget-head">
    <span>context usage: {formatTokenCount(tokenUsage.projectedTurnTokens)} / {formatTokenCount(tokenUsage.contextWindowTokens)} tokens</span>
    <small>{formatUsagePercent(tokenUsage.usagePercent)}</small>
  </div>
  <div class="compose-budget-bar" role="presentation" aria-hidden="true">
    <span class="compose-budget-fill" style={`width:${Math.min(100, tokenUsage.usagePercent)}%`}></span>
    <span class="compose-budget-threshold" style={`left:${tokenUsage.thresholdPercent}%`}></span>
  </div>
  <div class="compose-budget-meta">
    <span>context {formatTokenCount(tokenUsage.contextTokens)} · draft {formatTokenCount(tokenUsage.draftTokens)} · turn {formatTokenCount(tokenUsage.projectedTurnTokens)}</span>
    <span>
      {#if providerSpentKnown}
        spent {formatTokenCount(providerUsage.totalTokens)} via provider
      {:else}
        spent tracking pending provider usage
      {/if}
    </span>
  </div>
</div>

<style>
  .compose-budget-panel {
    border: 0.5px solid rgba(130, 170, 204, 0.24);
    border-radius: 10px;
    background:
      linear-gradient(180deg, rgba(50, 77, 103, 0.26), rgba(28, 43, 60, 0.2));
    padding: 8px 9px;
    display: grid;
    gap: 6px;
    margin-bottom: 4px;
    box-shadow: inset 0 0 0 1px rgba(94, 132, 165, 0.15);
  }

  .compose-budget-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    font-size: 8px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: rgba(199, 223, 246, 0.8);
  }

  .compose-budget-head small {
    font-size: 8px;
    color: rgba(228, 239, 251, 0.88);
    letter-spacing: 0.04em;
  }

  .compose-budget-bar {
    position: relative;
    height: 8px;
    border-radius: 999px;
    overflow: hidden;
    border: 0.5px solid rgba(154, 188, 220, 0.26);
    background: rgba(29, 48, 69, 0.46);
  }

  .compose-budget-fill {
    position: absolute;
    inset: 0 auto 0 0;
    width: 0;
    background: linear-gradient(90deg, rgba(106, 174, 223, 0.76), rgba(216, 186, 128, 0.74));
    transition: width 0.16s ease;
  }

  .compose-budget-threshold {
    position: absolute;
    top: -1px;
    bottom: -1px;
    width: 2px;
    margin-left: -1px;
    background: rgba(245, 230, 198, 0.86);
    box-shadow: 0 0 6px rgba(245, 230, 198, 0.44);
    opacity: 0.9;
  }

  .compose-budget-meta {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 8px;
    letter-spacing: 0.04em;
    color: rgba(182, 209, 233, 0.72);
    text-transform: lowercase;
  }
</style>
