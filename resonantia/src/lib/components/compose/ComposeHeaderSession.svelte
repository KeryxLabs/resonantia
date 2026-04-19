<script lang="ts">
  export let sessionId = "";
  export let onSessionInput: () => void = () => {};
  export let onClose: () => void = () => {};
</script>

<div class="drawer-header">
  <span class="drawer-title">compose</span>
  <button class="close-btn" on:click={onClose}>✕</button>
</div>
<input
  class="drawer-input drawer-session-input"
  type="text"
  placeholder="session id"
  bind:value={sessionId}
  on:input={onSessionInput}
/>

<style>
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

  @media (max-width: 520px) {
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
  }
</style>
