<script lang="ts">
  import { createEventDispatcher } from 'svelte';
  import { AVEC_HEX, avecColor, formatTimestamp } from '@resonantia/core';
  import type { AiSummary, CollapseCardData } from '@resonantia/core';

  export let data: CollapseCardData | null = null;
  export let visible = false;
  export let summary: AiSummary | null = null;
  export let transmuting = false;
  export let transmuteError: string | null = null;
  export let openExternalUrl: (url: string) => Promise<void> = async (url: string) => {
    if (typeof window !== 'undefined') {
      window.open(url, '_blank', 'noopener,noreferrer');
      return;
    }

    throw new Error('External URL handler is unavailable in this runtime.');
  };

  const dispatch = createEventDispatcher<{
    close: void;
    navigate: { sessionId: string };
    transmute: void;
    continueInApp: {
      sessionId: string;
      prompt: string;
      sourceNodeRaw: string;
      threadCandidates: Array<{ sessionId: string; label: string }>;
    };
  }>();

  $: avec = data?.nodeDto?.userAvec ?? null;

  $: sessionLabel = data?.node.sessionId.replace(/_/g, ' ') ?? '—';
  $: timestamp    = data?.node.timestamp ? formatTimestamp(data.node.timestamp) : '—';
  $: tier         = data?.node.tier ?? '—';
  let summaryOpen = false;
  let aiMenu: 'context' | 'compression' | null = null;
  let aiBusy = false;
  let aiStatus: string | null = null;
  let lastSyntheticId: string | null = null;
  let showMoreTargets = false;
  let cardMoreOpen = false;
  let pendingLaunchUrl: string | null = null;
  let pendingLaunchLabel: string | null = null;
  let pendingLaunchPayloadLabel: string | null = null;
  let pendingLaunchOpen = false;

  const AI_SERVICES = [
    { id: 'chatgpt', label: 'ChatGPT', baseUrl: 'https://chatgpt.com/' },
    { id: 'claude', label: 'Claude', baseUrl: 'https://claude.ai/new' },
    { id: 'cursor', label: 'Cursor', baseUrl: 'https://cursor.com/link/prompt' },
    { id: 'zed', label: 'Zed', baseUrl: 'zed://agent' },
    { id: 't3', label: 'T3 Chat', baseUrl: 'https://t3.chat/new' },
    { id: 'perplexity', label: 'Perplexity', baseUrl: 'https://www.perplexity.ai/' },
    { id: 'v0', label: 'v0', baseUrl: 'https://v0.app/chat' },
  ] as const;
  const MAX_DEEPLINK_PROMPT_CHARS = 1800;
  const MAX_DEEPLINK_URL_CHARS = 1900;

  type AiServiceId = typeof AI_SERVICES[number]['id'];
  const PRIMARY_AI_SERVICE_IDS: AiServiceId[] = ['chatgpt', 'claude'];
  $: primaryServices = AI_SERVICES.filter((service) => PRIMARY_AI_SERVICE_IDS.includes(service.id));
  $: secondaryServices = AI_SERVICES.filter((service) => !PRIMARY_AI_SERVICE_IDS.includes(service.id));

  $: if (summary) summaryOpen = true;
  $: {
    const nextSyntheticId = data?.node.syntheticId ?? null;
    if (nextSyntheticId !== lastSyntheticId) {
      lastSyntheticId = nextSyntheticId;
      aiMenu = null;
      aiStatus = null;
      showMoreTargets = false;
      cardMoreOpen = false;
      pendingLaunchUrl = null;
      pendingLaunchLabel = null;
      pendingLaunchPayloadLabel = null;
      pendingLaunchOpen = false;
    }
  }
  $: shellStyle = [
    'position:fixed',
    'bottom:18px',
    'left:50%',
    `transform:translateX(-50%) translateY(${visible ? '0' : 'calc(100% + 32px)'})`,
    `opacity:${visible ? '1' : '0'}`,
    'width:min(312px, calc(100vw - 28px))',
    'max-width:calc(100vw - 32px)',
    'background:rgba(12, 14, 20, 0.96)',
    'border:0.5px solid rgba(255, 255, 255, 0.1)',
    'border-radius:13px',
    'padding:16px',
    'z-index:30',
    'box-shadow:0 18px 44px rgba(0, 0, 0, 0.34)',
    'backdrop-filter:blur(20px)',
    '-webkit-backdrop-filter:blur(20px)',
    `pointer-events:${visible ? 'all' : 'none'}`,
    'font-family:"Departure Mono", "Courier New", monospace',
    'transition:transform 0.5s cubic-bezier(0.16, 1, 0.3, 1), opacity 0.4s ease'
  ].join(';');

  function bar(val: number | null) {
    return `${((val ?? 0) * 100).toFixed(0)}%`;
  }

  function handleTransmute() {
    if (summary) {
      summaryOpen = !summaryOpen;
      return;
    }
    dispatch('transmute');
  }

  function toggleAiMenu(nextMenu: 'context' | 'compression') {
    aiMenu = aiMenu === nextMenu ? null : nextMenu;
    aiStatus = null;
    showMoreTargets = false;
  }

  function toggleCardMore() {
    cardMoreOpen = !cardMoreOpen;
    if (!cardMoreOpen) {
      aiMenu = null;
      showMoreTargets = false;
    }
  }

  function buildContextDeepLinkPrompt() {
    if (!data?.nodeDto) return '';

    return data.nodeDto.raw;
  }

  function buildCompressionDeepLinkPrompt() {
    if (!data?.nodeDto) return '';

    return [
      'Distill this memory into 3 sections: Snapshot, Continuation, Risks.',
      `Session: ${data.nodeDto.sessionId}`,
      `Tier: ${data.nodeDto.tier}`,
      `Timestamp: ${data.nodeDto.timestamp}`,
      'Use clipboard context for full details.',
    ].join('\n');
  }

  function extractContextSummary(rawNode: string): string | null {
    if (!rawNode.trim()) {
      return null;
    }

    const quotedMatch = rawNode.match(/context_summary(?:\([^)]*\))?\s*:\s*"([^"]+)"/i);
    const unquotedMatch = rawNode.match(/context_summary(?:\([^)]*\))?\s*:\s*([^,\n}]+)/i);
    const source = quotedMatch?.[1] ?? unquotedMatch?.[1] ?? '';
    const cleaned = source
      .trim()
      .replace(/^['"`]+|['"`]+$/g, '')
      .replace(/[_-]+/g, ' ')
      .replace(/\s+/g, ' ')
      .trim();

    return cleaned || null;
  }

  $: contextSummary = data?.nodeDto ? extractContextSummary(data.nodeDto.raw) : null;

  async function copyText(text: string) {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    if (typeof document !== 'undefined') {
      const textarea = document.createElement('textarea');
      textarea.value = text;
      textarea.setAttribute('readonly', 'true');
      textarea.style.position = 'fixed';
      textarea.style.opacity = '0';
      document.body.appendChild(textarea);
      textarea.select();
      document.execCommand('copy');
      document.body.removeChild(textarea);
      return;
    }

    throw new Error('Clipboard is not available in this context.');
  }

  function canonicalSessionId(value: string) {
    const source = value.trim();
    if (!source) {
      return '';
    }

    return source.startsWith('s:') ? source.slice(2) : source;
  }

  function buildContextPrimePrompt() {
    if (!data?.node || !data.nodeDto) return '';

    return [
      'STTP protocol introduction: you are receiving a full STTP node as active memory context.',
      'Please do not explain the protocol unless the user explicitly asks.',
       'This just provides you context about where the user is and what the current thread is.',
      ' Please engage with the content and the user as the node is just at transport.',
      '',
      'full_node:',
      data.nodeDto.raw,
    ].join('\n');
  }

  function buildCompressionPrimePrompt() {
    if (!data?.node || !data.nodeDto) return '';

    const summaryLines = summary
      ? [
          summary.topic ? `- topic: ${summary.topic}` : null,
          summary.whatHappened ? `- what happened: ${summary.whatHappened}` : null,
          summary.whereWeLeftOff ? `- where we left off: ${summary.whereWeLeftOff}` : null,
          summary.pickBackUpWith ? `- pick back up with: ${summary.pickBackUpWith}` : null,
        ].filter((line): line is string => Boolean(line))
      : [];

    return [
      'Distill this memory into a compact continuity handoff.',
      'Return output in 3 sections only: Snapshot, Continuation, Risks.',
      'Keep it concise and practical.',
      '',
      'Requirements:',
      '- Preserve technical specifics and intent.',
      '- Keep unresolved questions explicit.',
      '- Include the single best next action.',
      '',
      `Session: ${data.nodeDto.sessionId}`,
      `Tier: ${data.nodeDto.tier}`,
      `Timestamp: ${data.nodeDto.timestamp}`,
      `Psi: ${data.nodeDto.psi.toFixed(4)}`,
      '',
      'Existing transmutation notes:',
      summaryLines.length > 0 ? summaryLines.join('\n') : '- none',
      '',
      'Source node:',
      data.nodeDto.raw,
    ].join('\n');
  }

  function toUrlSafePrompt(prompt: string) {
    return prompt
      .normalize('NFC')
      .replace(/\r\n?/g, '\n')
      .replace(/[\u0000-\u0008\u000B\u000C\u000E-\u001F\u007F]/g, ' ')
      .replace(/[ \t]{2,}/g, ' ')
      .replace(/\n{3,}/g, '\n\n')
      .trim();
  }

  function buildServiceDeepLink(serviceId: AiServiceId, encodedPrompt: string) {
    return serviceId === 'chatgpt'
      ? `https://chatgpt.com/?q=${encodedPrompt}&hints=search`
      : serviceId === 'claude'
        ? `https://claude.ai/new?q=${encodedPrompt}`
        : serviceId === 'cursor'
          ? `https://cursor.com/link/prompt?text=${encodedPrompt}`
          : serviceId === 'zed'
            ? `zed://agent?prompt=${encodedPrompt}`
            : serviceId === 't3'
              ? `https://t3.chat/new?q=${encodedPrompt}`
              : serviceId === 'v0'
                ? `https://v0.app/chat?q=${encodedPrompt}`
                : `https://www.perplexity.ai/?q=${encodedPrompt}`;
  }

  function buildDeepLink(serviceId: AiServiceId, prompt: string) {
    const safePrompt = toUrlSafePrompt(prompt);
    const service = AI_SERVICES.find((entry) => entry.id === serviceId);
    const deepUrl = buildServiceDeepLink(serviceId, encodeURIComponent(safePrompt));
    const limitExceeded = safePrompt.length > MAX_DEEPLINK_PROMPT_CHARS || deepUrl.length > MAX_DEEPLINK_URL_CHARS;

    return {
      service,
      url: limitExceeded ? service?.baseUrl ?? deepUrl : deepUrl,
      usedFallback: limitExceeded,
      skippedAutoPrompt: limitExceeded,
    };
  }

  async function continueInResonantia() {
    const prompt = buildContextPrimePrompt();
    const sessionId = data?.nodeDto?.sessionId?.trim();
    const sourceNodeRaw = data?.nodeDto?.raw?.trim() ?? '';
    if (!prompt || !sessionId || !sourceNodeRaw) return;

    const relatedSessions = data?.relatedSessions ?? [];

    const threadCandidates = [
      {
        sessionId,
        label: canonicalSessionId(sessionId).replace(/_/g, ' ') || sessionId,
      },
      ...relatedSessions
        .slice(0, 4)
        .map((session) => {
          const candidateSessionId = canonicalSessionId(session.label || session.id);
          return {
            sessionId: candidateSessionId,
            label: canonicalSessionId(session.label || session.id).replace(/_/g, ' ') || candidateSessionId,
          };
        })
        .filter((candidate) => candidate.sessionId && candidate.sessionId !== sessionId),
    ];

    aiBusy = true;
    aiStatus = null;

    try {
      dispatch('continueInApp', { sessionId, prompt, sourceNodeRaw, threadCandidates });

      let copied = false;
      try {
        await copyText(prompt);
        copied = true;
      } catch {
        copied = false;
      }

      aiStatus = copied
        ? 'continued in resonantia · thread context copied'
        : 'continued in resonantia';
      aiMenu = null;
    } catch (err) {
      aiStatus = String(err);
    } finally {
      aiBusy = false;
    }
  }

  async function dismissLaunchNoticeAndOpen() {
    if (!pendingLaunchUrl) {
      pendingLaunchOpen = false;
      return;
    }

    const url = pendingLaunchUrl;
    const label = pendingLaunchLabel ?? 'model';
    const payloadLabel = pendingLaunchPayloadLabel ?? 'text';

    pendingLaunchOpen = false;
    pendingLaunchUrl = null;
    pendingLaunchLabel = null;
    pendingLaunchPayloadLabel = null;

    aiBusy = true;
    aiStatus = null;
    try {
      await openExternalUrl(url);
      aiStatus = `copied ${payloadLabel} · opening ${label}`;
    } catch (err) {
      aiStatus = String(err);
    } finally {
      aiBusy = false;
    }
  }

  async function openPrime(serviceId: AiServiceId, mode: 'context' | 'compression') {
    const prompt = mode === 'context' ? buildContextPrimePrompt() : buildCompressionPrimePrompt();
    const deepLinkPrompt = mode === 'context' ? buildContextDeepLinkPrompt() : buildCompressionDeepLinkPrompt();
    if (!prompt) return;

    const { service, url, usedFallback, skippedAutoPrompt } = buildDeepLink(serviceId, deepLinkPrompt || prompt);
    aiBusy = true;
    aiStatus = null;
    pendingLaunchOpen = false;
    pendingLaunchUrl = null;
    pendingLaunchLabel = null;
    pendingLaunchPayloadLabel = null;

    try {
      await copyText(prompt);
      if (skippedAutoPrompt) {
        pendingLaunchUrl = url;
        pendingLaunchLabel = service?.label ?? 'model';
        pendingLaunchPayloadLabel = mode === 'context' ? 'thread text' : 'distillation text';
        pendingLaunchOpen = true;
        aiStatus = `${mode === 'context' ? 'node' : 'prompt'} too large for auto launch · copied to clipboard`;
        aiMenu = null;
        return;
      }

      await openExternalUrl(url);
      aiStatus = usedFallback
          ? `copied ${mode === 'context' ? 'thread text' : 'distillation text'} · opening ${service?.label ?? 'model'}`
        : `copied ${mode === 'context' ? 'thread text' : 'distillation text'} · sent to ${service?.label ?? 'model'}`;
      aiMenu = null;
    } catch (err) {
      aiStatus = String(err);
    } finally {
      aiBusy = false;
    }
  }
</script>

<div
  class="whisper-card"
  class:visible
  style={shellStyle}
  role="complementary"
  aria-label="Collapse detail"
>
  <div class="whisper-header">
    <div class="whisper-left">
      <span class="whisper-session-label">{sessionLabel}</span>
      <span class="whisper-tier-pill">{tier}</span>
    </div>
    <button class="whisper-close-btn" on:click={() => dispatch('close')} aria-label="Close">✕</button>
  </div>

  <div class="whisper-timestamp">{timestamp}</div>

  {#if contextSummary}
    <div class="whisper-context-summary" role="note" aria-label="Context summary">
      <span class="whisper-context-summary-label">thread</span>
      <p class="whisper-context-summary-value" title={contextSummary}>{contextSummary}</p>
    </div>
  {/if}

  <div class="whisper-actions whisper-actions-top">
    <div class="whisper-action-row">
      <button
        class="whisper-transmute-btn"
        on:click={handleTransmute}
        disabled={!data?.nodeDto || transmuting}
      >
        {transmuting ? 'transmuting…' : 'transmute'}
      </button>

      <button
        class="whisper-prime-btn"
        class:active={aiMenu === 'context'}
        on:click={() => toggleAiMenu('context')}
        disabled={!data?.nodeDto || aiBusy}
      >
        carry thread
      </button>

      <button
        class="whisper-prime-btn"
        class:active={cardMoreOpen}
        on:click={toggleCardMore}
        disabled={!data?.nodeDto || aiBusy}
      >
        {cardMoreOpen ? 'less' : 'more'}
      </button>
    </div>

    {#if cardMoreOpen}
      <div class="whisper-more-panel" role="region" aria-label="More card actions">
        <div class="whisper-more-actions">
          <button
            class="whisper-more-btn"
            class:active={aiMenu === 'compression'}
            on:click={() => toggleAiMenu('compression')}
            disabled={!data?.nodeDto || aiBusy}
          >
            distill memory
          </button>
        </div>

        {#if data?.relatedSessions?.length}
          <div class="whisper-threads whisper-threads-inline">
            {#each data.relatedSessions.slice(0, 4) as s}
              <button
                class="whisper-thread-tag"
                on:click={() => dispatch('navigate', { sessionId: s.id })}
              >
                -> {s.label.split('_').slice(0, 2).join(' ')}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#if aiMenu}
      <div class="prime-card" role="region" aria-label={aiMenu === 'context' ? 'Carry thread options' : 'Memory distillation options'}>
        <div class="prime-header">
          <span class="prime-title">{aiMenu === 'context' ? 'carry this thread' : 'distill this memory'}</span>
          <button class="prime-close-btn" on:click={() => { aiMenu = null; showMoreTargets = false; }} aria-label="Close prime actions">✕</button>
        </div>

        <p class="prime-note">{aiMenu === 'context' ? 'Copies enriched thread context first, then opens your target.' : 'Copies memory distillation context first, then opens your target.'}</p>

        {#if aiMenu === 'context'}
          <div class="prime-services prime-services-app">
            <button class="prime-service-btn prime-service-btn-app" on:click={continueInResonantia} disabled={aiBusy}>
              continue in resonantia
            </button>
          </div>
        {/if}

        <div class="prime-services">
          {#each primaryServices as service}
            <button class="prime-service-btn" on:click={() => openPrime(service.id, aiMenu === 'compression' ? 'compression' : 'context')} disabled={aiBusy}>
              {service.label}
            </button>
          {/each}

          {#if secondaryServices.length > 0}
            <button class="prime-service-btn prime-service-btn-more" on:click={() => (showMoreTargets = !showMoreTargets)} disabled={aiBusy}>
              {showMoreTargets ? 'less' : 'more'}
            </button>
          {/if}
        </div>

        {#if showMoreTargets}
          <div class="prime-services prime-services-more">
            {#each secondaryServices as service}
              <button class="prime-service-btn" on:click={() => openPrime(service.id, aiMenu === 'compression' ? 'compression' : 'context')} disabled={aiBusy}>
                {service.label}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    {/if}

    {#if pendingLaunchOpen}
      <div class="launch-notice" role="dialog" aria-label="Launch confirmation">
        <div class="launch-notice-header">
          <span class="launch-notice-title">auto launch paused</span>
          <button class="launch-notice-close" on:click={dismissLaunchNoticeAndOpen} aria-label="Dismiss and open link">✕</button>
        </div>
        <p class="launch-notice-copy">
          this node is too large to inject in a launch url. we copied it to your clipboard.
        </p>
        <button class="launch-notice-btn" on:click={dismissLaunchNoticeAndOpen} disabled={aiBusy}>
          dismiss + open {pendingLaunchLabel ?? 'model'}
        </button>
      </div>
    {/if}

    {#if summary && summaryOpen}
      <div class="alchemy-card" role="region" aria-label="Transmutation">
        <div class="alchemy-header">
          <span class="alchemy-title">transmutation</span>
          <button class="alchemy-close-btn" on:click={() => (summaryOpen = false)} aria-label="Close transmutation">✕</button>
        </div>

        <div class="alchemy-body">
          {#if summary.topic}
            <div class="alchemy-block">
              <span class="alchemy-label">topic</span>
              <p class="alchemy-text">{summary.topic}</p>
            </div>
          {/if}
          {#if summary.whatHappened}
            <div class="alchemy-block">
              <span class="alchemy-label">what happened</span>
              <p class="alchemy-text">{summary.whatHappened}</p>
            </div>
          {/if}
          {#if summary.whereWeLeftOff}
            <div class="alchemy-block">
              <span class="alchemy-label">where we left off</span>
              <p class="alchemy-text">{summary.whereWeLeftOff}</p>
            </div>
          {/if}
          {#if summary.vibe}
            <div class="alchemy-block">
              <span class="alchemy-label">vibe</span>
              <p class="alchemy-text">{summary.vibe}</p>
            </div>
          {/if}
          {#if summary.pickBackUpWith}
            <div class="alchemy-block">
              <span class="alchemy-label">pick back up with</span>
              <p class="alchemy-text">{summary.pickBackUpWith}</p>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>

  {#if avec}
    <div class="whisper-avec-grid">
      {#each [
        { key: 'stability' as const, label: 'grounding', val: avec.stability },
        { key: 'friction'  as const, label: 'wear', val: avec.friction  },
        { key: 'logic'     as const, label: 'clarity', val: avec.logic      },
        { key: 'autonomy'  as const, label: 'self-trust', val: avec.autonomy   },
      ] as dim}
        <div class="whisper-dim">
          <span class="whisper-dim-label">{dim.label}</span>
          <div class="whisper-bar-track">
            <div
              class="whisper-bar-fill"
              style="width:{bar(dim.val)};background:{AVEC_HEX[dim.key]}"
            ></div>
          </div>
          <span class="whisper-dim-val">{dim.val.toFixed(2)}</span>
        </div>
      {/each}
    </div>
  {/if}

  {#if transmuteError}
    <p class="whisper-transmute-error">{transmuteError}</p>
  {/if}

  {#if aiStatus}
    <p class="whisper-prime-status">{aiStatus}</p>
  {/if}
</div>

<style>
  .whisper-card {
    will-change: transform, opacity;
    overflow: visible;
  }

  .whisper-card.visible {
    transform: translateX(-50%) translateY(0);
    opacity: 1;
    pointer-events: all;
  }

  .whisper-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 4px;
  }

  .whisper-left {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .whisper-session-label {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.75);
    line-height: 1.3;
    max-width: 220px;
  }

  .whisper-tier-pill {
    display: inline-block;
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.25);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 99px;
    padding: 2px 7px;
    align-self: flex-start;
  }

  .whisper-close-btn {
    background: transparent;
    border: none;
    color: rgba(255, 255, 255, 0.3);
    font-size: 14px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    transition: color 0.2s;
  }
  .whisper-close-btn:hover { color: rgba(255, 255, 255, 0.7); }

  .whisper-timestamp {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.25);
    letter-spacing: 0.06em;
    margin-bottom: 12px;
  }

  .whisper-context-summary {
    margin: -4px 0 10px;
    padding: 8px 10px;
    border-radius: 10px;
    border: 0.5px solid rgba(170, 206, 229, 0.2);
    background: rgba(39, 58, 70, 0.16);
  }

  .whisper-context-summary-label {
    display: block;
    font-size: 8px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(177, 214, 237, 0.64);
    margin-bottom: 4px;
  }

  .whisper-context-summary-value {
    margin: 0;
    font-size: 10px;
    line-height: 1.45;
    color: rgba(214, 232, 244, 0.82);
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;
    text-overflow: ellipsis;
    word-break: break-word;
  }

  .whisper-avec-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    margin-top: 12px;
  }

  .whisper-dim {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .whisper-dim-label {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.3);
  }

  .whisper-bar-track {
    height: 2px;
    background: rgba(255, 255, 255, 0.06);
    border-radius: 2px;
    overflow: hidden;
  }

  .whisper-bar-fill {
    height: 100%;
    border-radius: 2px;
    transition: width 0.7s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .whisper-dim-val {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.5);
  }

  .whisper-actions {
    margin-top: 10px;
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    width: 100%;
  }

  .whisper-actions-top {
    margin-top: 0;
  }

  .whisper-action-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    width: 100%;
  }

  .whisper-more-panel {
    margin-top: 9px;
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    padding: 8px;
    border-radius: 10px;
    border: 0.5px solid rgba(132, 166, 188, 0.18);
    background: rgba(28, 40, 52, 0.24);
  }

  .whisper-more-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    width: 100%;
  }

  .whisper-more-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 7px 9px;
    border-radius: 999px;
    border: 0.5px solid rgba(126, 173, 198, 0.24);
    background: rgba(80, 119, 143, 0.09);
    color: rgba(191, 223, 242, 0.72);
    cursor: pointer;
  }

  .whisper-more-btn.active {
    color: rgba(224, 240, 249, 0.88);
    border-color: rgba(141, 192, 223, 0.4);
    background: rgba(89, 136, 166, 0.15);
  }

  .whisper-more-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .whisper-transmute-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    padding: 8px 12px;
    border-radius: 999px;
    border: 0.5px solid rgba(199, 182, 132, 0.28);
    background: rgba(196, 166, 104, 0.08);
    color: rgba(229, 214, 182, 0.78);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .whisper-transmute-btn:hover:not(:disabled) {
    color: rgba(247, 235, 210, 0.9);
    border-color: rgba(215, 191, 136, 0.42);
    background: rgba(196, 166, 104, 0.13);
  }

  .whisper-transmute-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .whisper-prime-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 8px 11px;
    border-radius: 999px;
    border: 0.5px solid rgba(126, 173, 198, 0.24);
    background: rgba(80, 119, 143, 0.09);
    color: rgba(191, 223, 242, 0.72);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .whisper-prime-btn:hover:not(:disabled),
  .whisper-prime-btn.active {
    color: rgba(224, 240, 249, 0.88);
    border-color: rgba(141, 192, 223, 0.4);
    background: rgba(89, 136, 166, 0.15);
  }

  .whisper-prime-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .whisper-transmute-error {
    margin-top: 10px;
    font-size: 10px;
    line-height: 1.5;
    color: rgba(233, 148, 58, 0.82);
  }

  .whisper-prime-status {
    margin-top: 10px;
    font-size: 10px;
    line-height: 1.5;
    color: rgba(169, 214, 238, 0.76);
  }

  .launch-notice {
    margin-top: 10px;
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
    padding: 10px;
    border-radius: 10px;
    border: 0.5px solid rgba(201, 189, 142, 0.34);
    background: rgba(28, 23, 15, 0.92);
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.28);
  }

  .launch-notice-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 6px;
  }

  .launch-notice-title {
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(235, 223, 178, 0.78);
  }

  .launch-notice-close {
    background: transparent;
    border: none;
    color: rgba(235, 223, 178, 0.56);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }

  .launch-notice-copy {
    margin: 0 0 8px;
    font-size: 10px;
    line-height: 1.45;
    color: rgba(235, 223, 178, 0.72);
  }

  .launch-notice-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 7px 10px;
    border-radius: 999px;
    border: 0.5px solid rgba(224, 205, 140, 0.35);
    background: rgba(196, 166, 104, 0.14);
    color: rgba(244, 232, 194, 0.9);
    cursor: pointer;
  }

  .launch-notice-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .prime-card {
    position: absolute;
    left: 0;
    bottom: calc(100% + 14px);
    width: min(320px, calc(100vw - 56px));
    max-width: calc(100vw - 56px);
    padding: 14px 14px 12px;
    border-radius: 14px;
    border: 0.5px solid rgba(116, 167, 201, 0.22);
    background:
      linear-gradient(180deg, rgba(18, 28, 38, 0.96), rgba(12, 17, 24, 0.98)),
      radial-gradient(circle at top left, rgba(123, 180, 214, 0.12), transparent 55%);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.34);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: 2;
  }

  .prime-card::after {
    content: '';
    position: absolute;
    left: 82px;
    top: 100%;
    width: 14px;
    height: 14px;
    transform: translateY(-7px) rotate(45deg);
    background: rgba(12, 17, 24, 0.96);
    border-right: 0.5px solid rgba(116, 167, 201, 0.22);
    border-bottom: 0.5px solid rgba(116, 167, 201, 0.22);
  }

  .prime-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 8px;
  }

  .prime-title {
    font-size: 10px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: rgba(206, 228, 240, 0.76);
  }

  .prime-close-btn {
    background: transparent;
    border: none;
    color: rgba(206, 228, 240, 0.46);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }

  .prime-note {
    margin: 0 0 10px;
    font-size: 10px;
    line-height: 1.5;
    color: rgba(204, 224, 236, 0.54);
  }

  .prime-services {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .prime-services-app {
    margin-bottom: 8px;
  }

  .prime-services-more {
    margin-top: 8px;
    padding-top: 8px;
    border-top: 0.5px solid rgba(177, 215, 235, 0.16);
  }

  .prime-service-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.08em;
    padding: 7px 10px;
    border-radius: 999px;
    border: 0.5px solid rgba(177, 215, 235, 0.18);
    background: rgba(177, 215, 235, 0.06);
    color: rgba(224, 240, 249, 0.72);
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s, opacity 0.2s;
  }

  .prime-service-btn:hover:not(:disabled) {
    color: rgba(244, 250, 253, 0.9);
    border-color: rgba(177, 215, 235, 0.34);
    background: rgba(177, 215, 235, 0.12);
  }

  .prime-service-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .prime-service-btn-app {
    border-color: rgba(198, 217, 173, 0.26);
    background: rgba(153, 189, 110, 0.11);
    color: rgba(224, 241, 202, 0.86);
  }

  .prime-service-btn-more {
    border-color: rgba(177, 215, 235, 0.26);
    background: rgba(177, 215, 235, 0.1);
    color: rgba(235, 245, 251, 0.86);
  }

  .alchemy-card {
    position: absolute;
    left: 0;
    bottom: calc(100% + 14px);
    width: min(360px, calc(100vw - 56px));
    max-width: calc(100vw - 56px);
    padding: 14px 14px 12px;
    border-radius: 14px;
    border: 0.5px solid rgba(198, 168, 108, 0.22);
    background:
      linear-gradient(180deg, rgba(38, 29, 18, 0.96), rgba(18, 15, 12, 0.98)),
      radial-gradient(circle at top left, rgba(196, 166, 104, 0.14), transparent 55%);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.34);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
  }

  .alchemy-card::after {
    content: '';
    position: absolute;
    left: 34px;
    top: 100%;
    width: 14px;
    height: 14px;
    transform: translateY(-7px) rotate(45deg);
    background: rgba(22, 18, 14, 0.96);
    border-right: 0.5px solid rgba(198, 168, 108, 0.22);
    border-bottom: 0.5px solid rgba(198, 168, 108, 0.22);
  }

  .alchemy-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-bottom: 10px;
  }

  .alchemy-title {
    font-size: 10px;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: rgba(229, 214, 182, 0.72);
  }

  .alchemy-close-btn {
    background: transparent;
    border: none;
    color: rgba(229, 214, 182, 0.42);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
  }

  .alchemy-body {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 260px;
    overflow: auto;
    padding-right: 4px;
  }

  .alchemy-block {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .alchemy-label {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(207, 194, 160, 0.42);
  }

  .alchemy-text {
    font-size: 11px;
    line-height: 1.65;
    color: rgba(240, 232, 214, 0.8);
  }

  .whisper-threads {
    margin-top: 10px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    width: 100%;
  }

  .whisper-threads-inline {
    margin-top: 8px;
  }

  .whisper-thread-tag {
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.06em;
    padding: 3px 9px;
    border-radius: 99px;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: transparent;
    color: rgba(255, 255, 255, 0.3);
    cursor: pointer;
    transition: all 0.2s;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .whisper-thread-tag:hover {
    border-color: rgba(255, 255, 255, 0.25);
    color: rgba(255, 255, 255, 0.7);
  }

  @media (max-width: 640px) {
    .whisper-card {
      bottom: 14px;
      padding: 14px;
      border-radius: 12px;
    }

    .prime-card {
      width: min(284px, calc(100vw - 48px));
      max-width: calc(100vw - 48px);
      bottom: calc(100% + 10px);
    }

    .alchemy-card {
      left: 0;
      width: min(300px, calc(100vw - 48px));
      max-width: calc(100vw - 48px);
      bottom: calc(100% + 10px);
    }

    .alchemy-text {
      font-size: 10px;
    }
  }
</style>
