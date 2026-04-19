<script lang="ts">
  export let autoEncodeEnabled = false;
  export let autoEncodeThresholdPercent = 72;
  export let loading = false;
  export let replyLoading = false;
  export let setAutoEncodeEnabled: (enabled: boolean) => void = () => {};
  export let setAutoEncodeThresholdPercent: (thresholdPercent: number) => void = () => {};
</script>

<div class="compose-chat-settings-panel" aria-label="chat settings">
  <div class="compose-chat-settings-head">
    <span>chat settings</span>
    <small>local only</small>
  </div>

  <label class="compose-chat-settings-row">
    <input
      type="checkbox"
      checked={autoEncodeEnabled}
      disabled={loading || replyLoading}
      on:change={(event) => {
        const checked = (event.currentTarget as HTMLInputElement).checked;
        setAutoEncodeEnabled(checked);
      }}
    />
    <span>auto compact thread</span>
  </label>

  <label class="compose-chat-settings-threshold" class:disabled={!autoEncodeEnabled}>
    <div class="compose-chat-settings-threshold-head">
      <span>auto compact threshold</span>
      <strong>{autoEncodeThresholdPercent}%</strong>
    </div>
    <input
      type="range"
      min="60"
      max="85"
      step="1"
      value={autoEncodeThresholdPercent}
      disabled={!autoEncodeEnabled || loading || replyLoading}
      on:input={(event) => {
        const value = Number((event.currentTarget as HTMLInputElement).value);
        setAutoEncodeThresholdPercent(value);
      }}
    />
  </label>

  <p class="compose-chat-settings-note">threshold is capped between 60% and 85% to preserve room for final encoding.</p>
</div>

<style>
  .compose-chat-settings-panel {
    margin-top: 2px;
    margin-bottom: 3px;
    border-radius: 10px;
    border: 0.5px solid rgba(112, 149, 176, 0.22);
    background: linear-gradient(180deg, rgba(23, 39, 56, 0.62), rgba(17, 29, 42, 0.58));
    padding: 8px 9px;
    display: grid;
    gap: 8px;
    animation: composeContextPopupIn 0.18s ease;
  }

  .compose-chat-settings-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(195, 216, 233, 0.82);
  }

  .compose-chat-settings-head small {
    color: rgba(171, 195, 214, 0.56);
  }

  .compose-chat-settings-row {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    font-size: 9px;
    letter-spacing: 0.04em;
    color: rgba(196, 216, 232, 0.84);
    text-transform: lowercase;
    cursor: pointer;
  }

  .compose-chat-settings-row input {
    width: 12px;
    height: 12px;
    margin: 0;
    accent-color: rgba(125, 169, 206, 0.95);
  }

  .compose-chat-settings-threshold {
    display: grid;
    gap: 6px;
  }

  .compose-chat-settings-threshold.disabled {
    opacity: 0.54;
  }

  .compose-chat-settings-threshold-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    font-size: 9px;
    letter-spacing: 0.04em;
    color: rgba(194, 215, 231, 0.82);
    text-transform: lowercase;
  }

  .compose-chat-settings-threshold-head strong {
    font-size: 10px;
    letter-spacing: 0.05em;
    color: rgba(221, 235, 245, 0.9);
  }

  .compose-chat-settings-threshold input[type='range'] {
    width: 100%;
    margin: 0;
    accent-color: rgba(122, 167, 204, 0.95);
  }

  .compose-chat-settings-note {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.03em;
    color: rgba(164, 186, 205, 0.62);
    text-transform: lowercase;
  }

  @keyframes composeContextPopupIn {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
