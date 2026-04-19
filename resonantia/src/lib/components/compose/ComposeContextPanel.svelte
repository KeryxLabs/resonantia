<script lang="ts">
  import { formatTimestamp } from "@resonantia/core";
  import type { ComposeContextNode, ComposeContextSession, ComposeInjectedNode } from "./types";

  export let contextOriginSessionId = "";
  export let contextBrowseSessionId = "";
  export let contextSessions: ComposeContextSession[] = [];
  export let contextNodes: ComposeContextNode[] = [];
  export let contextNodesLoading = false;
  export let contextNodesError: string | null = null;
  export let injectedNodes: ComposeInjectedNode[] = [];
  export let selectContextSession: (sessionId: string) => void = () => {};
  export let injectContextNode: (nodeKey: string) => void = () => {};
  export let removeInjectedNode: (nodeKey: string) => void = () => {};
  export let panelEl: HTMLDivElement | null = null;
</script>

<div class="compose-context-panel compose-context-popover" bind:this={panelEl} aria-label="context injector">
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

<style>
  .compose-context-panel {
    margin-bottom: 2px;
    padding: 8px 9px;
    border-radius: 12px;
    border: 0.5px solid rgba(132, 175, 212, 0.3);
    background:
      radial-gradient(circle at 0% 0%, rgba(94, 149, 194, 0.18), rgba(0, 0, 0, 0) 52%),
      linear-gradient(170deg, rgba(37, 61, 84, 0.34), rgba(23, 37, 52, 0.28));
    display: grid;
    gap: 6px;
    box-shadow: inset 0 0 0 1px rgba(107, 149, 184, 0.14);
  }

  .compose-context-popover {
    margin-top: 2px;
    margin-bottom: 2px;
    border-color: rgba(176, 211, 239, 0.36);
    box-shadow:
      inset 0 0 0 1px rgba(129, 172, 208, 0.2),
      0 8px 26px rgba(8, 16, 24, 0.32);
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
    color: rgba(210, 231, 249, 0.88);
  }

  .compose-context-head small {
    font-size: 8px;
    color: rgba(206, 224, 242, 0.74);
  }

  .compose-context-sessions {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-bottom: 0;
  }

  .compose-context-session {
    border: 0.5px solid rgba(142, 183, 216, 0.32);
    background: linear-gradient(180deg, rgba(88, 126, 160, 0.2), rgba(63, 94, 124, 0.16));
    color: rgba(213, 232, 248, 0.9);
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

  .compose-context-session:hover {
    border-color: rgba(175, 210, 239, 0.5);
    background: linear-gradient(180deg, rgba(107, 149, 186, 0.32), rgba(73, 108, 139, 0.24));
  }

  .compose-context-session.origin {
    border-color: rgba(205, 176, 116, 0.44);
    background: linear-gradient(160deg, rgba(178, 140, 80, 0.32), rgba(132, 100, 54, 0.24));
    color: rgba(242, 223, 186, 0.96);
  }

  .compose-context-session.selected {
    border-color: rgba(204, 231, 255, 0.58);
    background: linear-gradient(160deg, rgba(118, 173, 215, 0.42), rgba(80, 128, 168, 0.3));
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
    border: 0.5px solid rgba(156, 188, 214, 0.3);
    border-radius: 7px;
    padding: 6px;
    background: linear-gradient(170deg, rgba(58, 87, 114, 0.26), rgba(33, 55, 74, 0.22));
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
    border: 0.5px solid rgba(178, 213, 240, 0.44);
    background: linear-gradient(180deg, rgba(110, 161, 204, 0.33), rgba(82, 124, 160, 0.24));
    color: rgba(239, 248, 255, 0.96);
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
    background: linear-gradient(160deg, rgba(198, 167, 105, 0.24), rgba(143, 112, 58, 0.19));
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

  .drawer-error {
    font-size: 10px;
    color: rgba(233, 148, 58, 0.88);
    margin: 6px 0 0;
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
