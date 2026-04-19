<script lang="ts">
  import ComposeHeaderSession from './compose/ComposeHeaderSession.svelte';
  import ComposeTabs from './compose/ComposeTabs.svelte';
  import ComposeThread from './compose/ComposeThread.svelte';
  import ComposeInputRow from './compose/ComposeInputRow.svelte';
  import ComposeFooterActions from './compose/ComposeFooterActions.svelte';
  import ComposeUtilityActions from './compose/ComposeUtilityActions.svelte';
  import ComposeChatSettingsPanel from './compose/ComposeChatSettingsPanel.svelte';
  import ComposeSessionNodesPopover from './compose/ComposeSessionNodesPopover.svelte';
  import ComposeBottomTracker from './compose/ComposeBottomTracker.svelte';
  import ComposePastePanel from './compose/ComposePastePanel.svelte';
  import type {
    ComposeCalibrationAvec,
    ComposeContextNode,
    ComposeContextSession,
    ComposeInjectedNode,
    ComposeMessage,
    ComposeMode,
    ComposeProviderUsage,
    ComposeResult,
    ComposeTabInfo,
    ComposeTokenUsage,
    CrossSessionRoutingPreference,
  } from './compose/types';

  export let open = false;
  export let mode: ComposeMode = 'live';
  export let sessionId = '';
  export let draft = '';
  export let messages: ComposeMessage[] = [];
  export let tabs: ComposeTabInfo[] = [];
  export let activeTabId = '';
  export let maxTabs = 3;
  export let contextSessions: ComposeContextSession[] = [];
  export let contextOriginSessionId = '';
  export let contextBrowseSessionId = '';
  export let contextNodes: ComposeContextNode[] = [];
  export let contextNodesLoading = false;
  export let contextNodesError: string | null = null;
  export let injectedNodes: ComposeInjectedNode[] = [];
  export let loading = false;
  export let replyLoading = false;
  export let encodePromptSent = false;
  export let error: string | null = null;
  export let result: ComposeResult = null;
  export let promptCopyLoading = false;
  export let promptCopied = false;
  export let promptCopyError: string | null = null;
  export let pasteNodeOpen = false;
  export let pasteNodeDraft = '';
  export let pasteNodeLoading = false;
  export let crossSessionRoutingPreference: CrossSessionRoutingPreference = 'ask';
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
    provider: '',
    model: '',
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

  export let onClose: () => void = () => {};
  export let onSessionInput: () => void = () => {};
  export let onDraftInput: () => void = () => {};
  export let sendComposeMessage: () => Promise<void> | void = () => {};
  export let copyComposeEncodePrompt: () => Promise<void> | void = () => {};
  export let toggleComposePasteNode: () => void = () => {};
  export let clearComposeConversation: () => void = () => {};
  export let switchComposeToLive: () => void = () => {};
  export let saveComposePastedNode: () => Promise<void> | void = () => {};
  export let submitCompose: () => Promise<void> | void = () => {};
  export let clearCrossSessionRoutingPreference: () => void = () => {};
  export let setAutoEncodeEnabled: (enabled: boolean) => void = () => {};
  export let setAutoEncodeThresholdPercent: (thresholdPercent: number) => void = () => {};
  export let selectComposeTab: (tabId: string) => void = () => {};
  export let createComposeTab: () => void = () => {};
  export let closeComposeTab: (tabId: string) => void = () => {};
  export let selectContextSession: (sessionId: string) => void = () => {};
  export let injectContextNode: (nodeKey: string) => void = () => {};
  export let removeInjectedNode: (nodeKey: string) => void = () => {};

  let pasteInputEl: HTMLTextAreaElement | null = null;
  let pastePreviewEl: HTMLDivElement | null = null;
  let composeThreadEl: HTMLDivElement | null = null;
  let pastePrettyView = false;
  let composeAutoScrollKey = '';
  let contextPopupOpen = false;
  let chatSettingsOpen = false;
  let liveToolsOpen = false;
  let sessionNodesPopoverOpen = false;
  let sessionNodesPopoverTop = 88;
  let previousPasteNodeOpen = false;
  let liveShellEl: HTMLDivElement | null = null;
  let sessionNodesPopoverEl: HTMLDivElement | null = null;

  const STTP_KEYWORDS = new Set([
    'manual',
    'scheduled',
    'threshold',
    'resonance',
    'seed',
    'raw',
    'daily',
    'weekly',
    'monthly',
    'quarterly',
    'yearly',
    'null',
  ]);
  const STTP_TOKEN_RE = /(⏣|⊕⟨|⦿⟨|◈⟨|⍉⟨|⟩|[{}]|[A-Za-z_][A-Za-z0-9_]*(?:\(\.[0-9]+\))?(?=\s*:)|\b\d+(?:\.\d+)?\b)/g;

  $: pastePreviewSource = pastePrettyView ? prettifySttpVisual(pasteNodeDraft) : pasteNodeDraft;
  $: pasteNodePreviewHtml = renderPasteNodePreview(pastePreviewSource);

  $: if (pasteNodeOpen || mode === 'importare') {
    queueMicrotask(syncPasteEditorScroll);
  }

  $: if (!open || mode !== 'live') {
    contextPopupOpen = false;
    chatSettingsOpen = false;
    liveToolsOpen = false;
    sessionNodesPopoverOpen = false;
  }

  $: {
    if (mode === 'live' && pasteNodeOpen && !previousPasteNodeOpen) {
      liveToolsOpen = true;
    }

    previousPasteNodeOpen = pasteNodeOpen;
  }

  $: {
    const nextAutoScrollKey = `${open ? '1' : '0'}|${mode}|${messages.length}|${replyLoading ? '1' : '0'}`;
    if (nextAutoScrollKey !== composeAutoScrollKey) {
      composeAutoScrollKey = nextAutoScrollKey;
      if (open && mode === 'live') {
        queueMicrotask(() => {
          if (composeThreadEl) {
            composeThreadEl.scrollTop = composeThreadEl.scrollHeight;
          }
        });
      }
    }
  }

  function syncPasteEditorScroll() {
    if (!pasteInputEl || !pastePreviewEl) {
      return;
    }

    pastePreviewEl.scrollTop = pasteInputEl.scrollTop;
    pastePreviewEl.scrollLeft = pasteInputEl.scrollLeft;
  }

  function togglePastePrettyView() {
    pastePrettyView = !pastePrettyView;
    queueMicrotask(syncPasteEditorScroll);
  }

  function positionSessionNodesPopover(triggerEl: HTMLElement) {
    if (!liveShellEl) {
      return;
    }

    const shellRect = liveShellEl.getBoundingClientRect();
    const triggerRect = triggerEl.getBoundingClientRect();
    const desiredTop = (triggerRect.top - shellRect.top) + (triggerRect.height * 0.5) - 36;
    const minTop = 76;
    const maxTop = Math.max(minTop, liveShellEl.clientHeight - 320);
    sessionNodesPopoverTop = Math.max(minTop, Math.min(maxTop, desiredTop));
  }

  function handleSessionChipClick(session: ComposeContextSession, event: MouseEvent) {
    if (!contextPopupOpen) {
      contextPopupOpen = true;
    }

    const target = event.currentTarget;
    const sameSession = contextBrowseSessionId === session.sessionId;
    selectContextSession(session.sessionId);

    if (sameSession && sessionNodesPopoverOpen) {
      sessionNodesPopoverOpen = false;
      return;
    }

    sessionNodesPopoverOpen = true;
    if (target instanceof HTMLElement) {
      positionSessionNodesPopover(target);
    }
  }

  function handleWindowPointerDown(event: PointerEvent) {
    if (!sessionNodesPopoverOpen) {
      return;
    }

    const target = event.target;
    if (!(target instanceof Node)) {
      return;
    }

    if (sessionNodesPopoverEl?.contains(target)) {
      return;
    }

    if (target instanceof Element && target.closest('.compose-session-chip')) {
      return;
    }

    sessionNodesPopoverOpen = false;
  }

  function toggleContextPopup() {
    contextPopupOpen = !contextPopupOpen;
    if (!contextPopupOpen) {
      sessionNodesPopoverOpen = false;
      return;
    }

    chatSettingsOpen = false;
    liveToolsOpen = false;
  }

  function toggleChatSettingsPopup() {
    if (!liveToolsOpen) {
      liveToolsOpen = true;
    }

    chatSettingsOpen = !chatSettingsOpen;
    if (chatSettingsOpen) {
      contextPopupOpen = false;
      sessionNodesPopoverOpen = false;
    }
  }

  function toggleLiveTools() {
    liveToolsOpen = !liveToolsOpen;
    if (liveToolsOpen) {
      contextPopupOpen = false;
      sessionNodesPopoverOpen = false;
    }
  }

  function prettifySttpVisual(raw: string): string {
    const source = raw.trim();
    if (!source) {
      return raw;
    }

    let result = '';
    let indent = 0;

    const appendIndent = () => {
      if (result.endsWith('\n')) {
        result += '  '.repeat(Math.max(indent, 0));
      }
    };

    for (let i = 0; i < source.length; i++) {
      const ch = source[i];

      if (ch === '{') {
        result += '{\n';
        indent += 1;
        appendIndent();
        continue;
      }

      if (ch === '}') {
        result = result.trimEnd();
        indent = Math.max(0, indent - 1);
        result += `\n${'  '.repeat(indent)}}`;
        if (source[i + 1] && source[i + 1] !== '\n' && source[i + 1] !== '}' && source[i + 1] !== ',') {
          result += '\n';
          appendIndent();
        }
        continue;
      }

      if (ch === ',') {
        result += ',\n';
        appendIndent();
        continue;
      }

      if (ch === '\n') {
        result = result.trimEnd();
        result += '\n';
        appendIndent();
        continue;
      }

      if (ch === ' ' && result.endsWith('\n')) {
        continue;
      }

      result += ch;
    }

    return result.replace(/\n{3,}/g, '\n\n').trim();
  }

  function escapeHtml(value: string): string {
    return value
      .replaceAll('&', '&amp;')
      .replaceAll('<', '&lt;')
      .replaceAll('>', '&gt;')
      .replaceAll('"', '&quot;')
      .replaceAll("'", '&#39;');
  }

  function highlightSttpLine(line: string): string {
    if (!line) {
      return '&nbsp;';
    }

    let result = '';
    let cursor = 0;

    for (const match of line.matchAll(STTP_TOKEN_RE)) {
      const token = match[0];
      const tokenIndex = match.index ?? 0;
      result += escapeHtml(line.slice(cursor, tokenIndex));
      cursor = tokenIndex + token.length;

      if (token === '⏣' || token === '⊕⟨' || token === '⦿⟨' || token === '◈⟨' || token === '⍉⟨' || token === '⟩') {
        result += `<span class="sttp-marker">${token}</span>`;
        continue;
      }

      if (token === '{' || token === '}') {
        result += `<span class="sttp-brace">${token}</span>`;
        continue;
      }

      if (/^\d/.test(token)) {
        result += `<span class="sttp-number">${token}</span>`;
        continue;
      }

      if (STTP_KEYWORDS.has(token)) {
        result += `<span class="sttp-keyword">${token}</span>`;
        continue;
      }

      const confidenceIndex = token.indexOf('(.');
      if (confidenceIndex > -1 && token.endsWith(')')) {
        const base = token.slice(0, confidenceIndex);
        const confidence = token.slice(confidenceIndex);
        result += `<span class="sttp-key">${base}</span><span class="sttp-confidence">${confidence}</span>`;
      } else {
        result += `<span class="sttp-key">${token}</span>`;
      }
    }

    if (cursor < line.length) {
      result += escapeHtml(line.slice(cursor));
    }

    return result;
  }

  function renderPasteNodePreview(draft: string): string {
    if (!draft.trim()) {
      return '<span class="sttp-empty">highlighted preview appears here</span>';
    }

    return draft
      .split('\n')
      .map((line) => highlightSttpLine(line))
      .join('\n');
  }

  function composeOutcomeLabel(status: 'created' | 'updated' | 'duplicate' | 'skipped', duplicateSkipped: boolean) {
    if (duplicateSkipped || status === 'duplicate' || status === 'skipped') {
      return 'already present · duplicate skipped';
    }
    if (status === 'updated') {
      return 'updated';
    }
    return 'stored';
  }
</script>

<svelte:window on:pointerdown={handleWindowPointerDown} />

{#if open}
  <div class="drawer drawer-compose" class:importare={mode === 'importare'} role="dialog" aria-label={mode === 'importare' ? 'Import node' : 'Live chat'}>
    {#if mode === 'live'}
      <div class="compose-live-shell" bind:this={liveShellEl}>
        <div class="compose-live-stars" aria-hidden="true"></div>

        <aside class="compose-session-rail" class:open={contextPopupOpen} aria-label="thread sessions">
          <p class="compose-rail-label">sessions</p>
          {#if contextSessions.length > 0}
            {#each contextSessions as session}
              <button
                class="compose-session-chip"
                class:active={contextBrowseSessionId === session.sessionId}
                class:origin={contextOriginSessionId === session.sessionId}
                on:click={(event) => handleSessionChipClick(session, event)}
              >
                {session.label}
              </button>
            {/each}
          {:else}
            <p class="compose-session-empty">no linked sessions yet</p>
          {/if}
        </aside>

        <div class="compose-chat-col">
          <div class="compose-chat-header">
            <button class="compose-rail-toggle" type="button" aria-expanded={contextPopupOpen} on:click={toggleContextPopup} title="session context">
              <span></span>
              <span></span>
              <span></span>
            </button>

            <ComposeTabs
              {tabs}
              {activeTabId}
              {maxTabs}
              {selectComposeTab}
              {createComposeTab}
              {closeComposeTab}
            />

            <button
              class="compose-tools-toggle"
              type="button"
              aria-expanded={liveToolsOpen}
              on:click={toggleLiveTools}
              title="chat tools"
            >
              {liveToolsOpen ? 'hide tools' : 'tools'}
            </button>

            <button class="compose-close-btn" type="button" on:click={onClose} aria-label="close chat">x</button>
          </div>

          <div class="compose-session-row">
            <span class="compose-session-tag">session</span>
            <input
              class="compose-session-input"
              type="text"
              placeholder="session id"
              bind:value={sessionId}
              on:input={onSessionInput}
            />
          </div>

          <ComposeThread {messages} {replyLoading} bind:threadEl={composeThreadEl} />

          <div class="compose-input-zone">
            <ComposeInputRow
              bind:draft
              {sessionId}
              {loading}
              {replyLoading}
              {onDraftInput}
              {sendComposeMessage}
            />

            <ComposeBottomTracker
              {mode}
              {sessionId}
              {tokenUsage}
              {providerUsage}
              {calibrationAvec}
              {autoEncodeEnabled}
              {autoEncodeThresholdPercent}
            />
          </div>

          {#if contextPopupOpen && sessionNodesPopoverOpen}
            <div class="compose-session-nodes-popover-wrap" style={`top: ${sessionNodesPopoverTop}px;`}>
              <ComposeSessionNodesPopover
                sessionId={contextBrowseSessionId}
                {contextNodes}
                {contextNodesLoading}
                {contextNodesError}
                {injectedNodes}
                {injectContextNode}
                {removeInjectedNode}
                bind:popoverEl={sessionNodesPopoverEl}
              />
            </div>
          {/if}

          {#if liveToolsOpen}
            <div class="compose-live-tools-wrap">
              <ComposeUtilityActions
                {mode}
                {loading}
                {replyLoading}
                {promptCopyLoading}
                {promptCopied}
                {pasteNodeOpen}
                {pasteNodeLoading}
                {contextPopupOpen}
                {chatSettingsOpen}
                {crossSessionRoutingPreference}
                {copyComposeEncodePrompt}
                {toggleComposePasteNode}
                {toggleContextPopup}
                {clearComposeConversation}
                {toggleChatSettingsPopup}
                {clearCrossSessionRoutingPreference}
                {switchComposeToLive}
                compact={true}
              />

              {#if chatSettingsOpen}
                <ComposeChatSettingsPanel
                  {autoEncodeEnabled}
                  {autoEncodeThresholdPercent}
                  {loading}
                  {replyLoading}
                  {setAutoEncodeEnabled}
                  {setAutoEncodeThresholdPercent}
                />
              {/if}

              {#if pasteNodeOpen}
                <ComposePastePanel
                  {mode}
                  {sessionId}
                  bind:pasteNodeDraft
                  {pasteNodeLoading}
                  {pastePrettyView}
                  {pasteNodePreviewHtml}
                  bind:pasteInputEl
                  bind:pastePreviewEl
                  {togglePastePrettyView}
                  {syncPasteEditorScroll}
                  {toggleComposePasteNode}
                  {saveComposePastedNode}
                />
              {/if}

              <div class="compose-live-tools-actions">
                <button
                  class="compose-live-encode-btn"
                  type="button"
                  on:click={submitCompose}
                  disabled={loading || replyLoading || messages.length === 0 || !sessionId.trim()}
                >
                  {loading ? 'encoding…' : 'encode + save + continue'}
                </button>
              </div>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <ComposeHeaderSession
        {mode}
        bind:sessionId
        {onSessionInput}
        {onClose}
      />

      <p class="compose-importare-note">paste one complete node and store it directly.</p>

      <ComposeUtilityActions
        {mode}
        {loading}
        {replyLoading}
        {promptCopyLoading}
        {promptCopied}
        {pasteNodeOpen}
        {pasteNodeLoading}
        {contextPopupOpen}
        {chatSettingsOpen}
        {crossSessionRoutingPreference}
        {copyComposeEncodePrompt}
        {toggleComposePasteNode}
        {toggleContextPopup}
        {clearComposeConversation}
        {toggleChatSettingsPopup}
        {clearCrossSessionRoutingPreference}
        {switchComposeToLive}
      />
    {/if}

    {#if promptCopyError}
      <p class="drawer-error">copy failed: {promptCopyError}</p>
    {/if}

    {#if mode === 'importare'}
      <ComposePastePanel
        {mode}
        {sessionId}
        bind:pasteNodeDraft
        {pasteNodeLoading}
        {pastePrettyView}
        {pasteNodePreviewHtml}
        bind:pasteInputEl
        bind:pastePreviewEl
        {togglePastePrettyView}
        {syncPasteEditorScroll}
        {toggleComposePasteNode}
        {saveComposePastedNode}
      />
    {/if}

    {#if mode === 'live' && loading && encodePromptSent}
      <p class="drawer-success compose-encode-note">encoding prompt sent</p>
    {/if}
    {#if error}<p class="drawer-error">{error}</p>{/if}
    {#if result}
      <p class="drawer-success">
        {composeOutcomeLabel(result.status, result.duplicateSkipped)} · Ψ {result.psi.toFixed(4)}
      </p>
    {/if}

    {#if mode === 'importare'}
      <ComposeFooterActions
        {mode}
        {loading}
        {replyLoading}
        messagesCount={messages.length}
        {sessionId}
        {onClose}
        {submitCompose}
      />
    {/if}
  </div>
{/if}

<style>
  .drawer-compose {
    --compose-paste-height: 184px;
  }

  .drawer {
    position: absolute;
    top: max(64px, calc(var(--safe-top) + 46px));
    bottom: auto;
    left: 50%;
    transform: translateX(-50%);
    box-sizing: border-box;
    width: min(880px, calc(100vw - 24px));
    height: min(74dvh, 720px);
    max-height: min(74dvh, 720px);
    overflow-y: auto;
    overflow-x: hidden;
    background: rgba(11, 15, 21, 0.97);
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    border-radius: 14px;
    padding: 0;
    z-index: 20;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    font-family: 'IBM Plex Sans', sans-serif;
    overscroll-behavior: contain;
    scrollbar-width: thin;
    box-shadow: 0 22px 54px rgba(2, 7, 14, 0.55);
  }

  .drawer-compose {
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow-y: hidden;
    overflow-x: hidden;
  }

  .drawer-compose:not(.importare) {
    top: max(56px, calc(var(--safe-top) + 18px));
    bottom: max(16px, calc(var(--safe-bottom) + 14px));
    left: 50%;
    right: auto;
    transform: translateX(-50%);
    width: min(1280px, calc(100vw - 28px));
    height: auto;
    max-height: none;
    border-radius: 16px;
    overflow: hidden;
  }

  .drawer-compose.importare {
    --compose-paste-height: 224px;
    width: min(520px, calc(100vw - 24px));
    height: auto;
    max-height: min(620px, calc(100dvh - 160px));
    padding: 14px;
    gap: 6px;
    overflow-y: auto;
  }

  .compose-live-shell {
    position: relative;
    min-height: 0;
    height: 100%;
    flex: 1;
    display: flex;
    overflow: hidden;
    border-radius: 14px;
    background:
      radial-gradient(ellipse at 50% 56%, rgba(12, 29, 43, 0.34) 0%, rgba(9, 14, 23, 0.95) 68%),
      #0b0f15;
    isolation: isolate;
  }

  .compose-live-shell::before {
    content: '';
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse at 50% 52%, rgba(100, 190, 170, 0.08) 0%, rgba(100, 190, 170, 0) 58%),
      radial-gradient(ellipse at 50% 96%, rgba(100, 160, 220, 0.06) 0%, rgba(100, 160, 220, 0) 64%);
    opacity: 0.54;
    pointer-events: none;
    z-index: 0;
  }

  .compose-live-shell::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(7, 11, 18, 0.32) 0%, rgba(7, 11, 18, 0) 20%, rgba(7, 11, 18, 0) 80%, rgba(7, 11, 18, 0.32) 100%),
      linear-gradient(180deg, rgba(7, 11, 18, 0.24) 0%, rgba(7, 11, 18, 0) 24%, rgba(7, 11, 18, 0) 72%, rgba(7, 11, 18, 0.36) 100%);
    pointer-events: none;
    z-index: 0;
  }

  .compose-live-stars {
    pointer-events: none;
    position: absolute;
    inset: 0;
    background:
      radial-gradient(circle at 16% 18%, rgba(100, 190, 170, 0.1) 0, rgba(100, 190, 170, 0) 32%),
      radial-gradient(circle at 80% 12%, rgba(100, 190, 170, 0.08) 0, rgba(100, 190, 170, 0) 26%),
      radial-gradient(circle at 88% 76%, rgba(100, 190, 170, 0.08) 0, rgba(100, 190, 170, 0) 34%),
      radial-gradient(circle at 12% 74%, rgba(100, 190, 170, 0.07) 0, rgba(100, 190, 170, 0) 28%);
    opacity: 0.42;
    animation: composeStarFieldFloat 28s ease-in-out infinite alternate;
    z-index: 0;
  }

  @keyframes composeStarFieldFloat {
    from {
      transform: scale(1) translateY(0);
      opacity: 0.38;
    }
    to {
      transform: scale(1.01) translateY(-2px);
      opacity: 0.46;
    }
  }

  .compose-session-rail {
    width: 0;
    padding: 0;
    overflow: hidden;
    transition: width 0.32s cubic-bezier(0.4, 0, 0.2, 1), padding 0.32s cubic-bezier(0.4, 0, 0.2, 1);
    border-right: 0.5px solid rgba(255, 255, 255, 0.05);
    background: rgba(10, 15, 22, 0.86);
    z-index: 2;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .compose-session-rail.open {
    width: 170px;
    padding: 16px 10px;
  }

  .compose-rail-label {
    margin: 0 0 4px;
    font-size: 9px;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.2);
    text-transform: uppercase;
  }

  .compose-session-chip {
    border: 0.5px solid rgba(100, 180, 165, 0.16);
    background: rgba(100, 180, 165, 0.05);
    color: rgba(180, 210, 200, 0.62);
    border-radius: 20px;
    font-size: 10px;
    letter-spacing: 0.02em;
    text-align: left;
    padding: 4px 10px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    cursor: pointer;
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease;
  }

  .compose-session-chip:hover {
    color: rgba(100, 190, 170, 0.9);
    border-color: rgba(100, 190, 170, 0.32);
    background: rgba(100, 190, 170, 0.1);
  }

  .compose-session-chip.active {
    color: rgba(100, 190, 170, 0.96);
    border-color: rgba(100, 190, 170, 0.36);
    background: rgba(100, 190, 170, 0.12);
  }

  .compose-session-chip.origin {
    border-color: rgba(194, 166, 102, 0.36);
  }

  .compose-session-empty {
    margin: 0;
    font-size: 10px;
    color: rgba(255, 255, 255, 0.28);
    font-style: italic;
    text-transform: lowercase;
  }

  .compose-chat-col {
    position: relative;
    z-index: 3;
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: visible;
  }

  .compose-chat-col > * {
    position: relative;
    z-index: 1;
  }

  .compose-chat-header {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 16px 10px;
    border-bottom: 0.5px solid rgba(255, 255, 255, 0.04);
    background: linear-gradient(180deg, rgba(10, 16, 25, 0.34), rgba(10, 16, 25, 0.06));
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
  }

  .compose-rail-toggle {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    border: 0.5px solid rgba(255, 255, 255, 0.12);
    background: transparent;
    display: inline-flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    padding: 0 5px;
    cursor: pointer;
    flex-shrink: 0;
    transition: border-color 0.2s ease, background 0.2s ease;
  }

  .compose-rail-toggle:hover {
    border-color: rgba(100, 190, 170, 0.34);
    background: rgba(100, 190, 170, 0.08);
  }

  .compose-rail-toggle span {
    display: block;
    height: 1px;
    border-radius: 1px;
    background: rgba(150, 200, 190, 0.72);
  }

  .compose-rail-toggle span:nth-child(2) {
    width: 72%;
  }

  .compose-rail-toggle span:nth-child(3) {
    width: 86%;
  }

  .compose-close-btn {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.3);
    font-size: 14px;
    cursor: pointer;
    line-height: 1;
    padding: 0;
    flex-shrink: 0;
  }

  .compose-close-btn:hover {
    color: rgba(100, 190, 170, 0.86);
  }

  .compose-tools-toggle {
    border: 0.5px solid rgba(122, 179, 208, 0.24);
    border-radius: 999px;
    background: rgba(76, 123, 156, 0.14);
    color: rgba(181, 214, 236, 0.72);
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    cursor: pointer;
    padding: 4px 8px;
    flex-shrink: 0;
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease;
  }

  .compose-tools-toggle:hover {
    border-color: rgba(155, 207, 238, 0.42);
    background: rgba(92, 145, 181, 0.22);
    color: rgba(221, 240, 255, 0.92);
  }

  .compose-session-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px 0;
    background: linear-gradient(180deg, rgba(10, 16, 25, 0.08), rgba(10, 16, 25, 0));
  }

  .compose-session-tag {
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.12em;
    color: rgba(255, 255, 255, 0.22);
    flex-shrink: 0;
  }

  .compose-session-input {
    width: 100%;
    min-width: 0;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.03);
    border-radius: 7px;
    color: rgba(206, 217, 228, 0.84);
    font-size: 11px;
    letter-spacing: 0.03em;
    padding: 6px 10px;
    outline: none;
    transition: border-color 0.2s ease;
  }

  .compose-session-input:focus {
    border-color: rgba(100, 190, 170, 0.32);
  }

  .compose-session-input::placeholder {
    color: rgba(255, 255, 255, 0.24);
  }

  .compose-input-zone {
    border-top: 0.5px solid rgba(255, 255, 255, 0.05);
    padding: 10px 14px 0;
    position: relative;
    z-index: 2;
    flex-shrink: 0;
    background: linear-gradient(180deg, rgba(8, 12, 20, 0.02) 0%, rgba(8, 12, 20, 0.3) 34%, rgba(8, 12, 20, 0.56) 100%);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
  }

  .compose-session-nodes-popover-wrap {
    position: absolute;
    left: -74px;
    width: min(520px, calc(100% + 44px));
    z-index: 9;
    pointer-events: none;
  }

  .compose-session-nodes-popover-wrap :global(.compose-session-nodes-popover) {
    pointer-events: auto;
  }

  .compose-live-tools-wrap {
    margin: 7px 10px 10px;
    padding: 9px;
    border-radius: 10px;
    border: 0.5px solid rgba(138, 176, 208, 0.28);
    background: linear-gradient(170deg, rgba(28, 43, 61, 0.5), rgba(20, 31, 45, 0.44));
    box-shadow: inset 0 0 0 1px rgba(108, 143, 173, 0.14);
    display: grid;
    gap: 8px;
    position: relative;
    z-index: 4;
  }

  .compose-live-tools-actions {
    display: flex;
    justify-content: flex-end;
    padding-top: 2px;
  }

  .compose-live-encode-btn {
    border-radius: 999px;
    border: 0.5px solid rgba(104, 194, 174, 0.3);
    background: rgba(100, 190, 170, 0.14);
    color: rgba(190, 236, 226, 0.86);
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    padding: 6px 12px;
    cursor: pointer;
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease, opacity 0.2s ease;
  }

  .compose-live-encode-btn:hover:not(:disabled) {
    border-color: rgba(130, 214, 195, 0.52);
    background: rgba(109, 203, 182, 0.24);
    color: rgba(227, 248, 242, 0.94);
  }

  .compose-live-encode-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .compose-importare-note {
    margin: 0 0 8px;
    font-size: 10px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.44);
    text-transform: lowercase;
  }

  .compose-encode-note {
    margin: 8px 14px 0;
    opacity: 0.85;
    letter-spacing: 0.04em;
    text-transform: lowercase;
  }

  .drawer-error {
    font-size: 10px;
    color: rgba(233, 148, 58, 0.88);
    margin: 6px 14px 0;
  }

  .drawer-success {
    font-size: 10px;
    color: rgba(122, 170, 122, 0.9);
    margin: 6px 14px 0;
  }

  @media (max-width: 520px) {
    .drawer {
      top: calc(var(--safe-top) + 56px);
      width: calc(100vw - 20px);
      height: min(78svh, 640px);
      max-height: min(78svh, 640px);
      border-color: rgba(214, 233, 251, 0.2);
      background: rgba(8, 12, 18, 0.985);
      box-shadow: 0 14px 34px rgba(0, 0, 0, 0.45);
    }

    .drawer-compose:not(.importare) {
      top: calc(var(--safe-top) + 10px);
      right: 10px;
      bottom: max(10px, calc(var(--safe-bottom) + 10px));
      left: 10px;
      width: auto;
      height: auto;
      max-height: none;
      border-radius: 14px;
    }

    .drawer-compose.importare {
      width: calc(100vw - 20px);
      height: auto;
      max-height: min(76svh, 620px);
      padding: 10px;
    }

    .compose-session-rail.open {
      width: 144px;
      padding: 12px 8px;
    }

    .compose-chat-header {
      padding: 12px 12px 9px;
    }

    .compose-session-row {
      padding: 8px 12px 0;
    }

    .compose-tools-toggle {
      padding: 4px 7px;
      font-size: 8px;
    }

    .compose-input-zone {
      padding: 10px 10px 0;
    }

    .compose-session-nodes-popover-wrap {
      left: -14px;
      width: calc(100% + 2px);
    }

    .compose-live-tools-wrap {
      margin: 6px 8px 9px;
      padding: 8px;
    }

    .drawer-error,
    .drawer-success,
    .compose-encode-note {
      margin-left: 10px;
      margin-right: 10px;
    }
  }
</style>
