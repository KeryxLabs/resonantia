<script lang="ts">
  import { createEventDispatcher } from 'svelte';

  export let faded = false;
  export let hidden = false;

  const dispatch = createEventDispatcher<{
    open: void;
  }>();
</script>

<div class="compose-launch" class:faded={faded} class:hidden={hidden}>
  <button class="compose-btn" data-tour-target="compose-live" on:click={() => dispatch('open')}>+ compose</button>
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
</style>
