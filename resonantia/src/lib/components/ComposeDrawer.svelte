<script lang="ts">
  import { formatTimestamp } from '@resonantia/core';
  import { tick } from 'svelte';

  type ComposeMode = 'live' | 'importare';

  type ComposeMessage = {
    role: 'user' | 'assistant';
    content: string;
    at: string;
  };

  type ComposeTabInfo = {
    id: string;
    title: string;
    sessionId: string;
  };

  type ComposeContextSession = {
    sessionId: string;
    label: string;
  };

  type ComposeContextNode = {
    key: string;
    sessionId: string;
    title: string;
    timestamp: string;
    tier: string;
    psi: number;
    preview: string;
  };

  type ComposeInjectedNode = {
    key: string;
    title: string;
    sessionId: string;
    timestamp: string;
  };

  type ComposeResult = {
    psi: number;
    duplicateSkipped: boolean;
    status: 'created' | 'updated' | 'duplicate' | 'skipped';
  } | null;

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
  let drawerEl: HTMLDivElement | null = null;
  let contextPanelEl: HTMLDivElement | null = null;

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

  async function revealContextPanel() {
    await tick();

    if (!drawerEl || !contextPanelEl) {
      return;
    }

    const drawerRect = drawerEl.getBoundingClientRect();
    const panelRect = contextPanelEl.getBoundingClientRect();
    const targetTop = drawerEl.scrollTop + (panelRect.top - drawerRect.top) - 8;
    drawerEl.scrollTo({ top: Math.max(targetTop, 0), behavior: 'smooth' });
  }

  function toggleContextPopup() {
    contextPopupOpen = !contextPopupOpen;
    if (contextPopupOpen) {
      void revealContextPanel();
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

{#if open}
  <div class="drawer drawer-compose" class:importare={mode === 'importare'} bind:this={drawerEl} role="dialog" aria-label={mode === 'importare' ? 'Import node' : 'Compose context'}>
    <div class="drawer-header">
      <span class="drawer-title">{mode === 'importare' ? 'importare' : 'compose'}</span>
      <button class="close-btn" on:click={onClose}>✕</button>
    </div>
    <input
      class="drawer-input drawer-session-input"
      type="text"
      placeholder="session id"
      bind:value={sessionId}
      on:input={onSessionInput}
    />
    {#if mode === 'live'}
      <div class="compose-tabs" aria-label="compose live tabs">
        {#each tabs as tab}
          <div class="compose-tab" class:active={tab.id === activeTabId}>
            <button class="compose-tab-btn" on:click={() => selectComposeTab(tab.id)}>{tab.title}</button>
            {#if tabs.length > 1}
              <button class="compose-tab-close" aria-label="close tab" on:click={() => closeComposeTab(tab.id)}>x</button>
            {/if}
          </div>
        {/each}
        {#if tabs.length < maxTabs}
          <button class="compose-tab-add" on:click={createComposeTab}>+ tab</button>
        {/if}
      </div>

      <div class="compose-thread" bind:this={composeThreadEl} aria-live="polite">
        {#if messages.length === 0}
          <p class="compose-empty">chat first, then encode the thread into one protocol node.</p>
        {:else}
          {#each messages as message}
            <article class={`compose-bubble ${message.role === 'assistant' ? 'assistant' : 'user'}`}>
              <header class="compose-bubble-meta">
                <span>{message.role === 'assistant' ? 'resonare' : 'you'}</span>
                <small>{formatTimestamp(message.at)}</small>
              </header>
              <p>{message.content}</p>
            </article>
          {/each}
        {/if}

        {#if replyLoading}
          <article class="compose-bubble assistant compose-pending">
            <header class="compose-bubble-meta">
              <span>resonare</span>
              <small>thinking…</small>
            </header>
          </article>
        {/if}
      </div>

      <div class="compose-entry">
        <textarea
          class="drawer-textarea compose-input"
          placeholder="message…"
          bind:value={draft}
          rows="3"
          on:input={onDraftInput}
          on:keydown={(event) => {
            if (event.key === 'Enter' && !event.shiftKey) {
              event.preventDefault();
              void sendComposeMessage();
            }
          }}
        ></textarea>
        <button
          class="drawer-btn submit compose-send"
          on:click={sendComposeMessage}
          disabled={loading || replyLoading || !draft.trim() || !sessionId.trim()}
        >
          {replyLoading ? 'thinking…' : 'send'}
        </button>
      </div>
    {:else}
      <p class="compose-importare-note">paste one complete node and store it directly.</p>
    {/if}

    <div class="compose-utility-actions">
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
      {:else}
        <span class="compose-utility-divider">•</span>
        <button class="compose-link-btn compose-link-pill compose-link-pill-live" data-tour-target="compose-switch-live" on:click={switchComposeToLive} disabled={pasteNodeLoading}>switch to create live</button>
      {/if}
    </div>

    {#if mode === 'live' && contextPopupOpen}
      <div class="compose-context-panel compose-context-popover" bind:this={contextPanelEl} aria-label="context injector">
        <div class="compose-context-head">
          <span>session context</span>
          <small>{injectedNodes.length} injected</small>
        </div>

        {#if contextSessions.length > 0}
          <div class="compose-context-sessions">
            {#each contextSessions as session}
              <button
                class="compose-context-session"
                class:origin={contextOriginSessionId === session.sessionId}
                class:selected={contextBrowseSessionId === session.sessionId}
                on:click={() => selectContextSession(session.sessionId)}
              >
                {session.label}
              </button>
            {/each}
          </div>
        {:else}
          <p class="compose-thread-note">no sessions attached to this thread yet.</p>
        {/if}

        {#if !contextBrowseSessionId}
          <p class="compose-thread-note">choose a session chip to browse raw nodes.</p>
        {:else if contextNodesLoading}
          <p class="compose-thread-note">loading session nodes...</p>
        {:else if contextNodesError}
          <p class="drawer-error compose-thread-error">{contextNodesError}</p>
        {:else if contextNodes.length > 0}
          <div class="compose-context-node-list">
            {#each contextNodes as node}
              <article class="compose-context-node">
                <div class="compose-context-node-body">
                  <p class="compose-context-node-title">{node.title}</p>
                  <p class="compose-context-node-meta">{formatTimestamp(node.timestamp)} · {node.sessionId}</p>
                  <p class="compose-context-node-preview">{node.preview}</p>
                </div>
                <button class="compose-context-inject" on:click={() => injectContextNode(node.key)}>inject raw</button>
              </article>
            {/each}
          </div>
        {:else}
          <p class="compose-thread-note">no raw nodes found for this session yet.</p>
        {/if}

        {#if injectedNodes.length > 0}
          <div class="compose-injected-strip">
            {#each injectedNodes as node}
              <button class="compose-injected-chip" on:click={() => removeInjectedNode(node.key)}>
                x {node.title}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#if promptCopyError}
      <p class="drawer-error">copy failed: {promptCopyError}</p>
    {/if}
    {#if mode === 'importare' || pasteNodeOpen}
      <div class="compose-paste-panel">
        <p class="compose-paste-intro">paste a complete STTP node and save it directly.</p>
        <div class="compose-paste-toolbar">
          <button class="compose-link-btn" type="button" on:click={togglePastePrettyView}>
            {pastePrettyView ? 'pretty view on' : 'pretty view off'}
          </button>
          {#if pastePrettyView}
            <span class="compose-paste-mode-note">visual only</span>
          {/if}
        </div>
        <div class="compose-paste-editor" class:pretty={pastePrettyView}>
          <div class="compose-paste-preview-wrap" bind:this={pastePreviewEl} aria-hidden="true">
            <pre class="compose-paste-preview">{@html pasteNodePreviewHtml}</pre>
          </div>
          {#if !pastePrettyView}
            <textarea
              class="drawer-textarea compose-paste-input compose-paste-input-highlighted"
              placeholder="paste one full STTP node"
              bind:this={pasteInputEl}
              bind:value={pasteNodeDraft}
              rows="9"
              wrap="soft"
              on:input={syncPasteEditorScroll}
              on:scroll={syncPasteEditorScroll}
            ></textarea>
          {/if}
        </div>
        <div class="compose-paste-actions">
          {#if mode === 'live'}
            <button class="drawer-btn cancel" on:click={toggleComposePasteNode} disabled={pasteNodeLoading}>cancel paste</button>
          {/if}
          <button class="drawer-btn submit" on:click={saveComposePastedNode} disabled={pasteNodeLoading || !pasteNodeDraft.trim() || !sessionId.trim()}>
            {pasteNodeLoading ? 'saving…' : 'save pasted node'}
          </button>
        </div>
      </div>
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
    <div class="drawer-actions compose-actions">
      <button class="drawer-btn cancel" on:click={onClose}>{mode === 'importare' ? 'close' : 'cancel'}</button>
      {#if mode === 'live'}
        <button class="drawer-btn submit" on:click={submitCompose} disabled={loading || replyLoading || messages.length === 0 || !sessionId.trim()}>
          {loading ? 'encoding…' : 'encode + store'}
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .drawer-compose {
    --compose-paste-height: 184px;
  }

  .drawer {
    position: absolute;
    top: 64px;
    bottom: 84px;
    left: 50%;
    transform: translateX(-50%);
    box-sizing: border-box;
    width: min(456px, calc(100vw - 32px));
    max-height: calc(100dvh - 148px);
    overflow-y: auto;
    overflow-x: hidden;
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

  .drawer-compose {
    top: max(64px, calc(var(--safe-top) + 46px));
    bottom: auto;
    width: min(520px, calc(100vw - 24px));
    height: min(66dvh, 560px);
    max-height: min(66dvh, 560px);
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 14px;
    overflow-x: hidden;
  }

  .drawer-compose.importare {
    height: auto;
    max-height: min(620px, calc(100dvh - 160px));
  }

  .compose-thread {
    min-height: 148px;
    max-height: none;
    flex: 1 1 auto;
    overflow-y: auto;
    border: 0.5px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.018);
    padding: 8px;
    margin-bottom: 4px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .compose-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 2px;
  }

  .compose-tab {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 999px;
    border: 0.5px solid rgba(149, 188, 224, 0.26);
    background: rgba(78, 119, 151, 0.1);
    padding: 2px;
  }

  .compose-tab.active {
    border-color: rgba(204, 231, 255, 0.58);
    background: rgba(114, 168, 210, 0.24);
  }

  .compose-tab-btn,
  .compose-tab-close,
  .compose-tab-add {
    border: none;
    background: transparent;
    color: rgba(204, 228, 248, 0.86);
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    cursor: pointer;
  }

  .compose-tab-btn {
    padding: 3px 6px;
  }

  .compose-tab-close {
    padding: 2px 4px;
    color: rgba(218, 191, 191, 0.84);
  }

  .compose-tab-add {
    border-radius: 999px;
    border: 0.5px dashed rgba(170, 204, 236, 0.3);
    background: rgba(97, 132, 161, 0.08);
    padding: 3px 7px;
  }

  .compose-context-panel {
    margin-bottom: 2px;
    padding: 7px 8px;
    border-radius: 10px;
    border: 0.5px solid rgba(151, 193, 232, 0.24);
    background: rgba(91, 128, 166, 0.06);
    display: grid;
    gap: 6px;
  }

  .compose-context-popover {
    margin-top: 2px;
    margin-bottom: 2px;
    border-color: rgba(172, 207, 237, 0.28);
    box-shadow: inset 0 0 0 1px rgba(118, 158, 192, 0.16);
    animation: composeContextPopupIn 0.18s ease;
  }

  .compose-context-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin-bottom: 0;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(192, 220, 245, 0.78);
  }

  .compose-context-head small {
    font-size: 8px;
    color: rgba(205, 223, 242, 0.66);
  }

  .compose-context-sessions {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 0;
  }

  .compose-context-session {
    border: 0.5px solid rgba(143, 184, 220, 0.24);
    background: rgba(66, 101, 130, 0.12);
    color: rgba(192, 220, 242, 0.82);
    border-radius: 999px;
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    padding: 4px 8px;
    cursor: pointer;
    max-width: 170px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compose-context-session.origin {
    border-color: rgba(205, 176, 116, 0.44);
    background: rgba(169, 132, 74, 0.18);
    color: rgba(242, 223, 186, 0.96);
  }

  .compose-context-session.selected {
    border-color: rgba(204, 231, 255, 0.58);
    background: rgba(114, 168, 210, 0.24);
    color: rgba(232, 244, 255, 0.96);
  }

  .compose-context-session.origin.selected {
    border-color: rgba(230, 211, 158, 0.72);
    background: linear-gradient(135deg, rgba(192, 151, 88, 0.34), rgba(106, 78, 40, 0.3));
    color: rgba(253, 242, 220, 0.98);
  }

  .compose-context-node-list {
    max-height: 108px;
    overflow-y: auto;
    display: grid;
    gap: 5px;
  }

  .compose-context-node {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    border: 0.5px solid rgba(156, 188, 214, 0.2);
    border-radius: 7px;
    padding: 6px;
    background: rgba(45, 71, 95, 0.14);
  }

  .compose-context-node-body {
    min-width: 0;
    display: grid;
    gap: 3px;
  }

  .compose-context-node-title,
  .compose-context-node-meta,
  .compose-context-node-preview {
    margin: 0;
  }

  .compose-context-node-title {
    font-size: 8px;
    color: rgba(229, 241, 252, 0.88);
  }

  .compose-context-node-meta {
    font-size: 8px;
    color: rgba(184, 208, 228, 0.7);
  }

  .compose-context-node-preview {
    font-size: 8px;
    line-height: 1.35;
    color: rgba(199, 220, 238, 0.78);
    word-break: break-word;
  }

  .compose-context-inject {
    border: 0.5px solid rgba(178, 213, 240, 0.3);
    background: rgba(100, 147, 187, 0.2);
    color: rgba(232, 244, 255, 0.92);
    border-radius: 999px;
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    padding: 5px 7px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .compose-injected-strip {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 0;
  }

  .compose-injected-chip {
    border: 0.5px solid rgba(199, 180, 132, 0.36);
    background: rgba(196, 166, 104, 0.14);
    color: rgba(232, 220, 189, 0.9);
    border-radius: 999px;
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    padding: 4px 7px;
    cursor: pointer;
  }

  .compose-thread-note {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.03em;
    color: rgba(200, 225, 245, 0.72);
  }

  .compose-thread-error {
    margin-top: 6px;
  }

  .compose-empty {
    margin: auto 0;
    text-align: center;
    font-size: 10px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.34);
    letter-spacing: 0.03em;
  }

  .compose-bubble {
    border: 0.5px solid rgba(255, 255, 255, 0.065);
    border-radius: 9px;
    padding: 5px 7px;
    background: rgba(255, 255, 255, 0.03);
  }

  .compose-bubble.user {
    border-color: rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
  }

  .compose-bubble.assistant {
    border-color: rgba(214, 184, 109, 0.26);
    background: rgba(214, 184, 109, 0.08);
  }

  .compose-bubble-meta {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 5px;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.5);
  }

  .compose-bubble-meta small {
    font-size: 8px;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.35);
  }

  .compose-bubble p {
    margin: 0;
    font-size: 10px;
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.76);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .compose-pending {
    animation: composePulse 1.4s ease-in-out infinite;
  }

  .compose-entry {
    display: flex;
    gap: 6px;
    align-items: flex-end;
    flex-shrink: 0;
  }

  .compose-input {
    margin-bottom: 0;
    min-height: 64px;
  }

  .compose-send {
    min-width: 88px;
    margin-bottom: 1px;
  }

  .compose-importare-note {
    margin: 0 0 8px;
    font-size: 10px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.44);
    text-transform: lowercase;
  }

  .compose-utility-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 3px;
    margin-bottom: 2px;
  }

  .compose-utility-divider {
    color: rgba(255, 255, 255, 0.18);
    font-size: 9px;
    line-height: 1;
    user-select: none;
  }

  .compose-link-btn {
    border: 0.5px solid rgba(126, 173, 198, 0.24);
    background: rgba(80, 119, 143, 0.09);
    padding: 4px 9px;
    margin: 0;
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.06em;
    text-transform: lowercase;
    border-radius: 999px;
    color: rgba(191, 223, 242, 0.72);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
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

  .compose-link-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .compose-paste-panel {
    margin-top: 7px;
    padding: 10px;
    border-radius: 10px;
    border: 0.5px dashed rgba(255, 255, 255, 0.11);
    background: rgba(255, 255, 255, 0.012);
  }

  .compose-paste-intro {
    margin: 0 0 8px;
    font-size: 9px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.48);
    text-transform: lowercase;
  }

  .compose-paste-input {
    min-height: var(--compose-paste-height);
    height: var(--compose-paste-height);
    margin-bottom: 0;
    resize: none;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
    overflow-x: hidden;
  }

  .compose-paste-toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 7px;
  }

  .compose-paste-mode-note {
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    color: rgba(168, 188, 230, 0.74);
  }

  .compose-paste-editor {
    position: relative;
    min-height: var(--compose-paste-height);
    height: var(--compose-paste-height);
    margin-bottom: 0;
    min-width: 0;
  }

  .compose-paste-editor.pretty {
    border: 0.5px solid rgba(170, 193, 240, 0.24);
    border-radius: 6px;
    box-shadow: inset 0 0 0 1px rgba(112, 142, 204, 0.12);
  }

  .compose-paste-preview-wrap {
    position: absolute;
    inset: 0;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    border-radius: 6px;
    background: rgba(7, 8, 12, 0.9);
    overflow: auto;
    pointer-events: none;
  }

  .compose-paste-preview {
    margin: 0;
    padding: 8px 10px;
    font-size: 11px;
    line-height: 1.45;
    color: rgba(233, 235, 242, 0.88);
    white-space: pre-wrap;
    word-break: break-word;
    overflow-wrap: anywhere;
    min-height: 100%;
    max-width: 100%;
    box-sizing: border-box;
  }

  .compose-paste-preview :global(span) {
    overflow-wrap: anywhere;
    word-break: break-word;
    max-width: 100%;
  }

  .compose-paste-preview :global(.sttp-empty) {
    color: rgba(255, 255, 255, 0.38);
    font-style: italic;
  }

  .compose-paste-preview :global(.sttp-marker) {
    color: #f7c97b;
  }

  .compose-paste-preview :global(.sttp-brace) {
    color: rgba(214, 221, 255, 0.82);
  }

  .compose-paste-preview :global(.sttp-key) {
    color: #7cc6ff;
  }

  .compose-paste-preview :global(.sttp-confidence) {
    color: #ffd68f;
  }

  .compose-paste-preview :global(.sttp-number) {
    color: #8be6a8;
  }

  .compose-paste-preview :global(.sttp-keyword) {
    color: #efc995;
  }

  .drawer-compose.importare .compose-paste-input {
    min-height: 224px;
    height: 224px;
  }

  .drawer-compose.importare .compose-paste-editor {
    min-height: 224px;
    height: 224px;
  }

  .compose-paste-input-highlighted {
    position: relative;
    z-index: 1;
    background: transparent;
    border-color: rgba(255, 255, 255, 0.14);
    color: transparent;
    -webkit-text-fill-color: transparent;
    caret-color: rgba(244, 247, 255, 0.92);
    overflow-x: hidden;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    word-break: break-word;
  }

  .compose-paste-input-highlighted::selection {
    background: rgba(143, 183, 255, 0.28);
  }

  .compose-paste-input-highlighted::placeholder {
    color: transparent;
  }

  .compose-paste-editor.pretty .compose-paste-preview-wrap {
    position: relative;
    border: none;
    border-radius: 6px;
  }

  .compose-paste-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .compose-actions {
    justify-content: flex-end;
    align-items: center;
  }

  .compose-encode-note {
    margin-top: 8px;
    opacity: 0.85;
    letter-spacing: 0.04em;
    text-transform: lowercase;
  }

  @keyframes composePulse {
    0%,
    100% {
      opacity: 0.64;
    }
    50% {
      opacity: 1;
    }
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

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
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
    margin-bottom: 8px;
    outline: none;
    transition: border-color 0.2s;
  }

  .drawer-session-input {
    margin-bottom: 6px;
    font-family: 'IBM Plex Sans', sans-serif;
    font-weight: 500;
    letter-spacing: 0.01em;
    border-radius: 5px;
    padding: 7px 9px;
  }

  .drawer-session-input::placeholder {
    color: rgba(216, 231, 246, 0.48);
  }

  .drawer-input:focus {
    border-color: rgba(255, 255, 255, 0.25);
  }

  .drawer-textarea {
    width: 100%;
    box-sizing: border-box;
    background: rgba(255, 255, 255, 0.04);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 10px;
    color: rgba(255, 255, 255, 0.7);
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    resize: vertical;
    margin-bottom: 10px;
    outline: none;
    transition: border-color 0.2s;
  }

  .drawer-textarea:focus {
    border-color: rgba(255, 255, 255, 0.25);
  }

  .drawer-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 6px 14px;
    border-radius: 999px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .drawer-btn.cancel {
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.3);
  }

  .drawer-btn.cancel:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.22);
    color: rgba(255, 255, 255, 0.62);
    background: rgba(255, 255, 255, 0.04);
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
    color: rgba(233, 148, 58, 0.88);
    margin: 6px 0 0;
  }

  .drawer-success {
    font-size: 10px;
    color: rgba(122, 170, 122, 0.9);
    margin: 6px 0 0;
  }

  @media (hover: none) and (pointer: coarse) {
    .drawer-compose .drawer-input,
    .drawer-compose .drawer-textarea {
      font-size: 16px;
      line-height: 1.35;
    }
  }

  @media (max-width: 520px) {
    .drawer {
      top: calc(var(--safe-top) + 56px);
      bottom: max(74px, calc(var(--safe-bottom) + 58px));
      width: calc(100vw - 20px);
      max-height: calc(100dvh - 130px);
      padding: 16px;
    }

    .drawer-compose {
      top: calc(var(--safe-top) + 56px);
      bottom: auto;
      width: calc(100vw - 24px);
      height: min(68svh, 512px);
      max-height: min(68svh, 512px);
      padding: 10px;
      border-color: rgba(214, 233, 251, 0.2);
      background: rgba(8, 12, 18, 0.985);
      box-shadow: 0 14px 34px rgba(0, 0, 0, 0.45);
    }

    .drawer-header {
      margin-bottom: 7px;
    }

    .drawer-title {
      font-size: 14px;
    }

    .drawer-session-input {
      margin-bottom: 5px;
      padding: 6px 8px;
    }

    .compose-thread {
      max-height: none;
      min-height: 152px;
      padding: 7px;
    }

    .compose-entry {
      flex-direction: column;
      align-items: stretch;
      gap: 6px;
    }

    .compose-send {
      width: 100%;
      min-width: 0;
    }

    .compose-input {
      min-height: 60px;
    }

    .compose-utility-actions {
      align-items: center;
      gap: 4px;
      margin-top: 2px;
      margin-bottom: 1px;
      row-gap: 4px;
    }

    .compose-link-btn {
      width: auto;
      text-align: center;
      padding: 4px 7px;
      font-size: 8px;
      letter-spacing: 0.04em;
    }

    .compose-paste-actions {
      flex-direction: row;
      align-items: center;
      justify-content: flex-end;
      gap: 6px;
    }

    .compose-paste-actions .drawer-btn {
      width: auto;
      text-align: center;
      padding: 5px 10px;
    }

    .compose-utility-divider {
      display: none;
    }
  }
</style>
