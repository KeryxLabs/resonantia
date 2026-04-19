<script lang="ts">
  import { formatTimestamp } from "@resonantia/core";
  import type { ComposeMessage, ComposeMessageCrossSessionMeta } from "./types";

  export let messages: ComposeMessage[] = [];
  export let replyLoading = false;
  export let threadEl: HTMLDivElement | null = null;

  function compactSessionLabel(sessionId: string) {
    return sessionId.startsWith("s:") ? sessionId.slice(2) : sessionId;
  }

  function crossSessionBadgeLabel(meta: ComposeMessageCrossSessionMeta) {
    return `cross-session from ${compactSessionLabel(meta.sourceSessionId)}`;
  }
</script>

<div class="compose-thread" bind:this={threadEl} aria-live="polite">
  {#if messages.length === 0}
    <p class="compose-empty">chat first, then encode the thread into one protocol node.</p>
  {:else}
    {#each messages as message}
      <article class={`compose-bubble ${message.role === 'assistant' ? 'assistant' : 'user'}`}>
        <header class="compose-bubble-meta">
          <span>{message.role === 'assistant' ? 'resonare' : 'you'}</span>
          <div class="compose-bubble-meta-right">
            {#if message.crossSession}
              <span
                class="compose-cross-session-badge"
                title={`source ${message.crossSession.sourceSessionId} → thread ${message.crossSession.targetSessionId}`}
              >
                {crossSessionBadgeLabel(message.crossSession)}
              </span>
            {/if}
            <small>{formatTimestamp(message.at)}</small>
          </div>
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

<style>
  .compose-thread {
    min-height: 0;
    max-height: none;
    flex: 1 1 auto;
    overflow-y: auto;
    border: none;
    border-radius: 0;
    background: transparent;
    padding: 20px clamp(14px, 5.6vw, 118px) 18px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
    position: relative;
  }

  .compose-thread::before {
    content: '';
    position: absolute;
    inset: 0;
    background: radial-gradient(ellipse at 50% 50%, rgba(104, 188, 171, 0.045) 0%, rgba(104, 188, 171, 0) 62%);
    pointer-events: none;
  }

  .compose-thread::after {
    content: '';
    position: absolute;
    inset: 0;
    background:
      linear-gradient(180deg, rgba(6, 10, 16, 0.18) 0%, rgba(6, 10, 16, 0) 12%, rgba(6, 10, 16, 0) 88%, rgba(6, 10, 16, 0.24) 100%),
      linear-gradient(90deg, rgba(6, 10, 16, 0.12) 0%, rgba(6, 10, 16, 0) 14%, rgba(6, 10, 16, 0) 86%, rgba(6, 10, 16, 0.12) 100%);
    pointer-events: none;
  }

  .compose-thread > * {
    position: relative;
    z-index: 1;
  }

  .compose-empty {
    margin: auto 0;
    text-align: center;
    font-size: 12px;
    line-height: 1.8;
    color: rgba(192, 209, 227, 0.2);
    letter-spacing: 0.03em;
    font-style: italic;
    text-transform: lowercase;
    text-shadow: 0 2px 12px rgba(2, 7, 14, 0.22);
  }

  .compose-bubble {
    width: fit-content;
    max-width: min(74%, 640px);
    border: 0.5px solid rgba(255, 255, 255, 0.055);
    border-radius: 10px 10px 10px 2px;
    padding: 9px 13px;
    background: rgba(255, 255, 255, 0.026);
    box-shadow: 0 5px 14px rgba(3, 8, 15, 0.2);
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
  }

  .compose-bubble.user {
    margin-left: auto;
    border-color: rgba(100, 190, 170, 0.18);
    background: rgba(100, 190, 170, 0.075);
    border-radius: 10px 10px 2px 10px;
    box-shadow: 0 6px 15px rgba(16, 58, 50, 0.2);
  }

  .compose-bubble.assistant {
    margin-right: auto;
    border-color: rgba(255, 255, 255, 0.065);
    background: rgba(255, 255, 255, 0.03);
  }

  .compose-bubble-meta {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 4px;
    font-size: 9px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.39);
  }

  .compose-bubble-meta-right {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
  }

  .compose-bubble-meta small {
    font-size: 8px;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.3);
  }

  .compose-cross-session-badge {
    border: 0.5px solid rgba(217, 188, 128, 0.34);
    background: rgba(189, 152, 85, 0.13);
    color: rgba(242, 225, 192, 0.85);
    border-radius: 999px;
    padding: 1px 6px;
    font-size: 7px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    line-height: 1.4;
    white-space: nowrap;
  }

  .compose-bubble p {
    margin: 0;
    font-size: 12.5px;
    line-height: 1.62;
    letter-spacing: 0.01em;
    color: rgba(195, 205, 215, 0.88);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .compose-bubble.user p {
    color: rgba(195, 230, 220, 0.92);
  }

  .compose-pending {
    animation: composePulse 1.4s ease-in-out infinite;
  }

  @keyframes composeBubbleIn {
    from {
      opacity: 0;
      transform: translateY(6px) scale(0.996);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  @keyframes composePulse {
    0%,
    100% {
      opacity: 0.72;
    }
    50% {
      opacity: 0.94;
    }
  }

  @media (max-width: 520px) {
    .compose-thread {
      max-height: none;
      min-height: 0;
      padding: 12px 0;
      gap: 12px;
    }

    .compose-bubble {
      max-width: 90%;
      padding: 7px 11px;
    }

    .compose-bubble p {
      font-size: 12px;
    }
  }
</style>
