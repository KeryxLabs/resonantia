<script lang="ts">
  import type { ComposeMode, CrossSessionRoutingPreference } from "./types";

  export let mode: ComposeMode = "live";
  export let loading = false;
  export let replyLoading = false;
  export let promptCopyLoading = false;
  export let promptCopied = false;
  export let pasteNodeOpen = false;
  export let pasteNodeLoading = false;
  export let contextPopupOpen = false;
  export let chatSettingsOpen = false;
  export let crossSessionRoutingPreference: CrossSessionRoutingPreference = "ask";
  export let compact = false;

  export let copyComposeEncodePrompt: () => Promise<void> | void = () => {};
  export let toggleComposePasteNode: () => void = () => {};
  export let toggleContextPopup: () => void = () => {};
  export let clearComposeConversation: () => void = () => {};
  export let toggleChatSettingsPopup: () => void = () => {};
  export let clearCrossSessionRoutingPreference: () => void = () => {};
  export let switchComposeToLive: () => void = () => {};

  function crossSessionRoutingLabel(preference: CrossSessionRoutingPreference) {
    if (preference === "active-tab") {
      return "active chat";
    }

    if (preference === "match-session") {
      return "node session";
    }

    return "ask each time";
  }
</script>

<div class="compose-utility-actions" class:compact>
  <button class="compose-link-btn compose-link-pill compose-link-pill-gold" on:click={copyComposeEncodePrompt} disabled={promptCopyLoading || loading || replyLoading}>
    {promptCopyLoading ? 'copying distill prompt…' : promptCopied ? 'distill prompt copied' : 'copy distill prompt'}
  </button>
  {#if mode === 'live'}
    <span class="compose-utility-divider">•</span>
    <button class="compose-link-btn compose-link-pill" on:click={toggleComposePasteNode} disabled={pasteNodeLoading || loading || replyLoading}>
      {pasteNodeOpen ? 'hide paste save' : 'paste node to save'}
    </button>
    <span class="compose-utility-divider">•</span>
    <button
      class="compose-link-btn compose-link-pill compose-link-pill-context"
      class:active={contextPopupOpen}
      on:click={toggleContextPopup}
      disabled={loading || replyLoading}
      aria-expanded={contextPopupOpen}
    >
      {contextPopupOpen ? 'hide session context' : 'session context'}
    </button>
    <span class="compose-utility-divider">•</span>
    <button class="compose-link-btn compose-link-pill" on:click={clearComposeConversation} disabled={loading || replyLoading}>clear thread</button>
    <span class="compose-utility-divider">•</span>
    <button
      class="compose-link-btn compose-link-pill compose-link-pill-settings"
      class:active={chatSettingsOpen}
      on:click={toggleChatSettingsPopup}
      disabled={loading || replyLoading}
      aria-expanded={chatSettingsOpen}
    >
      {chatSettingsOpen ? 'hide chat settings' : 'chat settings'}
    </button>
    <span class="compose-utility-divider">•</span>
    <span class="compose-routing-pref">routing: {crossSessionRoutingLabel(crossSessionRoutingPreference)}</span>
    {#if crossSessionRoutingPreference !== 'ask'}
      <span class="compose-utility-divider">•</span>
      <button
        class="compose-link-btn compose-link-pill compose-link-pill-routing"
        on:click={clearCrossSessionRoutingPreference}
        disabled={loading || replyLoading}
      >
        reset routing choice
      </button>
    {/if}
  {:else}
    <span class="compose-utility-divider">•</span>
    <button class="compose-link-btn compose-link-pill compose-link-pill-live" data-tour-target="compose-switch-live" on:click={switchComposeToLive} disabled={pasteNodeLoading}>switch to create live</button>
  {/if}
</div>

<style>
  .compose-utility-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: 8px 12px 4px;
  }

  .compose-utility-actions.compact {
    margin: 0;
    gap: 4px;
  }

  .compose-utility-divider {
    color: rgba(255, 255, 255, 0.18);
    font-size: 9px;
    line-height: 1;
    user-select: none;
  }

  .compose-routing-pref {
    font-size: 8px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    color: rgba(187, 212, 234, 0.64);
  }

  .compose-link-btn {
    border: 0.5px solid rgba(126, 173, 198, 0.24);
    background: rgba(80, 119, 143, 0.09);
    padding: 4px 8px;
    margin: 0;
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    border-radius: 999px;
    color: rgba(191, 223, 242, 0.72);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .compose-utility-actions.compact .compose-link-btn {
    padding: 4px 7px;
    font-size: 8px;
    letter-spacing: 0.04em;
  }

  .compose-utility-actions.compact .compose-utility-divider {
    display: none;
  }

  .compose-utility-actions.compact .compose-routing-pref {
    width: 100%;
    margin-top: 1px;
  }

  .compose-link-btn:hover:not(:disabled) {
    color: rgba(224, 240, 249, 0.88);
    border-color: rgba(141, 192, 223, 0.4);
    background: rgba(89, 136, 166, 0.15);
  }

  .compose-link-pill-gold {
    border-color: rgba(199, 182, 132, 0.34);
    background: rgba(196, 166, 104, 0.1);
    color: rgba(229, 214, 182, 0.82);
  }

  .compose-link-pill-gold:hover:not(:disabled) {
    color: rgba(247, 235, 210, 0.92);
    border-color: rgba(215, 191, 136, 0.45);
    background: rgba(196, 166, 104, 0.16);
  }

  .compose-link-pill-live {
    border-color: rgba(153, 193, 121, 0.3);
    background: rgba(118, 163, 85, 0.1);
    color: rgba(212, 233, 189, 0.82);
  }

  .compose-link-pill-live:hover:not(:disabled) {
    border-color: rgba(180, 219, 148, 0.43);
    background: rgba(133, 178, 98, 0.17);
    color: rgba(230, 244, 214, 0.9);
  }

  .compose-link-pill-context {
    border-color: rgba(143, 184, 220, 0.3);
    background: rgba(75, 112, 146, 0.14);
    color: rgba(210, 231, 247, 0.82);
  }

  .compose-link-pill-context.active {
    border-color: rgba(184, 217, 245, 0.52);
    background: rgba(101, 151, 194, 0.24);
    color: rgba(234, 245, 255, 0.95);
  }

  .compose-link-pill-settings {
    border-color: rgba(152, 184, 216, 0.34);
    background: rgba(83, 117, 148, 0.18);
    color: rgba(210, 229, 245, 0.86);
  }

  .compose-link-pill-settings.active {
    border-color: rgba(186, 216, 241, 0.58);
    background: rgba(104, 150, 188, 0.26);
    color: rgba(237, 246, 255, 0.96);
  }

  .compose-link-pill-routing {
    border-color: rgba(201, 176, 126, 0.34);
    background: rgba(166, 133, 80, 0.12);
    color: rgba(236, 220, 190, 0.84);
  }

  .compose-link-pill-routing:hover:not(:disabled) {
    border-color: rgba(224, 198, 141, 0.48);
    background: rgba(188, 153, 93, 0.2);
    color: rgba(248, 237, 214, 0.92);
  }

  .compose-link-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  @media (max-width: 520px) {
    .compose-utility-actions {
      margin-left: 10px;
      margin-right: 10px;
      gap: 4px;
      row-gap: 4px;
    }

    .compose-link-btn {
      width: auto;
      text-align: center;
      padding: 4px 7px;
      font-size: 8px;
      letter-spacing: 0.04em;
    }

    .compose-utility-divider {
      display: none;
    }
  }
</style>
