<script lang="ts">
  import type { SyncNowResponse } from '@resonantia/core';

  export let visualState: 'idle' | 'syncing' | 'success' | 'error' = 'idle';
  export let pullLoading = false;
  export let buttonAria = 'sync with cloud';
  export let detailVisible = false;
  export let detailTitle = 'standing by';
  export let detailTimeLabel = 'never';
  export let detailSubtitle = '';
  export let pullResult: SyncNowResponse | null = null;
  export let pullError: string | null = null;
  export let accountTier: string | null = null;
  export let openSyncDetailHover: () => void = () => {};
  export let closeSyncDetailHover: () => void = () => {};
  export let runSyncPull: () => void | Promise<void> = () => {};
</script>

<div
  class="sync-cloud-wrap"
  role="group"
  aria-label="sync status"
  on:mouseenter={openSyncDetailHover}
  on:mouseleave={closeSyncDetailHover}
>
  <button
    class="sync-cloud-btn"
    class:syncing={visualState === 'syncing'}
    class:success={visualState === 'success'}
    class:error={visualState === 'error'}
    on:click={runSyncPull}
    on:focus={openSyncDetailHover}
    on:blur={closeSyncDetailHover}
    disabled={pullLoading}
    aria-label={buttonAria}
  >
    <svg class="sync-cloud-icon" viewBox="0 0 24 24" aria-hidden="true">
      <path class="sync-cloud-shape" d="M7 18h9a4 4 0 0 0 .5-7.97A6 6 0 0 0 6 9.5 4.5 4.5 0 0 0 7 18Z" />
      <g class="sync-cloud-cycle">
        <path d="M9.2 12.8a2.9 2.9 0 0 1 4.3-.7" />
        <path d="M13.6 12h-2v2" />
        <path d="M14.8 15.2a2.9 2.9 0 0 1-4.3.7" />
        <path d="M10.4 16h2v-2" />
      </g>
    </svg>
  </button>
  {#if detailVisible}
    <div
      class="sync-detail-popover"
      class:syncing={visualState === 'syncing'}
      class:success={visualState === 'success'}
      class:error={visualState === 'error'}
      role="status"
      aria-live="polite"
    >
      <div class="sync-detail-head">
        <i class="sync-detail-dot" aria-hidden="true"></i>
        <span class="sync-detail-title">{detailTitle}</span>
        <span class="sync-detail-time">{detailTimeLabel}</span>
      </div>

      {#if pullResult}
        <div class="sync-detail-row">
          <span>up</span>
          <span>↑{pullResult.upload.uploaded} · ~{pullResult.upload.skipped} · ={pullResult.upload.duplicate} · ×{pullResult.upload.rejected}</span>
        </div>
        <div class="sync-detail-row">
          <span>down</span>
          <span>↓{pullResult.download.fetched} · +{pullResult.download.created} · ~{pullResult.download.updated} · ={pullResult.download.duplicate}</span>
        </div>
        {#if detailSubtitle}
          <p class="sync-detail-note">{detailSubtitle}</p>
        {/if}
      {:else if pullError}
        <p class="sync-detail-error">{detailSubtitle}</p>
      {:else if visualState === 'syncing'}
        <p class="sync-detail-note">{detailSubtitle}</p>
      {/if}
      {#if accountTier}
        <div class="sync-tier-footer">
          <span class="sync-tier-badge" class:subscriber={accountTier === 'subscriber'}>{accountTier} plan</span>
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .sync-cloud-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .sync-cloud-btn {
    width: 24px;
    height: 19px;
    border-radius: 999px;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.02);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    cursor: pointer;
    transition: border-color 0.2s, background 0.2s, transform 0.2s, box-shadow 0.2s;
  }

  .sync-cloud-btn:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.038);
    transform: translateY(-0.5px);
  }

  .sync-cloud-btn:disabled {
    cursor: default;
  }

  .sync-cloud-btn.syncing {
    border-color: rgba(147, 196, 255, 0.34);
    background: rgba(94, 148, 216, 0.11);
    box-shadow: 0 0 10px rgba(90, 150, 221, 0.18);
  }

  .sync-cloud-btn.success {
    border-color: rgba(147, 230, 187, 0.3);
    background: rgba(82, 171, 125, 0.11);
  }

  .sync-cloud-btn.error {
    border-color: rgba(255, 128, 125, 0.34);
    background: rgba(190, 72, 69, 0.11);
  }

  .sync-cloud-icon {
    width: 12.5px;
    height: 12.5px;
  }

  .sync-cloud-icon path {
    fill: none;
    stroke: rgba(255, 255, 255, 0.62);
    stroke-width: 1.42;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: stroke 0.2s;
  }

  .sync-cloud-btn.syncing .sync-cloud-icon path {
    stroke: rgba(191, 222, 255, 0.96);
  }

  .sync-cloud-btn.success .sync-cloud-icon path {
    stroke: rgba(194, 245, 217, 0.98);
  }

  .sync-cloud-btn.error .sync-cloud-icon path {
    stroke: rgba(255, 210, 208, 0.97);
  }

  .sync-cloud-cycle {
    transform-origin: center;
    transform-box: fill-box;
  }

  .sync-cloud-btn.syncing .sync-cloud-cycle {
    animation: sync-cloud-spin 0.9s linear infinite;
  }

  .sync-detail-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: -2px;
    width: max-content;
    min-width: 146px;
    max-width: min(216px, 68vw);
    padding: 7px 8px;
    border-radius: 10px;
    border: 0.5px solid rgba(255, 255, 255, 0.11);
    background:
      radial-gradient(circle at 88% 4%, rgba(138, 174, 228, 0.13), transparent 40%),
      radial-gradient(circle at 8% 92%, rgba(112, 192, 153, 0.09), transparent 44%),
      rgba(10, 11, 14, 0.915);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: 28;
  }

  .sync-detail-popover::before {
    content: '';
    position: absolute;
    top: -4px;
    right: 12px;
    width: 8px;
    height: 8px;
    border-left: 0.5px solid rgba(255, 255, 255, 0.11);
    border-top: 0.5px solid rgba(255, 255, 255, 0.11);
    background: rgba(10, 11, 14, 0.915);
    transform: rotate(45deg);
  }

  .sync-detail-popover.syncing {
    border-color: rgba(147, 196, 255, 0.4);
  }

  .sync-detail-popover.success {
    border-color: rgba(147, 230, 187, 0.34);
  }

  .sync-detail-popover.error {
    border-color: rgba(255, 130, 126, 0.4);
  }

  .sync-detail-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .sync-detail-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: rgba(195, 205, 223, 0.7);
    box-shadow: 0 0 10px rgba(133, 163, 214, 0.35);
  }

  .sync-detail-popover.syncing .sync-detail-dot {
    background: rgba(186, 222, 255, 0.95);
    box-shadow: 0 0 12px rgba(117, 171, 237, 0.46);
  }

  .sync-detail-popover.success .sync-detail-dot {
    background: rgba(191, 245, 216, 0.96);
    box-shadow: 0 0 10px rgba(89, 176, 127, 0.42);
  }

  .sync-detail-popover.error .sync-detail-dot {
    background: rgba(255, 201, 198, 0.94);
    box-shadow: 0 0 10px rgba(202, 90, 82, 0.42);
  }

  .sync-detail-title {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.74);
  }

  .sync-detail-time {
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.42);
  }

  .sync-detail-row {
    display: grid;
    grid-template-columns: 34px 1fr;
    gap: 6px;
    margin-bottom: 3px;
  }

  .sync-detail-row span:first-child {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
  }

  .sync-detail-row span:last-child {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.71);
    text-align: right;
    white-space: nowrap;
  }

  .sync-detail-note,
  .sync-detail-error {
    margin: 6px 0 0;
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    line-height: 1.45;
  }

  .sync-detail-note {
    color: rgba(255, 255, 255, 0.54);
  }

  .sync-detail-error {
    color: rgba(255, 200, 197, 0.92);
  }

  .sync-tier-footer {
    margin-top: 6px;
    padding-top: 5px;
    border-top: 0.5px solid rgba(255, 255, 255, 0.07);
  }

  .sync-tier-badge {
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 2px 5px;
    border-radius: 3px;
    border: 0.5px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.42);
    background: rgba(255, 255, 255, 0.03);
  }

  .sync-tier-badge.subscriber {
    border-color: rgba(147, 230, 187, 0.28);
    color: rgba(191, 245, 216, 0.78);
    background: rgba(82, 171, 125, 0.09);
  }

  @keyframes sync-cloud-spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }
</style>
