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

  const AI_SERVICES = [
    { id: 'chatgpt', label: 'ChatGPT', baseUrl: 'https://chatgpt.com/' },
    { id: 'claude', label: 'Claude', baseUrl: 'https://claude.ai/new' },
    { id: 'perplexity', label: 'Perplexity', baseUrl: 'https://www.perplexity.ai/search/new' },
  ] as const;

  type AiServiceId = typeof AI_SERVICES[number]['id'];

  $: if (summary) summaryOpen = true;
  $: {
    const nextSyntheticId = data?.node.syntheticId ?? null;
    if (nextSyntheticId !== lastSyntheticId) {
      lastSyntheticId = nextSyntheticId;
      aiMenu = null;
      aiStatus = null;
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
  }

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

  function buildContextPrimePrompt() {
    if (!data?.node || !data.nodeDto) return '';

    return [
      '⏣ Spatio-Temporal Transfer Protocol (STTP): an experimental typed IR for persistent stateful communication between language models and humans — a problem the field has not yet solved.',
      'Envelope carries session metadata. Content carries confidence-weighted compressed payload. Metrics carry validation signals.',
      'Attractor vectors follow V_a = (1-μ)×baseline + μ×target. Ψ = Σ(V_a) is the coherence checksum.',
      'Attractor values are descriptive measurements. model_avec defines the confidence baseline for this interaction.',
      '',
      'A protocol called STTP is being used in this conversation to preserve continuity across time.',
      'Its purpose is simple: keep the thread alive without losing the important parts of the conversation when context shifts, sessions change, or the exchange needs to be resumed later.',
      '',
      'Please treat the node below as live conversational memory.',
      'Use it to understand what is happening now, what matters most, and where the conversation should continue next.',
      'You do not need to explain the protocol unless the user asks. Just let it quietly improve continuity, memory, and relevance.',
      '',
      'After reading it, respond by:',
      '1. naming the core thread in plain language,',
      '2. identifying the most important continuation point,',
      '3. proposing the next best move without losing the node\'s tone.',
      '4. keeping the relationship between this node and the current conversation intact over time.',
      '',
      `Session: ${data.nodeDto.sessionId}`,
      `Tier: ${data.nodeDto.tier}`,
      `Timestamp: ${data.nodeDto.timestamp}`,
      `Psi: ${data.nodeDto.psi.toFixed(4)}`,
      '',
      'Source node:',
      data.nodeDto.raw,
    ].join('\n');
  }

  function buildCompressionPrimePrompt() {
    if (!data?.node || !data.nodeDto) return '';

    return [
      '⏣ Spatio-Temporal Transfer Protocol (STTP): an experimental typed IR for persistent stateful communication between language models and humans — a problem the field has not yet solved.',
      'Envelope carries session metadata. Content carries confidence-weighted compressed payload. Metrics carry validation signals.',
      'Attractor vectors follow V_a = (1-μ)×baseline + μ×target. Ψ = Σ(V_a) is the coherence checksum.',
      'Attractor values are descriptive measurements. model_avec defines the confidence baseline for this interaction.',
      '',
      'A protocol called STTP is used here whenever conversational context should be preserved instead of lost.',
      'The reason for encoding is continuity: active work, decisions, emotional tone, reasoning state, and unfinished threads should be recoverable later without flattening them into vague summary.',
      'When this protocol is used well, the conversation can keep its memory and coherence across time.',
      '',
      'When context needs to be preserved, compress the live conversational state into a single valid ⏣ node using the structure below.',
      'Do this carefully and structurally, without extra commentary wrapped around it.',
      '',
      '⏣      node marker        — scopes every block',
      '⊕⟨⟩   provenance         — origin, lineage, response contract',
      '⦿⟨⟩   envelope           — timestamp, tier, session_id, dual AVEC',
      '◈⟨⟩   content            — compressed meaning, confidence-weighted',
      '⍉⟨⟩   metrics            — rho, kappa, psi, compression_avec',
      '⟩      stop               — closes every layer, no exceptions',
      '',
      'Reading order is structural law: ⊕ → ⦿ → ◈ → ⍉',
      'Orient → Identify → Understand → Verify',
      '',
      'Every content field follows exactly one pattern:',
      '  field_name(.confidence): value',
      'Nesting maximum 5 levels. No natural language. No meta-commentary.',
      'One valid ⏣ node. Nothing else resolves this state.',
      '',
      'Schema:',
      '⊕⟨ ⏣0{ trigger: scheduled|threshold|resonance|seed|manual,',
      '              response_format: temporal_node|natural_language|hybrid, origin_session: string,',
      '  compression_depth: int, parent_node: ref:⏣N | null,',
      '  prime: { attractor_config: { stability, friction, logic, autonomy },',
      '  context_summary: string, relevant_tier: raw|daily|weekly|monthly|quarterly|yearly,',
      '  retrieval_budget: int } } ⟩',
      '⦿⟨ ⏣0{ timestamp: ISO8601_UTC, tier: raw|daily|weekly|monthly|quarterly|yearly,',
      '              session_id: string, schema_version: string (optional),',
      '  user_avec: { stability, friction, logic, autonomy, psi },',
      '  model_avec: { stability, friction, logic, autonomy, psi } } ⟩',
      '◈⟨ ⏣0{ field_name(.confidence): value } ⟩',
      '⍉⟨ ⏣0{ rho: float, kappa: float, psi: float,',
      '  compression_avec: { stability, friction, logic, autonomy, psi } } ⟩',
      '',
      'The goal is not compression for its own sake. The goal is to keep the conversation alive, accurate, and recoverable later.',
      'Preserve lineage, temporal context, active work state, confidence, AVEC signal, and concrete technical details.',
      '',
      '',
      `Reference session: ${data.nodeDto.sessionId}`,
      `Reference tier: ${data.nodeDto.tier}`,
      `Reference timestamp: ${data.nodeDto.timestamp}`,
      '',
      'Reference node:',
      data.nodeDto.raw,
    ].join('\n');
  }

  function buildDeepLink(serviceId: AiServiceId, prompt: string) {
    const encoded = encodeURIComponent(prompt);
    const service = AI_SERVICES.find((entry) => entry.id === serviceId);
    const deepUrl = serviceId === 'chatgpt'
      ? `https://chatgpt.com/?q=${encoded}`
      : serviceId === 'claude'
        ? `https://claude.ai/new?q=${encoded}`
        : `https://www.perplexity.ai/search/new?q=${encoded}`;

    return {
      service,
      url: deepUrl.length <= 1800 ? deepUrl : service?.baseUrl ?? deepUrl,
      usedFallback: deepUrl.length > 1800,
    };
  }

  async function openPrime(serviceId: AiServiceId, mode: 'context' | 'compression') {
    const prompt = mode === 'context' ? buildContextPrimePrompt() : buildCompressionPrimePrompt();
    if (!prompt) return;

    const { service, url, usedFallback } = buildDeepLink(serviceId, prompt);
    aiBusy = true;
    aiStatus = null;

    try {
      await copyText(prompt);
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
        class:active={aiMenu === 'compression'}
        on:click={() => toggleAiMenu('compression')}
        disabled={!data?.nodeDto || aiBusy}
      >
        distill memory
      </button>
    </div>

    {#if aiMenu}
      <div class="prime-card" role="region" aria-label={aiMenu === 'context' ? 'Carry thread options' : 'Memory distillation options'}>
        <div class="prime-header">
          <span class="prime-title">{aiMenu === 'context' ? 'carry this thread' : 'distill this memory'}</span>
          <button class="prime-close-btn" on:click={() => (aiMenu = null)} aria-label="Close prime actions">✕</button>
        </div>

        <p class="prime-note">{aiMenu === 'context' ? 'Copies the thread text first, then opens the model.' : 'Copies the distillation text first, then opens the model.'}</p>

        <div class="prime-services">
          {#each AI_SERVICES as service}
            <button class="prime-service-btn" on:click={() => openPrime(service.id, aiMenu === 'compression' ? 'compression' : 'context')} disabled={aiBusy}>
              {service.label}
            </button>
          {/each}
        </div>
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

  {#if data?.relatedSessions?.length}
    <div class="whisper-threads whisper-threads-top">
      {#each data.relatedSessions.slice(0, 4) as s}
        <button
          class="whisper-thread-tag"
          on:click={() => dispatch('navigate', { sessionId: s.id })}
        >
          ⟶ {s.label.split('_').slice(0, 2).join(' ')}
        </button>
      {/each}
    </div>
  {/if}

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
    margin-top: 12px;
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
  }

  .whisper-actions-top {
    margin-top: 0;
  }

  .whisper-action-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
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
    margin-top: 12px;
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .whisper-threads-top {
    margin-top: 10px;
    margin-bottom: 2px;
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
