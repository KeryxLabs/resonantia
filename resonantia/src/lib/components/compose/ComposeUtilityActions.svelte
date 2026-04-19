<script lang="ts">
  import type { CrossSessionRoutingPreference } from "./types";

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
    gap: 3px;
  }

  .compose-utility-divider {
    color: rgba(255, 255, 255, 0.18);
    font-size: 9px;
    line-height: 1;
    user-select: none;
  }

  .compose-routing-pref {
    font-size: 8px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
    color: rgba(162, 190, 210, 0.62);
  }

  .compose-link-btn {
    border: 0.5px solid rgba(98, 136, 163, 0.25);
    background: rgba(15, 27, 40, 0.6);
    padding: 3px 7px;
    margin: 0;
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
    border-radius: 999px;
    color: rgba(177, 207, 227, 0.78);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .compose-utility-actions.compact .compose-link-btn {
    padding: 3px 6px;
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
    color: rgba(209, 230, 245, 0.9);
    border-color: rgba(129, 174, 203, 0.38);
    background: rgba(38, 67, 89, 0.58);
  }

  .compose-link-pill-gold {
    border-color: rgba(172, 151, 109, 0.3);
    background: rgba(54, 45, 28, 0.64);
    color: rgba(206, 194, 166, 0.8);
  }

  .compose-link-pill-gold:hover:not(:disabled) {
    color: rgba(228, 217, 191, 0.9);
    border-color: rgba(188, 167, 121, 0.44);
    background: rgba(70, 57, 34, 0.74);
  }

  .compose-link-pill-context {
    border-color: rgba(108, 149, 181, 0.3);
    background: rgba(28, 49, 68, 0.66);
    color: rgba(185, 212, 231, 0.82);
  }

  .compose-link-pill-context.active {
    border-color: rgba(149, 188, 215, 0.48);
    background: rgba(53, 90, 121, 0.74);
    color: rgba(212, 231, 244, 0.9);
  }

  .compose-link-pill-settings {
    border-color: rgba(108, 147, 175, 0.3);
    background: rgba(29, 49, 67, 0.66);
    color: rgba(185, 211, 230, 0.82);
  }

  .compose-link-pill-settings.active {
    border-color: rgba(150, 187, 212, 0.48);
    background: rgba(55, 91, 120, 0.74);
    color: rgba(214, 231, 243, 0.9);
  }

  .compose-link-pill-routing {
    border-color: rgba(167, 149, 111, 0.3);
    background: rgba(54, 43, 26, 0.64);
    color: rgba(205, 195, 171, 0.8);
  }

  .compose-link-pill-routing:hover:not(:disabled) {
    border-color: rgba(186, 166, 123, 0.44);
    background: rgba(71, 57, 34, 0.74);
    color: rgba(227, 216, 192, 0.88);
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
