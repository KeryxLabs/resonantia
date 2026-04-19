<script lang="ts">
  import type { ComposeTabInfo } from "./types";

  export let tabs: ComposeTabInfo[] = [];
  export let activeTabId = "";
  export let maxTabs = 3;
  export let selectComposeTab: (tabId: string) => void = () => {};
  export let createComposeTab: () => void = () => {};
  export let closeComposeTab: (tabId: string) => void = () => {};
</script>

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

<style>
  .compose-tabs {
    display: flex;
    flex-wrap: nowrap;
    gap: 5px;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .compose-tabs::-webkit-scrollbar {
    display: none;
  }

  .compose-tab {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    border-radius: 20px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    background: transparent;
    padding: 1px 2px;
    flex-shrink: 0;
  }

  .compose-tab.active {
    border-color: rgba(100, 190, 170, 0.34);
    background: rgba(100, 190, 170, 0.08);
  }

  .compose-tab-btn,
  .compose-tab-close,
  .compose-tab-add {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.3);
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.03em;
    text-transform: lowercase;
    cursor: pointer;
  }

  .compose-tab-btn {
    padding: 2px 9px;
    white-space: nowrap;
    max-width: 148px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .compose-tab-close {
    padding: 2px 4px;
    color: rgba(255, 255, 255, 0.24);
  }

  .compose-tab.active .compose-tab-btn {
    color: rgba(100, 190, 170, 0.82);
  }

  .compose-tab-add {
    border-radius: 20px;
    border: 0.5px dashed rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.2);
    padding: 3px 10px;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .compose-tab-add:hover {
    border-color: rgba(100, 190, 170, 0.32);
    color: rgba(100, 190, 170, 0.78);
  }
</style>
