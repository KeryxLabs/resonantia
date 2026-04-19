<script lang="ts">
  import { onDestroy } from 'svelte';
  import ComposeTabs from './compose/ComposeTabs.svelte';
  import ComposeThread from './compose/ComposeThread.svelte';
  import ComposeInputRow from './compose/ComposeInputRow.svelte';
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
    ComposeProviderUsage,
    ComposeResult,
    ComposeTabInfo,
    ComposeTokenUsage,
    CrossSessionRoutingPreference,
  } from './compose/types';

  export let open = false;
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
  export let saveComposePastedNode: () => Promise<void> | void = () => {};
  export let submitCompose: (mode?: 'save' | 'compact') => Promise<void> | void = () => {};
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
  let toolsToggleEl: HTMLButtonElement | null = null;
  let liveToolsPopoverEl: HTMLDivElement | null = null;
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
  let starCanvasEl: HTMLCanvasElement | null = null;
  let observedStarCanvasEl: HTMLCanvasElement | null = null;
  let starResizeObserver: ResizeObserver | null = null;
  let starDrawRaf = 0;
  let renderDrawer = false;
  let layerState: 'closed' | 'opening' | 'open' | 'closing' = 'closed';
  let layerCloseTimeout: ReturnType<typeof setTimeout> | null = null;
  let layerOpenRaf = 0;

  const COMPOSE_LAYER_TRANSITION_MS = 420;

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

  $: if (pasteNodeOpen) {
    queueMicrotask(syncPasteEditorScroll);
  }

  $: {
    if (open) {
      clearLayerCloseTimeout();

      if (!renderDrawer || layerState === 'closing' || layerState === 'closed') {
        renderDrawer = true;
        layerState = 'opening';
        queueLayerOpenState();
      }
    } else if (renderDrawer && layerState !== 'closing') {
      cancelLayerOpenState();
      layerState = 'closing';
      layerCloseTimeout = setTimeout(() => {
        renderDrawer = false;
        layerState = 'closed';
        layerCloseTimeout = null;
      }, COMPOSE_LAYER_TRANSITION_MS);
    }
  }

  $: {
    if (starCanvasEl !== observedStarCanvasEl) {
      if (starResizeObserver) {
        starResizeObserver.disconnect();
        starResizeObserver = null;
      }

      observedStarCanvasEl = starCanvasEl;

      if (starCanvasEl && typeof ResizeObserver !== 'undefined') {
        starResizeObserver = new ResizeObserver(() => {
          scheduleComposeStarfieldDraw();
        });
        starResizeObserver.observe(starCanvasEl);
      }

      scheduleComposeStarfieldDraw();
    }
  }

  $: if (renderDrawer && starCanvasEl) {
    scheduleComposeStarfieldDraw();
  }

  $: if (!open) {
    contextPopupOpen = false;
    chatSettingsOpen = false;
    liveToolsOpen = false;
    sessionNodesPopoverOpen = false;
  }

  $: {
    if (pasteNodeOpen && !previousPasteNodeOpen) {
      liveToolsOpen = true;
    }

    previousPasteNodeOpen = pasteNodeOpen;
  }

  $: {
    const nextAutoScrollKey = `${open ? '1' : '0'}|${messages.length}|${replyLoading ? '1' : '0'}`;
    if (nextAutoScrollKey !== composeAutoScrollKey) {
      composeAutoScrollKey = nextAutoScrollKey;
      if (open) {
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
    const target = event.target;
    if (!(target instanceof Node)) {
      return;
    }

    if (sessionNodesPopoverOpen) {
      if (sessionNodesPopoverEl?.contains(target)) {
        return;
      }

      if (target instanceof Element && target.closest('.compose-session-chip')) {
        return;
      }

      sessionNodesPopoverOpen = false;
    }

    if (liveToolsOpen) {
      if (liveToolsPopoverEl?.contains(target) || toolsToggleEl?.contains(target)) {
        return;
      }

      liveToolsOpen = false;
      chatSettingsOpen = false;
    }
  }

  function cancelLayerOpenState() {
    if (layerOpenRaf) {
      cancelAnimationFrame(layerOpenRaf);
      layerOpenRaf = 0;
    }
  }

  function scheduleComposeStarfieldDraw() {
    if (starDrawRaf) {
      return;
    }

    if (typeof requestAnimationFrame !== 'function') {
      drawComposeStarfield();
      return;
    }

    starDrawRaf = requestAnimationFrame(() => {
      starDrawRaf = 0;
      drawComposeStarfield();
    });
  }

  function drawComposeStarfield() {
    if (!starCanvasEl) {
      return;
    }

    const rect = starCanvasEl.getBoundingClientRect();
    const width = Math.max(1, Math.floor(rect.width));
    const height = Math.max(1, Math.floor(rect.height));
    const dpr = typeof window !== 'undefined' ? Math.min(window.devicePixelRatio || 1, 2) : 1;
    const canvasWidth = Math.max(1, Math.floor(width * dpr));
    const canvasHeight = Math.max(1, Math.floor(height * dpr));

    if (starCanvasEl.width !== canvasWidth || starCanvasEl.height !== canvasHeight) {
      starCanvasEl.width = canvasWidth;
      starCanvasEl.height = canvasHeight;
    }

    const ctx = starCanvasEl.getContext('2d');
    if (!ctx) {
      return;
    }

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvasWidth, canvasHeight);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const nodes = [
      { x: 0.12, y: 0.15, r: 28, core: 3 },
      { x: 0.85, y: 0.1, r: 20, core: 2.5 },
      { x: 0.9, y: 0.75, r: 38, core: 4 },
      { x: 0.08, y: 0.7, r: 18, core: 2 },
      { x: 0.5, y: 0.08, r: 14, core: 2 },
      { x: 0.75, y: 0.5, r: 22, core: 2.5 },
      { x: 0.22, y: 0.88, r: 16, core: 2 },
      { x: 0.6, y: 0.85, r: 12, core: 1.5 },
    ];

    for (const node of nodes) {
      const x = node.x * width;
      const y = node.y * height;

      ctx.beginPath();
      ctx.arc(x, y, node.r, 0, Math.PI * 2);
      ctx.strokeStyle = 'rgba(100, 190, 170, 0.07)';
      ctx.lineWidth = 0.5;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(x, y, node.r * 0.35, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(100, 190, 170, 0.06)';
      ctx.fill();

      ctx.beginPath();
      ctx.arc(x, y, node.core, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(100, 190, 170, 0.22)';
      ctx.fill();
    }

    for (let i = 0; i < nodes.length; i += 1) {
      for (let j = i + 1; j < nodes.length; j += 1) {
        const a = nodes[i];
        const b = nodes[j];
        const distance = Math.hypot((a.x - b.x) * width, (a.y - b.y) * height);

        if (distance >= 340) {
          continue;
        }

        ctx.beginPath();
        ctx.moveTo(a.x * width, a.y * height);
        ctx.lineTo(b.x * width, b.y * height);
        ctx.strokeStyle = `rgba(100, 190, 170, ${0.025 * (1 - (distance / 340))})`;
        ctx.lineWidth = 0.4;
        ctx.stroke();
      }
    }
  }

  function clearLayerCloseTimeout() {
    if (layerCloseTimeout) {
      clearTimeout(layerCloseTimeout);
      layerCloseTimeout = null;
    }
  }

  function queueLayerOpenState() {
    cancelLayerOpenState();

    if (typeof requestAnimationFrame !== 'function') {
      layerState = 'open';
      return;
    }

    layerOpenRaf = requestAnimationFrame(() => {
      layerOpenRaf = requestAnimationFrame(() => {
        layerState = 'open';
        layerOpenRaf = 0;
      });
    });
  }

  onDestroy(() => {
    cancelLayerOpenState();
    clearLayerCloseTimeout();

    if (starResizeObserver) {
      starResizeObserver.disconnect();
      starResizeObserver = null;
    }

    if (starDrawRaf && typeof cancelAnimationFrame === 'function') {
      cancelAnimationFrame(starDrawRaf);
      starDrawRaf = 0;
    }
  });

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

{#if renderDrawer}
  <div
    class="compose-immersive-root"
    class:is-opening={layerState === 'opening'}
    class:is-open={layerState === 'open'}
    class:is-closing={layerState === 'closing'}
  >
    <div class="compose-immersive-scrim" aria-hidden="true"></div>
    <div class="drawer drawer-compose" role="dialog" aria-label="Live chat" aria-modal="true">
    <div class="compose-live-shell" bind:this={liveShellEl}>
      <canvas class="compose-live-stars" bind:this={starCanvasEl} aria-hidden="true"></canvas>

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
            bind:this={toolsToggleEl}
            on:click={toggleLiveTools}
            title="chat tools"
          >
            {liveToolsOpen ? 'hide tools' : 'tools'}
          </button>

          <button class="compose-close-btn" type="button" on:click={onClose} aria-label="close chat">x</button>
        </div>

        {#if liveToolsOpen}
          <div class="compose-live-tools-popover">
            <div class="compose-live-tools-wrap" bind:this={liveToolsPopoverEl}>
              <ComposeUtilityActions
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
                <div class="compose-live-mode-grid">
                  <button
                    class="compose-live-mode-btn compose-live-mode-save"
                    type="button"
                    on:click={() => void submitCompose('save')}
                    disabled={loading || replyLoading || messages.length === 0 || !sessionId.trim()}
                  >
                    {loading ? 'encoding…' : 'save'}
                  </button>
                  <button
                    class="compose-live-mode-btn compose-live-mode-compact"
                    type="button"
                    on:click={() => void submitCompose('compact')}
                    disabled={loading || replyLoading || messages.length === 0 || !sessionId.trim()}
                  >
                    {loading ? 'encoding…' : 'compact'}
                  </button>
                </div>
                <p class="compose-live-tools-hint">save stores a node; compact stores, clears this thread, and reinjects the compacted node.</p>
              </div>
            </div>
          </div>
        {/if}

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
      </div>
    </div>

    {#if promptCopyError}
      <p class="drawer-error">copy failed: {promptCopyError}</p>
    {/if}

    {#if loading && encodePromptSent}
      <p class="drawer-success compose-encode-note">encoding prompt sent</p>
    {/if}
    {#if error}<p class="drawer-error">{error}</p>{/if}
    {#if result}
      <p class="drawer-success">
        {composeOutcomeLabel(result.status, result.duplicateSkipped)} · Ψ {result.psi.toFixed(4)}
      </p>
    {/if}

    </div>
  </div>
{/if}

<style>
  .compose-immersive-root {
    position: fixed;
    inset: 0;
    z-index: 172;
    isolation: isolate;
    overflow: hidden;
  }

  .compose-immersive-scrim {
    position: absolute;
    inset: 0;
    pointer-events: none;
    opacity: 0;
    background:
      linear-gradient(180deg, rgba(3, 7, 12, 0.16) 0%, rgba(3, 7, 12, 0.62) 100%);
    transition: opacity 420ms cubic-bezier(0.18, 0.8, 0.18, 1);
  }

  .compose-immersive-scrim::before {
    content: '';
    position: absolute;
    inset: -8% -10% -14%;
    background:
      radial-gradient(ellipse at 50% 114%, rgba(102, 154, 185, 0.18) 0%, rgba(102, 154, 185, 0) 58%),
      radial-gradient(ellipse at 16% 114%, rgba(78, 128, 156, 0.12) 0%, rgba(78, 128, 156, 0) 48%),
      radial-gradient(ellipse at 84% 114%, rgba(78, 128, 156, 0.1) 0%, rgba(78, 128, 156, 0) 46%);
    opacity: 0.4;
  }

  .compose-immersive-scrim::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(180deg, rgba(6, 10, 16, 0.22) 0%, rgba(6, 10, 16, 0) 24%, rgba(6, 10, 16, 0) 72%, rgba(6, 10, 16, 0.24) 100%),
      linear-gradient(90deg, rgba(6, 10, 16, 0.2) 0%, rgba(6, 10, 16, 0) 14%, rgba(6, 10, 16, 0) 86%, rgba(6, 10, 16, 0.2) 100%);
    opacity: 0.62;
  }

  .compose-immersive-root .drawer {
    opacity: 0;
    transform: translateY(26px) scale(0.988);
    transition:
      opacity 420ms cubic-bezier(0.18, 0.8, 0.18, 1),
      transform 420ms cubic-bezier(0.18, 0.8, 0.18, 1);
  }

  .compose-immersive-root.is-open .drawer,
  .compose-immersive-root.is-opening .drawer {
    opacity: 1;
    transform: translateY(0) scale(1);
  }

  .compose-immersive-root.is-open .compose-immersive-scrim,
  .compose-immersive-root.is-opening .compose-immersive-scrim {
    opacity: 1;
  }

  .compose-immersive-root.is-closing .drawer {
    opacity: 0;
    transform: translateY(18px) scale(0.992);
  }

  .compose-immersive-root.is-closing .compose-immersive-scrim {
    opacity: 0;
  }

  .drawer-compose {
    --compose-paste-height: 184px;
  }

  .drawer {
    position: absolute;
    top: max(0px, var(--safe-top, 0px));
    right: 0;
    bottom: max(0px, var(--safe-bottom, 0px));
    left: 0;
    box-sizing: border-box;
    width: auto;
    height: auto;
    max-height: none;
    overflow: hidden;
    background: transparent;
    border: none;
    border-radius: 0;
    padding: 0;
    z-index: 1;
    font-family: 'IBM Plex Sans', sans-serif;
    overscroll-behavior: contain;
    scrollbar-width: thin;
  }

  .drawer-compose {
    display: flex;
    flex-direction: column;
    gap: 0;
    overflow-y: hidden;
    overflow-x: hidden;
  }

  .compose-live-shell {
    position: relative;
    min-height: 0;
    height: 100%;
    flex: 1;
    display: flex;
    overflow: hidden;
    border-radius: 0;
    background: #0b0f15;
    isolation: isolate;
    box-shadow: inset 0 0 0 0.5px rgba(188, 216, 236, 0.12);
  }

  .compose-live-shell::before {
    content: '';
    position: absolute;
    inset: 0;
    background:
      radial-gradient(ellipse at 50% 114%, rgba(105, 156, 187, 0.2) 0%, rgba(105, 156, 187, 0) 62%),
      radial-gradient(ellipse at 18% 114%, rgba(84, 133, 161, 0.12) 0%, rgba(84, 133, 161, 0) 48%),
      radial-gradient(ellipse at 82% 114%, rgba(84, 133, 161, 0.1) 0%, rgba(84, 133, 161, 0) 46%);
    opacity: 0.44;
    pointer-events: none;
    z-index: 0;
  }

  .compose-live-shell::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(90deg, rgba(7, 11, 18, 0.22) 0%, rgba(7, 11, 18, 0) 18%, rgba(7, 11, 18, 0) 82%, rgba(7, 11, 18, 0.22) 100%),
      linear-gradient(180deg, rgba(7, 11, 18, 0.2) 0%, rgba(7, 11, 18, 0) 26%, rgba(7, 11, 18, 0) 72%, rgba(7, 11, 18, 0.28) 100%);
    pointer-events: none;
    z-index: 0;
  }

  .compose-live-stars {
    pointer-events: none;
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    display: block;
    opacity: 0.55;
    animation: composeStarFieldFloat 28s ease-in-out infinite alternate;
    z-index: 0;
  }

  @keyframes composeStarFieldFloat {
    from {
      transform: scale(1) translateY(0);
    }
    to {
      transform: scale(1.008) translateY(-1px);
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

  .compose-live-tools-popover {
    position: absolute;
    top: 52px;
    right: 12px;
    width: min(360px, calc(100% - 24px));
    z-index: 14;
    pointer-events: none;
  }

  .compose-live-tools-wrap {
    pointer-events: auto;
    padding: 9px;
    border-radius: 12px;
    border: 0.5px solid rgba(102, 143, 172, 0.22);
    background: linear-gradient(170deg, rgba(11, 22, 33, 0.86), rgba(9, 17, 27, 0.9));
    box-shadow: 0 10px 22px rgba(2, 7, 14, 0.34), inset 0 0 0 1px rgba(76, 116, 145, 0.1);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    display: grid;
    gap: 7px;
    position: relative;
    z-index: 4;
    max-height: min(74vh, calc(100% - 80px));
    overflow-y: auto;
  }

  .compose-live-tools-actions {
    display: grid;
    gap: 5px;
    padding-top: 2px;
    border-top: 0.5px solid rgba(114, 150, 178, 0.16);
    margin-top: 1px;
    padding-top: 7px;
  }

  .compose-live-mode-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 6px;
  }

  .compose-live-mode-btn {
    border-radius: 999px;
    border: 0.5px solid rgba(103, 148, 178, 0.22);
    background: rgba(39, 64, 83, 0.44);
    color: rgba(183, 211, 231, 0.8);
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    padding: 6px 10px;
    cursor: pointer;
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease, opacity 0.2s ease;
  }

  .compose-live-mode-save {
    border-color: rgba(98, 163, 146, 0.26);
    background: rgba(37, 74, 66, 0.48);
    color: rgba(178, 220, 206, 0.86);
  }

  .compose-live-mode-compact {
    border-color: rgba(110, 151, 184, 0.26);
    background: rgba(36, 66, 92, 0.48);
    color: rgba(188, 214, 234, 0.86);
  }

  .compose-live-mode-btn:hover:not(:disabled) {
    border-color: rgba(143, 188, 217, 0.38);
    background: rgba(57, 91, 118, 0.58);
    color: rgba(214, 234, 248, 0.92);
  }

  .compose-live-mode-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .compose-live-tools-hint {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.03em;
    color: rgba(162, 190, 210, 0.6);
    text-transform: lowercase;
    line-height: 1.4;
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
      top: max(0px, var(--safe-top, 0px));
      bottom: max(0px, var(--safe-bottom, 0px));
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

    .compose-live-tools-popover {
      top: 50px;
      right: 8px;
      width: calc(100% - 16px);
    }

    .compose-input-zone {
      padding: 10px 10px 0;
    }

    .compose-session-nodes-popover-wrap {
      left: -14px;
      width: calc(100% + 2px);
    }

    .compose-live-tools-wrap {
      padding: 8px;
      max-height: min(70vh, calc(100% - 72px));
    }

    .compose-live-mode-grid {
      grid-template-columns: 1fr;
    }

    .drawer-error,
    .drawer-success,
    .compose-encode-note {
      margin-left: 10px;
      margin-right: 10px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .compose-immersive-root .drawer,
    .compose-immersive-scrim,
    .compose-live-stars {
      transition: none;
      animation: none;
    }
  }
</style>
