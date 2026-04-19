<script lang="ts">
  import { formatTimestamp } from "@resonantia/core";
  import type { ComposeContextNode, ComposeInjectedNode } from "./types";

  export let sessionId = "";
  export let contextNodes: ComposeContextNode[] = [];
  export let contextNodesLoading = false;
  export let contextNodesError: string | null = null;
  export let injectedNodes: ComposeInjectedNode[] = [];
  export let injectContextNode: (nodeKey: string) => void = () => {};
  export let removeInjectedNode: (nodeKey: string) => void = () => {};
  export let popoverEl: HTMLDivElement | null = null;
</script>

<div class="compose-session-nodes-popover" bind:this={popoverEl} role="dialog" aria-label="session raw nodes">
  <div class="compose-session-nodes-head">
    <span>session node context</span>
    <small>{injectedNodes.length} injected</small>
  </div>

  {#if !sessionId.trim()}
    <p class="compose-session-nodes-note">choose a session chip to browse raw nodes.</p>
  {:else if contextNodesLoading}
    <p class="compose-session-nodes-note">loading session nodes...</p>
  {:else if contextNodesError}
    <p class="compose-session-nodes-error">{contextNodesError}</p>
  {:else if contextNodes.length > 0}
    <div class="compose-session-nodes-list">
      {#each contextNodes as node}
        <article class="compose-session-node-item">
          <div class="compose-session-node-badges">
            <span class="compose-session-node-badge tier">tier {node.tier}</span>
            <span class="compose-session-node-badge score">checksum Ψ {node.psi.toFixed(2)}</span>
          </div>

          <div class="compose-session-node-row">
            <span class="compose-session-node-date">{formatTimestamp(node.timestamp)}</span>
            <p class="compose-session-node-summary">{node.preview || 'summary unavailable'}</p>
            <button class="compose-session-node-inject" on:click={() => injectContextNode(node.key)}>inject</button>
          </div>
        </article>
      {/each}
    </div>
  {:else}
    <p class="compose-session-nodes-note">no raw nodes found for this session yet.</p>
  {/if}

  {#if injectedNodes.length > 0}
    <div class="compose-session-nodes-injected-strip">
      {#each injectedNodes as node}
        <button class="compose-session-nodes-injected-chip" on:click={() => removeInjectedNode(node.key)}>
          x {node.title}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  :root {
    --avec-context: rgba(100, 190, 170, 0.88);
    --avec-stability: rgba(100, 160, 220, 0.88);
    --avec-friction: rgba(200, 130, 80, 0.88);
    --avec-logic: rgba(160, 120, 220, 0.88);
    --avec-autonomy: rgba(100, 190, 120, 0.88);
  }

  .compose-session-nodes-popover {
    border-radius: 12px;
    border: 0.5px solid rgba(100, 190, 170, 0.28);
    background:
      radial-gradient(circle at 14% 8%, rgba(100, 190, 170, 0.14), rgba(0, 0, 0, 0) 48%),
      radial-gradient(circle at 86% 0%, rgba(100, 160, 220, 0.1), rgba(0, 0, 0, 0) 40%),
      linear-gradient(170deg, rgba(13, 20, 31, 0.96), rgba(10, 16, 26, 0.95));
    box-shadow:
      inset 0 0 0 1px rgba(90, 138, 161, 0.18),
      0 14px 30px rgba(5, 10, 18, 0.52);
    padding: 8px;
    display: grid;
    gap: 6px;
    max-height: 272px;
    overflow: hidden;
    animation: composeSessionPopoverIn 0.16s ease;
  }

  .compose-session-nodes-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    font-size: 8px;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: rgba(196, 222, 241, 0.84);
  }

  .compose-session-nodes-head small {
    font-size: 8px;
    letter-spacing: 0.07em;
    color: rgba(188, 207, 229, 0.62);
  }

  .compose-session-nodes-note,
  .compose-session-nodes-error {
    margin: 0;
    font-size: 8px;
    letter-spacing: 0.03em;
    text-transform: lowercase;
  }

  .compose-session-nodes-note {
    color: rgba(197, 220, 240, 0.74);
  }

  .compose-session-nodes-error {
    color: rgba(233, 148, 58, 0.9);
  }

  .compose-session-nodes-list {
    max-height: 164px;
    overflow-y: auto;
    display: grid;
    gap: 5px;
    padding-right: 2px;
  }

  .compose-session-node-item {
    display: grid;
    gap: 3px;
    border-radius: 8px;
    border: 0.5px solid rgba(124, 172, 205, 0.24);
    background: linear-gradient(170deg, rgba(36, 54, 74, 0.44), rgba(24, 38, 54, 0.42));
    padding: 6px;
  }

  .compose-session-node-badges {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .compose-session-node-badge {
    border-radius: 999px;
    border: 0.5px solid rgba(173, 206, 233, 0.3);
    background: rgba(67, 92, 116, 0.28);
    color: rgba(218, 233, 248, 0.86);
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.05em;
    text-transform: lowercase;
    padding: 1px 5px;
    white-space: nowrap;
    line-height: 1.25;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .compose-session-node-badge::before {
    content: '';
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: rgba(196, 220, 243, 0.74);
  }

  .compose-session-node-badge.tier {
    border-color: rgba(100, 160, 220, 0.38);
    background: rgba(62, 98, 138, 0.26);
    color: rgba(201, 224, 247, 0.9);
  }

  .compose-session-node-badge.tier::before {
    background: var(--avec-stability);
  }

  .compose-session-node-badge.score {
    border-color: rgba(160, 120, 220, 0.38);
    background: rgba(90, 72, 132, 0.24);
    color: rgba(225, 214, 248, 0.9);
  }

  .compose-session-node-badge.score::before {
    background: var(--avec-logic);
  }

  .compose-session-node-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .compose-session-node-date {
    font-size: 8px;
    letter-spacing: 0.03em;
    color: rgba(198, 162, 112, 0.8);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .compose-session-node-summary {
    margin: 0;
    font-size: 8px;
    line-height: 1.25;
    color: rgba(195, 214, 232, 0.84);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }

  .compose-session-node-inject {
    border: 0.5px solid rgba(100, 190, 120, 0.4);
    background: rgba(62, 126, 78, 0.24);
    color: rgba(208, 238, 214, 0.9);
    border-radius: 999px;
    font-family: 'Departure Mono', monospace;
    font-size: 6px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    line-height: 1.25;
    padding: 2px 5px;
    cursor: pointer;
    flex-shrink: 0;
    transition: border-color 0.2s ease, background 0.2s ease, color 0.2s ease;
  }

  .compose-session-node-inject:hover {
    border-color: rgba(126, 214, 148, 0.64);
    background: rgba(72, 155, 95, 0.34);
    color: rgba(228, 249, 233, 0.97);
  }

  .compose-session-nodes-injected-strip {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .compose-session-nodes-injected-chip {
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

  @keyframes composeSessionPopoverIn {
    from {
      opacity: 0;
      transform: translateX(-6px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
</style>
