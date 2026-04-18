<script lang="ts">
  import { AVEC_HEX } from '@resonantia/core';

  type CalibrationVector = {
    stability: number;
    friction: number;
    logic: number;
    autonomy: number;
  };

  type CalibrationProfile = {
    id: string;
    label: string;
    blurb: string;
    trigger: string;
    values: CalibrationVector;
  };

  type CalibrationQuestionOption = {
    label: string;
    note: string;
    values: CalibrationVector;
  };

  type CalibrationQuestion = {
    prompt: string;
    options: CalibrationQuestionOption[];
  };

  export let open = false;
  export let sessionId = '';
  export let stability = 0.5;
  export let friction = 0.2;
  export let logic = 0.8;
  export let autonomy = 0.9;
  export let trigger = 'manual';
  export let loading = false;
  export let error: string | null = null;
  export let guideOpen = false;
  export let guideAnswers: number[] = [];
  export let calibrationPsi = 2;
  export let currentCalibrationVector: CalibrationVector = {
    stability: 0.5,
    friction: 0.5,
    logic: 0.5,
    autonomy: 0.5,
  };
  export let closestCalibrationProfile: { profile: CalibrationProfile; distance: number } | null = null;
  export let calibrationProfiles: CalibrationProfile[] = [];
  export let calibrationQuestions: CalibrationQuestion[] = [];
  export let calibrationSurfaceStyle: (values: CalibrationVector, intensity?: number) => string = () => '';
  export let calibrationSpectrumStyle: (values: CalibrationVector) => string = () => '';
  export let calibrationAuraStyle: (values: CalibrationVector) => string = () => '';
  export let applyCalibrationProfile: (profile: CalibrationProfile) => void = () => {};
  export let selectGuideAnswer: (questionIndex: number, optionIndex: number) => void = () => {};
  export let applyCalibrationGuide: () => void = () => {};
  export let submitCalibrate: () => void = () => {};
  export let onClose: () => void = () => {};
  export let onSessionInput: () => void = () => {};
</script>

{#if open}
  <div class="drawer" role="dialog" aria-label="Find your current mode">
    <div class="drawer-header">
      <span class="drawer-title">find your current mode</span>
      <button class="close-btn" on:click={onClose}>✕</button>
    </div>
    <input
      class="drawer-input"
      type="text"
      placeholder="session id (required)"
      bind:value={sessionId}
      on:input={onSessionInput}
    />
    <p class="calibration-intro">Pick the mode that feels closest, or answer a few quick questions and adjust it gently below.</p>
    <section class="calibration-panel" style={calibrationSurfaceStyle(currentCalibrationVector, 1.15)}>
      <div class="calibration-topline">
        <span class="calibration-kicker">current mode</span>
        <span class="calibration-psi">signal {calibrationPsi.toFixed(2)}</span>
      </div>
      <div class="calibration-spectrum" style={calibrationSpectrumStyle(currentCalibrationVector)}></div>
      {#if closestCalibrationProfile}
        <p class="calibration-profile-name">{closestCalibrationProfile.profile.label}</p>
        <p class="calibration-profile-blurb">{closestCalibrationProfile.profile.blurb}</p>
      {/if}
    </section>
    <div class="profile-grid">
      {#each calibrationProfiles as profile}
        <button
          class="profile-chip"
          class:selected={closestCalibrationProfile?.profile.id === profile.id}
          style={calibrationSurfaceStyle(profile.values, closestCalibrationProfile?.profile.id === profile.id ? 1.35 : 0.9)}
          on:click={() => applyCalibrationProfile(profile)}
        >
          <i class="profile-aura" aria-hidden="true" style={calibrationAuraStyle(profile.values)}></i>
          <span>{profile.label}</span>
          <small>{profile.blurb}</small>
        </button>
      {/each}
    </div>
    <div class="guide-actions">
      <button class="guide-toggle" class:open={guideOpen} on:click={() => (guideOpen = !guideOpen)}>
        {guideOpen ? 'hide questions' : 'help me find it'}
      </button>
      {#if guideOpen}
        <button class="guide-apply" on:click={applyCalibrationGuide}>use these answers</button>
      {/if}
    </div>
    {#if guideOpen}
      <section class="guide-panel">
        {#each calibrationQuestions as question, questionIndex}
          <div class="guide-question">
            <p class="guide-prompt">{question.prompt}</p>
            <div class="guide-options">
              {#each question.options as option, optionIndex}
                <button
                  class="guide-option"
                  class:selected={guideAnswers[questionIndex] === optionIndex}
                  style={calibrationSurfaceStyle(option.values, guideAnswers[questionIndex] === optionIndex ? 1.15 : 0.72)}
                  on:click={() => selectGuideAnswer(questionIndex, optionIndex)}
                >
                  <i class="guide-aura" aria-hidden="true" style={calibrationAuraStyle(option.values)}></i>
                  <span>{option.label}</span>
                  <small>{option.note}</small>
                </button>
              {/each}
            </div>
          </div>
        {/each}
      </section>
    {/if}
    <p class="calibration-subhead">fine tune</p>
    <div class="slider-row">
      <span class="slider-label" style="color:{AVEC_HEX.stability}">grounding</span>
      <input type="range" min="0" max="1" step="0.01" bind:value={stability} class="avec-slider" style="accent-color: {AVEC_HEX.stability};" />
      <span class="slider-val" style="color:{AVEC_HEX.stability}">{stability.toFixed(2)}</span>
    </div>
    <div class="slider-row">
      <span class="slider-label" style="color:{AVEC_HEX.friction}">wear</span>
      <input type="range" min="0" max="1" step="0.01" bind:value={friction} class="avec-slider" style="accent-color: {AVEC_HEX.friction};" />
      <span class="slider-val" style="color:{AVEC_HEX.friction}">{friction.toFixed(2)}</span>
    </div>
    <div class="slider-row">
      <span class="slider-label" style="color:{AVEC_HEX.logic}">clarity</span>
      <input type="range" min="0" max="1" step="0.01" bind:value={logic} class="avec-slider" style="accent-color: {AVEC_HEX.logic};" />
      <span class="slider-val" style="color:{AVEC_HEX.logic}">{logic.toFixed(2)}</span>
    </div>
    <div class="slider-row">
      <span class="slider-label" style="color:{AVEC_HEX.autonomy}">self-trust</span>
      <input type="range" min="0" max="1" step="0.01" bind:value={autonomy} class="avec-slider" style="accent-color: {AVEC_HEX.autonomy};" />
      <span class="slider-val" style="color:{AVEC_HEX.autonomy}">{autonomy.toFixed(2)}</span>
    </div>
    <p class="calibration-source">saved from {trigger.replaceAll('_', ' ')}</p>
    {#if error}<p class="drawer-error">{error}</p>{/if}
    <div class="drawer-actions">
      <button class="drawer-btn cancel" on:click={onClose}>cancel</button>
      <button class="drawer-btn submit" on:click={submitCalibrate} disabled={loading || !sessionId.trim()}>
        {loading ? 'saving…' : 'save mode'}
      </button>
    </div>
  </div>
{/if}

<style>
  .drawer {
    position: absolute;
    top: 64px;
    bottom: 84px;
    left: 50%;
    transform: translateX(-50%);
    box-sizing: border-box;
    width: min(456px, calc(100vw - 32px));
    max-height: calc(100dvh - 148px);
    overflow-y: auto;
    overflow-x: hidden;
    background: rgba(10, 11, 14, 0.97);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 20px;
    z-index: 20;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    font-family: 'Departure Mono', 'Courier New', monospace;
    overscroll-behavior: contain;
    scrollbar-width: thin;
  }

  .drawer-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 14px;
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
    margin-bottom: 10px;
    outline: none;
    transition: border-color 0.2s;
  }

  .drawer-input:focus {
    border-color: rgba(255, 255, 255, 0.25);
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
  }

  .slider-label {
    font-size: 9px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    width: 72px;
    flex-shrink: 0;
  }

  .avec-slider {
    flex: 1;
    accent-color: rgba(255, 255, 255, 0.4);
    height: 2px;
  }

  .slider-val {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.4);
    width: 32px;
    text-align: right;
    flex-shrink: 0;
  }

  .calibration-intro {
    margin: 0 0 12px;
    font-size: 10px;
    line-height: 1.55;
    color: rgba(255, 255, 255, 0.48);
  }

  .calibration-subhead {
    margin: 2px 0 10px;
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.34);
  }

  .calibration-source {
    margin: 4px 0 0;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.32);
  }

  .calibration-panel {
    margin-bottom: 12px;
    padding: 12px 13px 11px;
    border-radius: 10px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
  }

  .calibration-spectrum {
    width: 100%;
    height: 6px;
    border-radius: 999px;
    margin-bottom: 10px;
    opacity: 0.9;
  }

  .calibration-topline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .calibration-kicker,
  .calibration-psi {
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.38);
  }

  .calibration-profile-name {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-size: 20px;
    font-style: italic;
    color: rgba(255, 249, 235, 0.88);
  }

  .calibration-profile-blurb {
    margin: 4px 0 0;
    font-size: 10px;
    line-height: 1.5;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.48);
  }

  .profile-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin-bottom: 12px;
  }

  .profile-chip,
  .guide-option,
  .guide-toggle,
  .guide-apply {
    font-family: 'Departure Mono', monospace;
    cursor: pointer;
  }

  .profile-chip {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    padding: 9px 10px;
    text-align: left;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    border-radius: 9px;
    color: rgba(255, 255, 255, 0.72);
    transition: background 0.2s, border-color 0.2s, color 0.2s;
    overflow: hidden;
  }

  .profile-aura,
  .guide-aura {
    display: block;
    width: 12px;
    height: 12px;
    border-radius: 999px;
    margin-bottom: 3px;
  }

  .profile-chip span {
    font-size: 10px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }

  .profile-chip small {
    font-size: 9px;
    line-height: 1.4;
    color: rgba(255, 255, 255, 0.42);
  }

  .profile-chip:hover,
  .profile-chip.selected {
    border-color: rgba(214, 184, 109, 0.34);
    color: rgba(255, 248, 233, 0.92);
    transform: translateY(-1px);
  }

  .guide-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .guide-toggle,
  .guide-apply {
    padding: 7px 10px;
    border-radius: 999px;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.03);
    color: rgba(255, 255, 255, 0.58);
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    transition: background 0.2s, border-color 0.2s, color 0.2s;
  }

  .guide-toggle.open,
  .guide-toggle:hover,
  .guide-apply:hover {
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.18);
    color: rgba(255, 255, 255, 0.82);
  }

  .guide-panel {
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .guide-question {
    padding: 10px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.025);
    border: 0.5px solid rgba(255, 255, 255, 0.06);
  }

  .guide-prompt {
    margin: 0 0 8px;
    font-size: 10px;
    letter-spacing: 0.06em;
    color: rgba(255, 255, 255, 0.64);
  }

  .guide-options {
    display: grid;
    gap: 6px;
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .guide-option {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 8px 9px;
    text-align: left;
    border-radius: 8px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.68);
    transition: background 0.2s, border-color 0.2s, color 0.2s;
    overflow: hidden;
  }

  .guide-option span {
    font-size: 10px;
    letter-spacing: 0.04em;
  }

  .guide-option small {
    font-size: 9px;
    line-height: 1.35;
    color: rgba(255, 255, 255, 0.38);
  }

  .guide-option:hover,
  .guide-option.selected {
    border-color: rgba(214, 184, 109, 0.3);
    color: rgba(255, 248, 233, 0.9);
  }

  .drawer-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }

  .drawer-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    padding: 6px 14px;
    border-radius: 5px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .drawer-btn.cancel {
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.3);
  }

  .drawer-btn.submit {
    background: rgba(255, 255, 255, 0.06);
    border: 0.5px solid rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.8);
  }

  .drawer-btn.submit:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.35);
  }

  .drawer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .drawer-error {
    font-size: 10px;
    color: rgba(233, 148, 58, 0.8);
    margin: 6px 0 0;
  }

  @media (hover: none) and (pointer: coarse) {
    .drawer-input {
      font-size: 16px;
      line-height: 1.35;
    }
  }

  @media (max-width: 520px) {
    .drawer {
      top: calc(env(safe-area-inset-top, 0px) + 56px);
      bottom: max(74px, calc(env(safe-area-inset-bottom, 0px) + 58px));
      width: calc(100vw - 20px);
      max-height: calc(100dvh - 130px);
      padding: 16px;
    }

    .profile-grid,
    .guide-options {
      grid-template-columns: 1fr;
    }
  }
</style>
