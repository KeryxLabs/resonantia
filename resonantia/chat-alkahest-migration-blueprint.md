# Chat + Alkahest Migration Blueprint

## Goal

Promote live chat to a dedicated first-class surface and move importare responsibilities into Alkahest, while preserving existing behaviors:

- cross-session continue routing
- context injection from node cards
- token threshold pacing and auto-encode controls
- STTP store/validation path fidelity

## Why This Split

- Chat and importare have different jobs and interaction rhythms.
- Alkahest already owns ritualized transformation and storage flows.
- Camera integration becomes cleaner when each surface has one primary purpose.

## Current Wiring Snapshot

- Chat + importare mode switch lives in `Weaver.svelte` via `composeMode` and `openCompose('live' | 'importare')`.
- Launcher menu currently exposes both `create live` and `importare` via `ComposeLauncher.svelte`.
- Importare save path is currently handled by compose paste mode (`composePasteNodeDraft` + `saveComposePastedNode`).
- Alkahest has a staged flow (`scan -> configure -> run`) and already performs distill/store/export behavior.
- Walkthrough expects `importare` on compose launcher target (`compose-importare`).

## Target Product Map

- Telescope: observe and traverse memory.
- Alkahest: import, distill, export, store.
- Chat: live conversational threading, context routing, encode/save continuation.

## Migration Phases

### Phase 1: Surface Decoupling (No Behavior Change)

1. Keep existing compose internals, but rename user-facing terminology from compose to chat where visible.
2. Keep all chat behavior intact (tabs, context inject, auto-encode, provider usage, threshold UI).
3. Restrict launcher to one primary chat action and remove importare from launcher menu.

Deliverables:

- chat-only launcher action
- no importare entrypoint in launcher
- all live thread behaviors preserved

Acceptance:

- continue-in-app from node card still opens/targets chat thread
- context browse/inject still works
- auto-encode still triggers at configured threshold

### Phase 2: Move Importare into Alkahest

1. Add an explicit Alkahest mode for direct node import (paste + validate + store).
2. Reuse existing store pipeline semantics (`storeContext` validity + status handling).
3. Keep import path independent from chat state so import does not mutate active chat thread.

Deliverables:

- import UI lives in Alkahest panel flow
- import success/failure messaging aligned with existing Alkahest status style
- no importare branch required inside chat surface

Acceptance:

- pasted node save works from Alkahest with same validation semantics
- duplicate/upsert status surfaced
- graph refresh behavior matches existing store flows

### Phase 3: Remove Legacy Compose Import Branch

1. Remove `importare` mode branching from chat container.
2. Remove compose paste panel wiring from chat state.
3. Keep encode-save-continue flow as chat-native action.

Deliverables:

- chat surface is single-purpose (live)
- dead state and props removed

Acceptance:

- chat compile and runtime remain clean
- touched-file diagnostics clean

### Phase 4: Walkthrough + Target Mapping Update

1. Replace walkthrough step target for import with Alkahest import target.
2. Update allowlist selectors to match new interaction path.
3. Keep tutorial sequence intact: telescope -> alkahest -> import -> live chat.

Deliverables:

- walkthrough uses Alkahest import selector
- no compose-importare dependency remains

Acceptance:

- walkthrough progression remains satisfiable end-to-end

### Phase 5: Camera-Mode Readiness for Chat

1. Introduce chat camera enter/exit state hooks mirroring telescope/alkahest pattern.
2. Do not change default camera behavior yet; create opt-in scaffolding only.
3. Ensure chat overlay uses existing `cameraOverlayEngaged` safety gating.

Deliverables:

- chat camera transition primitives added
- no regression in pan/zoom or overlay exclusivity

Acceptance:

- camera state returns correctly after opening/closing chat mode

## Implementation Order (Recommended)

1. Phase 1 and Phase 2 behind a small feature flag if needed.
2. Phase 3 cleanup immediately after Phase 2 validation.
3. Phase 4 walkthrough target migration.
4. Phase 5 camera scaffolding.

## Regression Checklist

- node card `continueInApp` route behavior
- cross-session routing preference read/persist/reset
- chat token usage + provider usage tracking
- auto-encode threshold persistence
- Alkahest distill/export behavior untouched
- import save path validation and upsert reporting
- walkthrough step flow and target selectors

## Risks and Mitigations

- Risk: hidden coupling between import and chat paste state.
  - Mitigation: migrate store logic into shared helper and call from Alkahest import action.
- Risk: walkthrough selectors break after launcher changes.
  - Mitigation: update selector map and run scripted walkthrough pass.
- Risk: camera overlay conflicts with new chat mode.
  - Mitigation: follow existing telescope/alkahest transition pattern and epsilon settle logic.

## Definition of Done

- Importare fully lives in Alkahest.
- Chat is a dedicated live thread surface.
- Walkthrough reflects new flow.
- Diagnostics for touched files are clean.
- STTP checkpoint stored with migration status and next execution slice.