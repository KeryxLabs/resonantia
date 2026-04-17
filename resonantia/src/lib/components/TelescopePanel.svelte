<script lang="ts">
  import type { GraphSessionDto } from '@resonantia/core';

  export let telescopeCameraEngaged = false;
  export let level = 0;
  export let telescopeCanAccess = false;
  export let negativeLayerActive = false;
  export let telescopeOpen = false;
  export let telescopePhase: 'idle' | 'entering' | 'exiting' = 'idle';
  export let telescopeDialOffsetY = 0;
  export let telescopeRangeLabel = '';
  export let telescopeTimelineSessions: GraphSessionDto[] = [];

  export let toggleNegativeLayer: () => void = () => {};
  export let openTelescope: () => void = () => {};
  export let handleTelescopeClosePointerDown: (event: PointerEvent) => void = () => {};
  export let handleTelescopeDialMouseDown: (event: MouseEvent) => void = () => {};
  export let handleTelescopeDialTouchStart: (event: TouchEvent) => void = () => {};
  export let handleTelescopeDialKeydown: (event: KeyboardEvent) => void = () => {};
  export let handleTelescopeEyeKeydown: (event: KeyboardEvent) => void = () => {};
  export let selectTelescopeSession: (session: GraphSessionDto) => void = () => {};
  export let renameTelescopeSession: (session: GraphSessionDto) => void = () => {};
  export let telescopeSessionColor: (session: GraphSessionDto, alpha?: number) => string = () => 'rgba(255,255,255,0.6)';
  export let telescopeSessionTitle: (session: GraphSessionDto) => string = (session) => session.label;
  export let telescopeSessionMeta: (session: GraphSessionDto) => string = () => '';
  export let telescopeSessionDateLabel: (timestamp: string) => string = () => '';
  export let shortLabel: (text: string, words: number) => string = (text) => text;
</script>

<div class="telescope-shell" aria-label="timeline telescope">
  <button
    class="negative-layer-btn"
    class:active={negativeLayerActive}
    class:hidden={telescopeCameraEngaged || level !== 0}
    on:click={toggleNegativeLayer}
    aria-label={negativeLayerActive ? 'return to constellation scale' : 'zoom out to negative layer'}
  >
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="rgba(178,208,255,0.86)" stroke-width="1.55" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="8"></circle>
      <line x1="8.5" y1="12" x2="15.5" y2="12"></line>
    </svg>
  </button>
  <button
    class="telescope-icon"
    data-tour-target="telescope"
    class:hidden={telescopeCameraEngaged || !telescopeCanAccess}
    on:click={openTelescope}
    aria-label="open timeline telescope"
  >
    <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="rgba(190,170,255,0.85)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="11" cy="11" r="4"></circle>
      <line x1="19" y1="19" x2="14.65" y2="14.65"></line>
      <line x1="11" y1="7" x2="11" y2="5"></line>
      <line x1="11" y1="17" x2="11" y2="19"></line>
      <line x1="7" y1="11" x2="5" y2="11"></line>
      <line x1="15" y1="11" x2="17" y2="11"></line>
    </svg>
  </button>
</div>

<div
  class="telescope-stage"
  class:visible={telescopeCameraEngaged}
  class:closing={telescopePhase === 'exiting'}
  aria-hidden={!telescopeCameraEngaged}
>
  {#if telescopeOpen}
    <button class="telescope-backdrop" on:pointerdown={handleTelescopeClosePointerDown} aria-label="close timeline telescope"></button>
  {/if}
  <div
    class="telescope-instrument"
    class:open={telescopeOpen || telescopePhase === 'exiting'}
    class:interactive={telescopeOpen}
    class:closing={telescopePhase === 'exiting'}
  >
    <div class="telescope-scope-wrap">
      <svg width="58" height="300" viewBox="0 0 52 270" fill="none" aria-hidden="true">
        <ellipse cx="26" cy="14" rx="15" ry="4" fill="#141025" stroke="rgba(160,140,255,0.4)" stroke-width="0.8"></ellipse>
        <ellipse cx="26" cy="14" rx="10" ry="2.5" fill="#0a0818" stroke="rgba(160,140,255,0.5)" stroke-width="0.5"></ellipse>
        <ellipse cx="24.5" cy="13.2" rx="3.5" ry="1" fill="rgba(200,185,255,0.12)"></ellipse>

        <path d="M11 14 L15 118 L37 118 L41 14 Z" fill="#0f0d1e" stroke="rgba(120,100,190,0.2)" stroke-width="0.5"></path>
        <path d="M11 14 L15 118" stroke="rgba(170,150,255,0.1)" stroke-width="1.2"></path>
        <path d="M41 14 L37 118" stroke="rgba(60,45,110,0.2)" stroke-width="0.8"></path>

        <g
          class="telescope-dial"
          style={`transform: translateY(${telescopeDialOffsetY.toFixed(2)}px);`}
          on:mousedown={handleTelescopeDialMouseDown}
          on:touchstart={handleTelescopeDialTouchStart}
          on:keydown={handleTelescopeDialKeydown}
          role="button"
          tabindex="0"
          aria-label="adjust timeline zoom"
        >
          <path d="M13 96 L15 118 L37 118 L39 96 Z" fill="#1c1840" stroke="rgba(155,135,255,0.4)" stroke-width="0.5"></path>
          <line x1="13" y1="96" x2="39" y2="96" stroke="rgba(160,140,255,0.3)" stroke-width="0.7"></line>
          <line x1="15" y1="118" x2="37" y2="118" stroke="rgba(160,140,255,0.3)" stroke-width="0.7"></line>
          <line x1="23" y1="97" x2="22.5" y2="117" stroke="rgba(180,160,255,0.2)" stroke-width="0.6"></line>
          <line x1="26" y1="97" x2="26" y2="117" stroke="rgba(190,170,255,0.4)" stroke-width="0.9"></line>
          <line x1="29" y1="97" x2="29.5" y2="117" stroke="rgba(180,160,255,0.2)" stroke-width="0.6"></line>
          <circle cx="26" cy="107" r="2" fill="rgba(210,190,255,0.75)"></circle>
        </g>

        <path d="M15 118 L17 178 L35 178 L37 118 Z" fill="#0c0a1a" stroke="rgba(100,85,165,0.18)" stroke-width="0.5"></path>

        <path d="M16 175 L17 190 L35 190 L36 175 Z" fill="#181535" stroke="rgba(140,120,215,0.3)" stroke-width="0.5"></path>
        <line x1="16" y1="175" x2="36" y2="175" stroke="rgba(145,125,215,0.25)" stroke-width="0.7"></line>
        <line x1="17" y1="190" x2="35" y2="190" stroke="rgba(145,125,215,0.25)" stroke-width="0.7"></line>

        <path d="M17 190 L19 232 L33 232 L35 190 Z" fill="#090716" stroke="rgba(100,85,155,0.18)" stroke-width="0.5"></path>
        <path d="M18 230 L15 244 L37 244 L34 230 Z" fill="#13102a" stroke="rgba(130,110,200,0.3)" stroke-width="0.5"></path>
        <ellipse cx="26" cy="232" rx="9" ry="2" fill="#181535" stroke="rgba(140,120,205,0.3)" stroke-width="0.5"></ellipse>
        <ellipse cx="26" cy="244" rx="11" ry="2.5" fill="#1c193a" stroke="rgba(150,130,215,0.35)" stroke-width="0.5"></ellipse>

        <line x1="26" y1="246" x2="16" y2="256" stroke="rgba(100,85,155,0.3)" stroke-width="0.8"></line>
        <line x1="26" y1="246" x2="36" y2="256" stroke="rgba(100,85,155,0.3)" stroke-width="0.8"></line>
        <ellipse cx="16" cy="256" rx="2.5" ry="0.8" fill="rgba(100,85,155,0.25)"></ellipse>
        <ellipse cx="36" cy="256" rx="2.5" ry="0.8" fill="rgba(100,85,155,0.25)"></ellipse>

        <ellipse
          cx="26"
          cy="244"
          rx="13"
          ry="4"
          fill="transparent"
          class="telescope-eye-btn"
          on:pointerdown={handleTelescopeClosePointerDown}
          on:keydown={handleTelescopeEyeKeydown}
          role="button"
          tabindex="0"
          aria-label="close timeline telescope"
        ></ellipse>
      </svg>
    </div>

    <div class="telescope-timeline-layer">
      <span class="telescope-range-badge">{telescopeRangeLabel}</span>
      <div class="telescope-line"></div>
      <div class="telescope-sessions">
        {#if telescopeTimelineSessions.length === 0}
          <p class="telescope-empty">no sessions in this range</p>
        {:else}
          {#each telescopeTimelineSessions as session}
            <div
              class="telescope-item"
              on:click={() => selectTelescopeSession(session)}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  selectTelescopeSession(session);
                }
              }}
              role="button"
              tabindex="0"
              title={telescopeSessionTitle(session)}
            >
              <i
                class="telescope-dot"
                style={`background:${telescopeSessionColor(session)}; box-shadow: 0 0 7px ${telescopeSessionColor(session, 0.4)};`}
              ></i>
              <span class="telescope-copy">
                <span class="telescope-title">{shortLabel(telescopeSessionTitle(session), 4)}</span>
                <span class="telescope-meta">{telescopeSessionMeta(session)}</span>
              </span>
              <button
                class="telescope-rename"
                type="button"
                title="rename session"
                aria-label={`rename ${telescopeSessionTitle(session)}`}
                on:click|stopPropagation={() => renameTelescopeSession(session)}
              >
                rename
              </button>
              <span class="telescope-date">{telescopeSessionDateLabel(session.lastModified)}</span>
            </div>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</div>

<style>
  .telescope-shell {
    position: absolute;
    left: max(20px, calc(var(--safe-left) + 12px));
    bottom: max(20px, calc(var(--safe-bottom) + 12px));
    z-index: 16;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 7px;
    width: 62px;
    pointer-events: none;
  }

  .negative-layer-btn {
    width: 31px;
    height: 31px;
    border-radius: 999px;
    background: rgba(12, 15, 22, 0.9);
    border: 0.5px solid rgba(132, 172, 226, 0.32);
    display: flex;
    align-items: center;
    justify-content: center;
    margin-left: 33px;
    padding: 0;
    cursor: pointer;
    pointer-events: all;
    transition: border-color 0.2s, background 0.2s, opacity 0.16s, transform 0.16s;
  }

  .negative-layer-btn:hover {
    border-color: rgba(172, 206, 255, 0.58);
    background: rgba(30, 48, 78, 0.58);
  }

  .negative-layer-btn.active {
    border-color: rgba(188, 220, 255, 0.72);
    background: rgba(52, 83, 130, 0.54);
    box-shadow: 0 0 14px rgba(74, 130, 208, 0.2);
  }

  .negative-layer-btn.hidden {
    opacity: 0;
    pointer-events: none;
    transform: translateY(3px);
  }

  .telescope-stage {
    --telescope-open-scale: 1.28;
    --telescope-enter-scale: 1.28;
    position: absolute;
    inset: 0;
    z-index: 16;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    opacity: 0;
    transition: opacity 0.24s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .telescope-stage.visible {
    opacity: 1;
    pointer-events: all;
  }

  .telescope-stage.visible.closing {
    opacity: 0.82;
    transition-duration: 0.44s;
  }

  .telescope-backdrop {
    position: absolute;
    inset: 0;
    border: none;
    margin: 0;
    padding: 0;
    background: transparent;
    cursor: default;
    z-index: 1;
  }

  .telescope-backdrop:focus {
    outline: none;
  }

  .telescope-icon {
    width: 38px;
    height: 38px;
    border-radius: 999px;
    background: rgba(14, 10, 30, 0.9);
    border: 0.5px solid rgba(160, 140, 255, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    pointer-events: all;
    margin-left: 5px;
    margin-top: -2px;
    transition: border-color 0.2s, background 0.2s, opacity 0.16s;
    padding: 0;
  }

  .telescope-icon:hover {
    border-color: rgba(180, 160, 255, 0.62);
    background: rgba(62, 42, 122, 0.64);
  }

  .telescope-icon.hidden {
    opacity: 0;
    pointer-events: none;
  }

  .telescope-instrument {
    position: relative;
    z-index: 2;
    display: flex;
    align-items: flex-end;
    gap: 14px;
    opacity: 0;
    pointer-events: none;
    transform: scale(var(--telescope-open-scale)) translateY(0);
    transform-origin: 34% 84%;
    transition: opacity 0.2s cubic-bezier(0.22, 1, 0.36, 1), transform 0.2s cubic-bezier(0.22, 1, 0.36, 1);
  }

  .telescope-instrument.open {
    opacity: 1;
    transform: scale(var(--telescope-open-scale)) translateY(0);
  }

  .telescope-instrument.open.closing {
    opacity: 0.78;
    transform: scale(calc(var(--telescope-open-scale) * 0.985)) translateY(2px);
    transition-duration: 0.44s, 0.5s;
  }

  .telescope-instrument.open.interactive {
    pointer-events: all;
  }

  .telescope-scope-wrap {
    position: relative;
  }

  .telescope-dial {
    cursor: ns-resize;
    transition: transform 0.08s linear;
  }

  .telescope-eye-btn {
    cursor: pointer;
  }

  .telescope-timeline-layer {
    position: relative;
    width: 214px;
    height: 300px;
    padding-top: 6px;
    padding-left: 2px;
  }

  .telescope-range-badge {
    position: absolute;
    top: -18px;
    left: 2px;
    font-size: 10px;
    color: rgba(140, 125, 210, 0.48);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    white-space: nowrap;
  }

  .telescope-line {
    position: absolute;
    left: 8px;
    top: 8px;
    bottom: 8px;
    width: 1px;
    background: linear-gradient(to bottom, rgba(150, 130, 255, 0), rgba(150, 130, 255, 0.26) 18%, rgba(150, 130, 255, 0.26) 82%, rgba(150, 130, 255, 0));
  }

  .telescope-sessions {
    position: absolute;
    inset: 0;
    padding: 8px 0 8px 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    justify-content: flex-start;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
    -webkit-overflow-scrolling: touch;
    padding-right: 4px;
  }

  .telescope-item {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
    align-items: center;
    gap: 7px;
    width: 100%;
    border: none;
    background: transparent;
    cursor: pointer;
    padding: 5px 0 5px 2px;
    text-align: left;
    transition: opacity 0.14s;
  }

  .telescope-item:hover .telescope-title {
    color: rgba(226, 214, 255, 0.95);
  }

  .telescope-item:hover .telescope-dot {
    transform: scale(1.35);
  }

  .telescope-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
    transition: transform 0.14s;
  }

  .telescope-copy {
    display: flex;
    flex-direction: column;
    min-width: 0;
    gap: 1px;
  }

  .telescope-title {
    font-size: 11px;
    color: rgba(180, 165, 255, 0.74);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    transition: color 0.15s;
  }

  .telescope-meta {
    font-size: 9px;
    color: rgba(155, 144, 214, 0.5);
    letter-spacing: 0.03em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .telescope-date {
    font-size: 10px;
    color: rgba(120, 110, 180, 0.42);
    white-space: nowrap;
    padding-right: 2px;
  }

  .telescope-rename {
    border: 0.5px solid rgba(167, 193, 255, 0.28);
    background: rgba(70, 83, 121, 0.22);
    border-radius: 999px;
    color: rgba(205, 220, 255, 0.9);
    font-size: 8px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    padding: 3px 7px;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }

  .telescope-rename:hover {
    border-color: rgba(198, 216, 255, 0.56);
    background: rgba(88, 104, 150, 0.34);
  }

  .telescope-empty {
    margin: auto 0;
    padding-left: 17px;
    font-size: 10px;
    letter-spacing: 0.06em;
    color: rgba(170, 156, 224, 0.44);
    text-transform: lowercase;
  }

  @media (max-width: 520px) {
    .telescope-shell {
      left: max(12px, calc(var(--safe-left) + 10px));
      bottom: max(12px, calc(var(--safe-bottom) + 10px));
      width: 58px;
    }

    .negative-layer-btn {
      margin-left: 27px;
    }

    .telescope-icon {
      margin-left: 4px;
      margin-top: -1px;
    }

    .telescope-stage {
      --telescope-open-scale: 1.12;
      --telescope-enter-scale: 1.12;
    }

    .telescope-instrument {
      transform-origin: 36% 86%;
    }

    .telescope-timeline-layer {
      width: 188px;
      height: 278px;
    }
  }
</style>
