<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let open = false;
  export let loading = false;
  export let saving = false;
  export let error: string | null = null;
  export let saved = false;
  export let localModelOriginWarning: string | null = null;
  export let ollamaBaseUrl = '';
  export let ollamaModel = '';
  export let gatewayBaseUrl = '';
  export let gatewayAuthToken = '';
  export let cloudAuthAvailable = false;
  export let cloudAuthSignedIn = false;
  export let cloudAuthBusy = false;
  export let cloudAuthStatus = 'cloud account disconnected';
  export let cloudAuthError: string | null = null;
  export let advancedOpen = false;
  export let accountTier: string | null = null;
  export let accountMemberSince: string | null = null;
  let showTechnicalTokenField = false;

  function formatMemberSince(iso: string): string {
    try {
      return new Date(iso).toLocaleDateString([], { month: 'short', year: 'numeric' });
    } catch {
      return '';
    }
  }

  const dispatch = createEventDispatcher<{
    close: void;
    save: void;
    demo: void;
    connectCloud: void;
    refreshCloudToken: void;
    clearCloudToken: void;
  }>();
</script>

{#if open}
  <div class="drawer" role="dialog" aria-label="Settings">
    <div class="drawer-header">
      <span class="drawer-title">settings</span>
      <button class="close-btn" on:click={() => dispatch('close')}>✕</button>
    </div>

    <!-- ── Resonantia Account (normie primary) ── -->
    <div class="settings-section">
      <span class="settings-section-label">resonantia account</span>
      {#if !cloudAuthAvailable}
        <p class="settings-note">cloud sync is not available in this build.</p>
      {:else if cloudAuthSignedIn}
        <p class="settings-note settings-account-connected">{cloudAuthStatus}</p>
        {#if accountTier || accountMemberSince}
          <div class="account-meta">
            {#if accountTier}
              <span class="account-tier-badge" class:subscriber={accountTier === 'subscriber'}>{accountTier} plan</span>
            {/if}
            {#if accountMemberSince}
              <span class="account-since">since {formatMemberSince(accountMemberSince)}</span>
            {/if}
          </div>
        {/if}
        {#if cloudAuthError}
          <p class="drawer-error settings-inline-warning">{cloudAuthError}</p>
        {/if}
        <div class="cloud-auth-actions">
          <button
            class="drawer-btn cancel"
            type="button"
            on:click={() => dispatch('clearCloudToken')}
            disabled={loading || saving || cloudAuthBusy}
          >
            sign out
          </button>
          <button
            class="drawer-btn"
            type="button"
            on:click={() => dispatch('connectCloud')}
            disabled={loading || saving || cloudAuthBusy}
          >
            switch account
          </button>
        </div>
      {:else}
        <p class="settings-account-tagline">sync your conversations across devices and keep them backed up.</p>
        {#if cloudAuthError}
          <p class="drawer-error settings-inline-warning">{cloudAuthError}</p>
        {/if}
        <button
          class="drawer-btn submit settings-account-cta"
          type="button"
          on:click={() => dispatch('connectCloud')}
          disabled={loading || saving || cloudAuthBusy}
        >
          {cloudAuthBusy ? 'connecting…' : 'sign in to resonantia'}
        </button>
      {/if}
    </div>

    <!-- ── Advanced settings ── -->
    <button
      class="settings-advanced-toggle"
      on:click={() => (advancedOpen = !advancedOpen)}
      disabled={loading || saving}
    >
      {advancedOpen ? '▴ hide advanced settings' : '▾ advanced settings'}
    </button>

    {#if advancedOpen}
      <div class="settings-advanced-panel">
        <span class="settings-subsection-label">ai model</span>

        <label class="settings-field">
          <span class="settings-label">server address</span>
          <span class="settings-note">Only change this if your AI model runs somewhere other than the default local setup.</span>
          <input class="drawer-input" type="text" placeholder="http://127.0.0.1:11434" bind:value={ollamaBaseUrl} disabled={loading || saving} />
        </label>
        {#if localModelOriginWarning}
          <p class="drawer-error settings-inline-warning">{localModelOriginWarning}</p>
        {/if}

        <label class="settings-field">
          <span class="settings-label">model name</span>
          <span class="settings-note">Example: gemma3, llama3.2, mistral. Use the model available on your AI server.</span>
          <input class="drawer-input" type="text" placeholder="llama3.2" bind:value={ollamaModel} disabled={loading || saving} />
        </label>

        <span class="settings-subsection-label">bring your own cloud</span>
        <p class="settings-note settings-byoc-intro">Only set this if you self-host a Resonantia gateway. Leave blank to use the hosted service.</p>

        <label class="settings-field">
          <span class="settings-label">gateway url</span>
          <input
            class="drawer-input"
            type="text"
            placeholder="https://your-gateway.example.com"
            bind:value={gatewayBaseUrl}
            disabled={loading || saving}
          />
        </label>

        <button
          class="settings-advanced-toggle technical-toggle"
          type="button"
          on:click={() => (showTechnicalTokenField = !showTechnicalTokenField)}
          disabled={loading || saving}
        >
          {showTechnicalTokenField ? 'hide auth token' : 'manual auth token'}
        </button>

        {#if showTechnicalTokenField}
          <label class="settings-field">
            <span class="settings-label">auth token</span>
            <span class="settings-note">Set automatically after sign in. Only paste manually if instructed.</span>
            <input
              class="drawer-input"
              type="password"
              placeholder="paste session jwt"
              bind:value={gatewayAuthToken}
              disabled={loading || saving}
              autocomplete="off"
            />
          </label>
        {/if}
      </div>
    {/if}

    <button
      class="settings-advanced-toggle"
      on:click={() => dispatch('demo')}
      disabled={loading || saving}
    >
      run guided demo
    </button>

    {#if loading}<p class="drawer-success">loading config…</p>{/if}
    {#if error}<p class="drawer-error">{error}</p>{/if}
    {#if saved}<p class="drawer-success">settings saved</p>{/if}
    <div class="drawer-actions">
      <button class="drawer-btn cancel" on:click={() => dispatch('close')}>cancel</button>
      <button class="drawer-btn submit" on:click={() => dispatch('save')} disabled={loading || saving}>
        {saving ? 'saving…' : 'save'}
      </button>
    </div>
  </div>
{/if}

<style>
  .drawer {
    position: absolute;
    top: 64px;
    bottom: 84px;
    left: 50%;
    transform: translateX(-50%);
    width: min(456px, calc(100vw - 32px));
    max-height: calc(100dvh - 148px);
    overflow-y: auto;
    background: rgba(10, 11, 14, 0.97);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 20px;
    z-index: 20;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    font-family: 'Departure Mono', 'Courier New', monospace;
    overscroll-behavior: contain;
    scrollbar-width: thin;
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
  }

  .drawer-title {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 15px;
    color: rgba(255, 255, 255, 0.55);
  }

  .close-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.3);
    font-size: 14px;
    cursor: pointer;
    padding: 0;
    transition: color 0.2s;
  }

  .close-btn:hover {
    color: rgba(255, 255, 255, 0.7);
  }

  .drawer-input {
    width: 100%;
    box-sizing: border-box;
    background: rgba(255, 255, 255, 0.04);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 10px;
    color: rgba(255, 255, 255, 0.7);
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    margin-bottom: 10px;
    outline: none;
    transition: border-color 0.2s;
  }

  .drawer-input:focus {
    border-color: rgba(255, 255, 255, 0.25);
  }

  .settings-field {
    display: grid;
    gap: 4px;
    margin-bottom: 10px;
    align-items: start;
  }

  .settings-label {
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.56);
  }

  .settings-note {
    font-size: 9px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.36);
  }

  .settings-field .drawer-input {
    margin-bottom: 0;
  }

  .settings-advanced-toggle {
    margin: 0 0 10px;
    width: 100%;
    background: rgba(255, 255, 255, 0.02);
    border: 0.5px dashed rgba(255, 255, 255, 0.12);
    border-radius: 7px;
    color: rgba(255, 255, 255, 0.5);
    padding: 7px 9px;
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.2s, color 0.2s, background 0.2s;
  }

  .settings-advanced-toggle:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.26);
    color: rgba(255, 255, 255, 0.78);
    background: rgba(255, 255, 255, 0.04);
  }

  .settings-advanced-toggle:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .settings-advanced-panel {
    margin-bottom: 10px;
    padding: 11px;
    border-radius: 9px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
  }

  .settings-inline-warning {
    margin: -2px 0 9px;
    line-height: 1.45;
  }

  .drawer-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }

  .cloud-auth-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 8px;
    margin-bottom: 8px;
  }

  .settings-section {
    margin-bottom: 12px;
    padding: 12px;
    border-radius: 9px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
  }

  .settings-section-label {
    display: block;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
    margin-bottom: 8px;
  }

  .settings-account-tagline {
    font-size: 10px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.55);
    margin: 0 0 10px;
  }

  .settings-account-connected {
    color: rgba(122, 170, 122, 0.85);
    margin-top: 2px;
    margin-bottom: 4px;
  }

  .account-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .account-tier-badge {
    font-size: 8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 2px 6px;
    border-radius: 4px;
    border: 0.5px solid rgba(255, 255, 255, 0.14);
    color: rgba(255, 255, 255, 0.5);
    background: rgba(255, 255, 255, 0.04);
  }

  .account-tier-badge.subscriber {
    border-color: rgba(147, 230, 187, 0.3);
    color: rgba(191, 245, 216, 0.88);
    background: rgba(82, 171, 125, 0.1);
  }

  .account-since {
    font-size: 8px;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.35);
  }

  .settings-account-cta {
    width: 100%;
    padding: 9px 14px;
    font-size: 11px;
  }

  .settings-subsection-label {
    display: block;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.35);
    margin: 12px 0 8px;
  }

  .settings-subsection-label:first-child {
    margin-top: 0;
  }

  .settings-byoc-intro {
    margin: -4px 0 10px;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 6px 14px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .drawer-btn.cancel {
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.3);
  }

  .drawer-btn.submit {
    background: rgba(255, 255, 255, 0.06);
    border: 0.5px solid rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.8);
  }

  .drawer-btn.submit:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.35);
  }

  .drawer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .drawer-error {
    font-size: 10px;
    color: rgba(233, 148, 58, 0.8);
    margin: 6px 0 0;
  }

  .drawer-success {
    font-size: 10px;
    color: rgba(122, 170, 122, 0.9);
    margin: 6px 0 0;
  }

  @media (max-width: 520px) {
    .drawer {
      top: calc(env(safe-area-inset-top, 0px) + 56px);
      bottom: max(74px, calc(env(safe-area-inset-bottom, 0px) + 58px));
      width: calc(100vw - 20px);
      max-height: calc(100dvh - 130px);
      padding: 16px;
    }
  }
</style>
