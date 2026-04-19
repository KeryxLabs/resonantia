<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let faded = false;
  export let hidden = false;
  export let menuOpen = false;

  const dispatch = createEventDispatcher<{
    toggle: void;
    live: void;
  }>();
</script>

<div class="compose-launch" class:faded={faded} class:hidden={hidden}>
  {#if menuOpen}
    <div class="compose-launch-popover" role="menu" aria-label="compose options">
      <button class="compose-launch-item" data-tour-target="compose-live" on:click={() => dispatch('live')}>create live</button>
    </div>
  {/if}
  <button class="compose-btn" data-tour-target="compose-toggle" class:open={menuOpen} on:click={() => dispatch('toggle')}>+ compose</button>
</div>

<style>
  .compose-launch {
    position: absolute;
    bottom: max(24px, calc(env(safe-area-inset-bottom, 0px) + 12px));
    right: max(24px, calc(env(safe-area-inset-right, 0px) + 12px));
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    z-index: 10;
    transition: all 0.2s;
  }

  .compose-launch.faded {
    opacity: 0;
    transform: translateY(8px);
    pointer-events: none;
  }

  .compose-launch.hidden {
    opacity: 0;
    transform: translateY(8px);
    pointer-events: none;
  }

  .compose-launch-popover {
    width: 146px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: rgba(10, 11, 14, 0.95);
    border: 0.5px solid rgba(255, 255, 255, 0.12);
    border-radius: 10px;
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.26);
  }

  .compose-launch-item {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    text-align: left;
    color: rgba(255, 255, 255, 0.62);
    background: transparent;
    border: 0.5px solid transparent;
    border-radius: 6px;
    padding: 8px 10px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s;
  }

  .compose-launch-item:hover {
    color: rgba(255, 255, 255, 0.86);
    border-color: rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
  }

  .compose-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.6);
    background: rgba(14, 16, 22, 0.88);
    border: 0.5px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 10px 18px;
    cursor: pointer;
    transition: all 0.2s;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
  }

  .compose-btn:hover {
    color: #fff;
    border-color: rgba(255, 255, 255, 0.28);
    background: rgba(20, 23, 32, 0.95);
  }

  .compose-btn.open {
    color: rgba(255, 255, 255, 0.86);
    border-color: rgba(255, 255, 255, 0.3);
    background: rgba(20, 23, 32, 0.95);
  }
</style>
