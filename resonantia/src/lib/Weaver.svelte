<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { avecColor, avecToRgb, shortLabel, AVEC_HEX, AVEC_COLORS } from '@resonantia/core';
  import CollapseCard from '@resonantia/ui/components/CollapseCard.svelte';
  import TourActionMenu from './components/TourActionMenu.svelte';
  import ComposeLauncher from './components/ComposeLauncher.svelte';
  import ComposeDrawer from './components/ComposeDrawer.svelte';
  import SyncCloudStatus from './components/SyncCloudStatus.svelte';
  import TelescopePanel from './components/TelescopePanel.svelte';
  import CalibrateDrawer from './components/CalibrateDrawer.svelte';
  import SettingsDrawer from './components/SettingsDrawer.svelte';
  import WalkthroughGuide from './components/WalkthroughGuide.svelte';
  import AdventureOnboarding from './components/AdventureOnboarding.svelte';
  import { getCloudAuthStatus, getGatewayAuthToken, signOutCloud, redirectToCloudSignIn, getCloudAccount } from './cloudAuth';
  import { getGatewayBaseUrl as getManagedGatewayBaseUrl } from './config';
  import { resonantiaClient } from './resonantiaClient';
  import type {
    AiSummary,
    AvecState,
    ChatMessage,
    GraphResponse,
    GraphSessionDto,
    GraphNodeDto,
    HealthResponse,
    ListNodesResponse,
    NodeDto,
    StoreContextResponse,
    SyncNowResponse,
    CollapseCardData,
    Vec2,
  } from '@resonantia/core';

  const FONT_MONO    = "'Departure Mono', 'Courier New', monospace";
  const FONT_DISPLAY = "'Fraunces', Georgia, serif";

  // ── Canvas ──────────────────────────────────────────
  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let raf = 0;
  let container: HTMLDivElement;

  let viewportWidth = 800;
  let viewportHeight = 600;
  let deviceScale = 1;
  const ACTIVE_FRAME_STEP_MS = 1000 / 60;
  const IDLE_FRAME_STEP_MS = 1000 / 24;
  const RECENT_INTERACTION_WINDOW_MS = 1400;
  const CAMERA_POS_EPSILON = 0.35;
  const CAMERA_SCALE_EPSILON = 0.01;
  let lastFrameDrawAt = 0;
  let lastInteractionAt = 0;

  function W() { return viewportWidth; }
  function H() { return viewportHeight; }

  function nowMs() {
    return typeof performance !== 'undefined' ? performance.now() : Date.now();
  }

  function noteInteraction() {
    lastInteractionAt = nowMs();
  }

  function startRenderLoop() {
    if (raf !== 0) {
      return;
    }

    raf = requestAnimationFrame(draw);
  }

  function stopRenderLoop() {
    if (raf === 0) {
      return;
    }

    cancelAnimationFrame(raf);
    raf = 0;
  }

  // ── Data ──────────────────────────────────────────────────
  let graph: GraphResponse | null = null;
  let loading = true;
  let error: string | null = null;
  let sourceBadgeLabel = 'source: pending';
  let sourceBadgeTitle = 'read path: n/a · transport: n/a';
  let sourceBadgeTone: 'unknown' | 'local' | 'cloud' | 'mem' = 'unknown';
  let lastTransportLabel = '';
  let sessionAvecMap: Record<string, { stability: number; friction: number; logic: number; autonomy: number; psi: number }> = {};
  let nodeAvecMap: Record<string, { stability: number; friction: number; logic: number; autonomy: number; psi: number }> = {};
  let sessionById = new Map<string, GraphSessionDto>();
  let selectedSessionNodes: GraphNodeDto[] = [];
  let selectedSessionTopMoments: GraphNodeDto[] = [];
  const ONBOARDING_DISMISSED_KEY = 'resonantia:onboarding-dismissed:v1';
  const ADVENTURE_COMPLETED_KEY  = 'resonantia:adventure-completed:v1';
  const ADVENTURE_SESSION_FALLBACK = 'first-user-experience-resonantia';
  const WALKTHROUGH_SESSION_SEED = 'resonantia-demo';
  let onboardingOpen = false;
  let onboardingDismissed = false;
  let onboardingHydrated = false;
  let adventureOpen = false;
  let adventureCompleted = false;
  let adventureHydrated = false;
  let hasGraphData = false;

  type WalkthroughMode = 'first-run' | 'demo';
  type WalkthroughStep = 'intro' | 'settings' | 'checkin' | 'telescope' | 'importare' | 'live' | 'complete';

  let walkthroughMode: WalkthroughMode = 'first-run';
  let walkthroughStep: WalkthroughStep = 'intro';
  let walkthroughCueVisible = false;
  let walkthroughCueTimer: ReturnType<typeof setTimeout> | null = null;
  let walkthroughAdvanceTimer: ReturnType<typeof setTimeout> | null = null;
  let walkthroughStepLocked = false;
  let walkthroughTargetSelector: string | null = null;
  let walkthroughAllowedSelectors: string[] = [];
  let walkthroughStepSatisfied = false;
  let walkthroughCompact = false;

  const AVEC_DIMS = ['stability', 'friction', 'logic', 'autonomy'] as const;
  type AvecDim = typeof AVEC_DIMS[number];

  const sessionPos: Record<string, Vec2> = {};
  const nodePos:    Record<string, Vec2> = {};

  // ── Camera ──────────────────────────────────────────────
  // Camera: the world point at screen-center, and the current scale
  let camX = 0, camY = 0, camScale = 1;
  let targetCamX = 0, targetCamY = 0, targetCamScale = 1;

  const CONSTELLATION_SCALE = 1.2;
  const WAVE_SCALE     = 3.1;
  const COLLAPSE_SCALE = 10.2;
  const LERP           = 0.09;

  function compactViewport() {
    return W() <= 520;
  }

  function mediumViewport() {
    return W() <= 860;
  }

  function constellationCameraScale() {
    if (compactViewport()) return 1.02;
    if (mediumViewport()) return 1.1;
    return CONSTELLATION_SCALE;
  }

  function collapseCameraScale() {
    if (compactViewport()) return 8.95;
    if (mediumViewport()) return 9.6;
    return COLLAPSE_SCALE;
  }

  function negativeLayerScale() {
    if (compactViewport()) return 0.74;
    if (mediumViewport()) return 0.84;
    return 0.92;
  }

  function constellationLayerScale() {
    return negativeLayerActive ? negativeLayerScale() : constellationCameraScale();
  }

  // ── Level state machine ────────────────────────────────────────
  // 0 = constellation  1 = wave (session)  2 = collapse (node/moment)
  let level = 0;
  let negativeLayerActive = false;
  let selectedSession: GraphSessionDto | null = null;
  let selectedNode:    GraphNodeDto    | null = null;
  let cardData:    CollapseCardData | null = null;
  let cardVisible  = false;
  let transmuting = false;
  let transmuteError: string | null = null;
  let transmutationCache: Record<string, AiSummary> = {};

  $: currentTransmutation = cardData ? transmutationCache[cardData.node.syntheticId] ?? null : null;

  function matchesSelectedNode(graphNode: GraphNodeDto, dto: NodeDto) {
    return dto.syntheticId === graphNode.syntheticId;
  }

  // ── Interaction ───────────────────────────────────────────
  let dragging    = false;
  let didDrag     = false;
  let activePanPointerId: number | null = null;
  let negativeLayerDriftX = 0;
  let negativeLayerDriftY = 0;
  let dragStart:   Vec2 = { x: 0, y: 0 };
  let panCamStart: Vec2 = { x: 0, y: 0 };

  let t = 0;
  const STATIC_STARS = Array.from({ length: 80 }, () => ({
    x:     Math.random(),
    y:     Math.random(),
    s:     0.3 + Math.random() * 0.9,
    phase: Math.random() * Math.PI * 2,
  }));

  type CollapseStream = {
    side: number;
    offset: number;
    col: { r: number; g: number; b: number };
    destCol: { r: number; g: number; b: number };
    thickness: number;
    phase: number;
    speed: number;
  };

  const COLLAPSE_ICO_VERTS = (() => {
    const phi = (1 + Math.sqrt(5)) / 2;
    const raw = [
      [-1, phi, 0], [1, phi, 0], [-1, -phi, 0], [1, -phi, 0],
      [0, -1, phi], [0, 1, phi], [0, -1, -phi], [0, 1, -phi],
      [phi, 0, -1], [phi, 0, 1], [-phi, 0, -1], [-phi, 0, 1],
    ];
    return raw.map(([x, y, z]) => {
      const n = Math.sqrt(x * x + y * y + z * z);
      return [x / n, y / n, z / n] as [number, number, number];
    });
  })();

  const COLLAPSE_ICO_FACES = [
    [0,11,5],[0,5,1],[0,1,7],[0,7,10],[0,10,11],
    [1,5,9],[5,11,4],[11,10,2],[10,7,6],[7,1,8],
    [3,9,4],[3,4,2],[3,2,6],[3,6,8],[3,8,9],
    [4,9,5],[2,4,11],[6,2,10],[8,6,7],[9,8,1],
  ];

  function buildCollapseStreams(avec: { stability: number; friction: number; logic: number; autonomy: number }): CollapseStream[] {
    const ordered = Object.entries({
      stability: avec.stability,
      friction: avec.friction,
      logic: avec.logic,
      autonomy: avec.autonomy,
    }).sort((a, b) => b[1] - a[1]) as [keyof typeof AVEC_COLORS, number][];

    const streams: CollapseStream[] = [];
    for (let i = 0; i < 4; i++) {
      const side = i % 2 === 0 ? -1 : 1;
      const dim = ordered[Math.min(i, ordered.length - 1)][0];
      const nextDim = ordered[Math.min(i + 1, ordered.length - 1)][0];
      const weight = ordered[Math.min(i, ordered.length - 1)][1];
      const count = Math.round(weight * 3) + 1;
      for (let j = 0; j < count; j++) {
        streams.push({
          side,
          offset: (j - (count - 1) / 2) * 7,
          col: AVEC_COLORS[dim],
          destCol: AVEC_COLORS[nextDim],
          thickness: 0.65 + weight * 1.7 * (0.6 + hashUnit(`${dim}-${j}`) * 0.4),
          phase: hashUnit(`${dim}-${j}-phase`) * Math.PI * 2,
          speed: 0.35 + hashUnit(`${dim}-${j}-speed`) * 0.28,
        });
      }
    }
    return streams;
  }

  function projectCollapseOrb(v: [number, number, number], rx: number, ry: number, rz: number, scale: number, cx: number, cy: number) {
    let [x, y, z] = v;
    const ry1 = y * Math.cos(rx) - z * Math.sin(rx);
    const rz1 = y * Math.sin(rx) + z * Math.cos(rx);
    const rx2 = x * Math.cos(ry) + rz1 * Math.sin(ry);
    const rz2 = -x * Math.sin(ry) + rz1 * Math.cos(ry);
    const rx3 = rx2 * Math.cos(rz) - ry1 * Math.sin(rz);
    const ry3 = rx2 * Math.sin(rz) + ry1 * Math.cos(rz);
    const dist = 3;
    const fov = dist / (dist + rz2);
    return { x: cx + rx3 * scale * fov, y: cy + ry3 * scale * fov, z: rz2 };
  }

  function getCollapseOrbFaceColor(faceIdx: number, avec: { stability: number; friction: number; logic: number; autonomy: number }, brightness: number): string {
    const dims = [avec.stability, avec.friction, avec.logic, avec.autonomy];
    const cols = [AVEC_COLORS.stability, AVEC_COLORS.friction, AVEC_COLORS.logic, AVEC_COLORS.autonomy];
    const dimIndex = faceIdx % 4;
    const col = cols[dimIndex];
    const alpha = Math.min(brightness * dims[dimIndex] * 0.74 + brightness * 0.28, 0.92);
    return `rgba(${col.r},${col.g},${col.b},${alpha})`;
  }

  function activeCamX() {
    return camX + negativeLayerDriftX;
  }

  function activeCamY() {
    return camY + negativeLayerDriftY;
  }

  function toScreen(wx: number, wy: number): Vec2 {
    return {
      x: (wx - activeCamX()) * camScale + W() / 2,
      y: (wy - activeCamY()) * camScale + H() / 2,
    };
  }

  function toWorld(sx: number, sy: number): Vec2 {
    return {
      x: (sx - W() / 2) / camScale + activeCamX(),
      y: (sy - H() / 2) / camScale + activeCamY(),
    };
  }

  function canvasXY(e: { clientX: number; clientY: number }): Vec2 {
    const rect = canvas.getBoundingClientRect();
    return {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
    };
  }

  function hashUnit(input: string): number {
    let hash = 2166136261;
    for (let i = 0; i < input.length; i++) {
      hash ^= input.charCodeAt(i);
      hash = Math.imul(hash, 16777619);
    }
    return ((hash >>> 0) % 10000) / 10000;
  }

  function sessionKey(sessionId: string): string {
    return sessionId.startsWith('s:') ? sessionId : `s:${sessionId}`;
  }

  $: sessionById = graph
    ? new Map(graph.sessions.map((session) => [session.id, session]))
    : new Map();

  $: {
    if (!graph || !selectedSession) {
      selectedSessionNodes = [];
      selectedSessionTopMoments = [];
    } else {
      const selectedKey = sessionKey(selectedSession.id);
      const nodes = graph.nodes.filter((node) => sessionKey(node.sessionId) === selectedKey);
      selectedSessionNodes = nodes;
      selectedSessionTopMoments = [...nodes]
        .sort((left, right) => right.psi - left.psi)
        .slice(0, Math.min(4, nodes.length));
    }
  }

  function sessionRenderPos(session: GraphSessionDto): Vec2 {
    const base = sessionPos[session.id];
    if (!base) return { x: W() / 2, y: H() / 2 };
    if (level !== 0) return base;

    const seed = hashUnit(session.id);
    const drift = 8 + Math.min(12, session.nodeCount * 0.35);
    return {
      x: base.x + Math.sin(t * 0.22 + seed * Math.PI * 2) * drift,
      y: base.y + Math.cos(t * 0.18 + seed * Math.PI * 1.7) * drift * 0.72,
    };
  }

  function nodeRenderPos(node: GraphNodeDto): Vec2 {
    const base = nodePos[node.id];
    if (!base) return { x: W() / 2, y: H() / 2 };
    if (level === 2) return base;

    const seed = hashUnit(node.syntheticId || node.id);
    const avec = graphNodeAvec(node);
    const drift = 2.2 + node.psi * 0.22 + avec.autonomy * 2.1;
    const sway = 1.4 + avec.logic * 1.2;
    return {
      x: base.x + Math.sin(t * (0.34 + avec.friction * 0.12) + seed * Math.PI * 2) * drift,
      y: base.y + Math.cos(t * (0.28 + avec.stability * 0.08) + seed * Math.PI * 1.6) * sway,
    };
  }

  function sessionHitRadius(s: GraphSessionDto): number {
    return Math.max(18, (sessionRadius(s) + 14) * camScale);
  }

  function nodeHitRadius(n: GraphNodeDto): number {
    return Math.max(16, (nodeRadius(n) + 12) * camScale);
  }


  // ── Data loading ─────────────────────────────────────────────
  async function loadGraph() {
    try {
      loading = true;
      error   = null;
      const [graphRes, nodesRes]: [GraphResponse, ListNodesResponse] = await Promise.all([
        resonantiaClient.getGraph(200),
        resonantiaClient.listNodes(400),
      ]);
      graph = graphRes;
      if (nodesRes.transport) {
        lastTransportLabel = nodesRes.transport;
      }
      applySourceBadge(nodesRes.source, nodesRes.transport ?? lastTransportLabel);
      hydrateAvecMaps(graphRes, nodesRes.nodes);
      layoutConstellation();
      camX = targetCamX = W() / 2;
      camY = targetCamY = H() / 2;
      camScale = targetCamScale = constellationLayerScale();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function readOnboardingDismissedState(): boolean {
    if (typeof localStorage === 'undefined') {
      return false;
    }

    try {
      return localStorage.getItem(ONBOARDING_DISMISSED_KEY) === '1';
    } catch {
      return false;
    }
  }

  function persistOnboardingDismissedState(value: boolean) {
    if (typeof localStorage === 'undefined') {
      return;
    }

    try {
      if (value) {
        localStorage.setItem(ONBOARDING_DISMISSED_KEY, '1');
      } else {
        localStorage.removeItem(ONBOARDING_DISMISSED_KEY);
      }
    } catch {
      // Ignore storage failures in strict/private browser contexts.
    }
  }

  function readAdventureCompleted(): boolean {
    if (typeof localStorage === 'undefined') return false;
    try { return localStorage.getItem(ADVENTURE_COMPLETED_KEY) === '1'; } catch { return false; }
  }

  function persistAdventureCompleted() {
    if (typeof localStorage === 'undefined') return;
    try { localStorage.setItem(ADVENTURE_COMPLETED_KEY, '1'); } catch {}
  }

  $: hasGraphData = Boolean(graph && ((graph.sessions?.length ?? 0) > 0 || (graph.nodes?.length ?? 0) > 0));

  // Adventure gates the walkthrough: show adventure first for cold users who haven't seen it.
  // If they skip → adventureCompleted=true but onboardingDismissed=false → walkthrough fires next.
  // If they complete → adventureCompleted=true + onboardingDismissed=true → walkthrough skipped.
  $: if (adventureHydrated && onboardingHydrated && !loading && !error && !hasGraphData && !adventureOpen && !onboardingOpen && !onboardingDismissed) {
    if (!adventureCompleted) {
      adventureOpen = true;
    } else {
      openWalkthrough('first-run');
    }
  }

  function walkthroughSelectorForStep(step: WalkthroughStep): string | null {
    switch (step) {
      case 'settings':
        return '[data-tour-target="settings"]';
      case 'checkin':
        return '[data-tour-target="checkin"]';
      case 'telescope':
        return '[data-tour-target="telescope"]';
      case 'importare':
        return '[data-tour-target="compose-importare"]';
      case 'live':
        if (composeOpen && composeMode === 'importare') {
          return '[data-tour-target="compose-switch-live"]';
        }
        return '[data-tour-target="compose-live"]';
      default:
        return null;
    }
  }

  function walkthroughAllowlistForStep(step: WalkthroughStep): string[] {
    switch (step) {
      case 'settings':
        return ['[data-tour-target="settings"]', '[data-tour-target="menu-toggle"]'];
      case 'checkin':
        return ['[data-tour-target="checkin"]', '[data-tour-target="menu-toggle"]'];
      case 'telescope':
        return ['[data-tour-target="telescope"]'];
      case 'importare':
        return ['[data-tour-target="compose-importare"]', '[data-tour-target="compose-toggle"]'];
      case 'live':
        return ['[data-tour-target="compose-live"]', '[data-tour-target="compose-switch-live"]', '[data-tour-target="compose-toggle"]'];
      default:
        return [];
    }
  }

  $: walkthroughTargetSelector = onboardingOpen
    ? walkthroughSelectorForStep(walkthroughStep)
    : null;

  $: walkthroughAllowedSelectors = onboardingOpen
    ? walkthroughAllowlistForStep(walkthroughStep)
    : [];

  $: walkthroughCompact = onboardingOpen
    && walkthroughStep !== 'intro'
    && walkthroughStep !== 'complete'
    && (settingsOpen || calibrateOpen || composeOpen || telescopeCameraEngaged);

  $: if (!onboardingOpen) {
    walkthroughStepSatisfied = false;
  }

  function markWalkthroughStepSatisfied(step: WalkthroughStep) {
    if (onboardingOpen && walkthroughStep === step) {
      walkthroughStepSatisfied = true;
    }
  }

  function toggleMenu() {
    menuOpen = !menuOpen;
  }

  function clearWalkthroughCueTimer() {
    if (walkthroughCueTimer !== null) {
      clearTimeout(walkthroughCueTimer);
      walkthroughCueTimer = null;
    }
  }

  function clearWalkthroughAdvanceTimer() {
    if (walkthroughAdvanceTimer !== null) {
      clearTimeout(walkthroughAdvanceTimer);
      walkthroughAdvanceTimer = null;
    }
  }

  function setWalkthroughStep(step: WalkthroughStep, cueDelayMs = 380) {
    walkthroughStep = step;
    walkthroughStepSatisfied = false;
    walkthroughCueVisible = cueDelayMs <= 0;
    clearWalkthroughCueTimer();

    if (cueDelayMs > 0) {
      walkthroughCueTimer = setTimeout(() => {
        walkthroughCueVisible = true;
        walkthroughCueTimer = null;
      }, cueDelayMs);
    }
  }

  function queueWalkthroughAdvance(
    next: WalkthroughStep,
    options?: {
      holdMs?: number;
      cueDelayMs?: number;
      before?: () => void;
      after?: () => void;
    },
  ) {
    if (!onboardingOpen || walkthroughStepLocked) {
      return;
    }

    walkthroughStepLocked = true;
    options?.before?.();
    clearWalkthroughAdvanceTimer();

    walkthroughAdvanceTimer = setTimeout(() => {
      walkthroughAdvanceTimer = null;
      options?.after?.();
      walkthroughStepLocked = false;
      setWalkthroughStep(next, options?.cueDelayMs ?? 340);
    }, options?.holdMs ?? 620);
  }

  function dismissOnboarding(permanently = true) {
    onboardingOpen = false;
    walkthroughStepLocked = false;
    clearWalkthroughCueTimer();
    clearWalkthroughAdvanceTimer();
    walkthroughStep = 'intro';
    walkthroughCueVisible = false;

    if (permanently && walkthroughMode === 'first-run') {
      onboardingDismissed = true;
      persistOnboardingDismissedState(true);
    }
  }

  function ensureWalkthroughSessionId() {
    const seeded = composeSessionId.trim() || calibSessionId.trim() || WALKTHROUGH_SESSION_SEED;
    composeSessionId = seeded;
    if (!calibSessionId.trim()) {
      calibSessionId = seeded;
    }
    return seeded;
  }

  function openWalkthrough(mode: WalkthroughMode) {
    walkthroughMode = mode;
    walkthroughStepLocked = false;
    clearWalkthroughCueTimer();
    clearWalkthroughAdvanceTimer();
    closeTelescope();
    closeTransientUi();
    syncDetailAutoOpen = false;
    syncDetailHover = false;
    onboardingOpen = true;
    setWalkthroughStep('intro', 0);
  }

  function handleWalkthroughStart() {
    if (!onboardingOpen) {
      return;
    }

    setWalkthroughStep('settings', 360);
  }

  function handleWalkthroughNext() {
    if (!onboardingOpen || walkthroughStepLocked) {
      return;
    }

    if (walkthroughStep !== 'intro' && walkthroughStep !== 'complete' && !walkthroughStepSatisfied) {
      return;
    }

    switch (walkthroughStep) {
      case 'settings':
        queueWalkthroughAdvance('checkin', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            settingsOpen = false;
            menuOpen = false;
          },
        });
        break;
      case 'checkin':
        queueWalkthroughAdvance('telescope', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            calibrateOpen = false;
            menuOpen = false;
          },
        });
        break;
      case 'telescope':
        queueWalkthroughAdvance('importare', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            closeTelescope();
            menuOpen = false;
          },
        });
        break;
      case 'importare':
        queueWalkthroughAdvance('live', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            composeModeMenuOpen = false;
            composeOpen = false;
          },
        });
        break;
      case 'live':
        queueWalkthroughAdvance('complete', {
          holdMs: 220,
          cueDelayMs: 0,
          before: () => {
            composeModeMenuOpen = false;
            composeOpen = false;
            closeTelescope();
            menuOpen = false;
          },
        });
        break;
      default:
        break;
    }
  }

  function handleWalkthroughDismiss(event: CustomEvent<{ permanently: boolean }>) {
    dismissOnboarding(event.detail.permanently);
  }

  function openOnboardingTutorial() {
    openWalkthrough('demo');
  }

  function openCinematicDemo() {
    menuOpen = false;
    openWalkthrough('demo');
  }

  function averageAvecStates(states: Array<{ stability: number; friction: number; logic: number; autonomy: number; psi: number }>) {
    if (!states.length) {
      return { stability: 0.5, friction: 0.5, logic: 0.5, autonomy: 0.5, psi: 2 };
    }

    const totals = states.reduce(
      (acc, state) => ({
        stability: acc.stability + state.stability,
        friction: acc.friction + state.friction,
        logic: acc.logic + state.logic,
        autonomy: acc.autonomy + state.autonomy,
        psi: acc.psi + state.psi,
      }),
      { stability: 0, friction: 0, logic: 0, autonomy: 0, psi: 0 },
    );

    return {
      stability: totals.stability / states.length,
      friction: totals.friction / states.length,
      logic: totals.logic / states.length,
      autonomy: totals.autonomy / states.length,
      psi: totals.psi / states.length,
    };
  }

  function mergedNodeAvec(node: NodeDto) {
    const states = [node.userAvec, node.modelAvec];
    if (node.compressionAvec) states.push(node.compressionAvec);
    return averageAvecStates(states);
  }

  function hydrateAvecMaps(graphRes: GraphResponse, nodes: NodeDto[]) {
    const nextNodeAvecMap: Record<string, { stability: number; friction: number; logic: number; autonomy: number; psi: number }> = {};
    const sessionBuckets: Record<string, Array<{ stability: number; friction: number; logic: number; autonomy: number; psi: number }>> = {};

    nodes.forEach(node => {
      const avec = mergedNodeAvec(node);
      nextNodeAvecMap[node.syntheticId] = avec;
      const key = sessionKey(node.sessionId);
      if (!sessionBuckets[key]) sessionBuckets[key] = [];
      sessionBuckets[key].push(avec);
    });

    const nextSessionAvecMap: Record<string, { stability: number; friction: number; logic: number; autonomy: number; psi: number }> = {};

    graphRes.sessions.forEach(session => {
      const key = sessionKey(session.id);
      nextSessionAvecMap[key] = averageAvecStates(sessionBuckets[key] ?? []);
    });

    sessionAvecMap = nextSessionAvecMap;
    nodeAvecMap = nextNodeAvecMap;
  }

  function fallbackAvec(psiValue: number) {
    const stability = Math.max(0.16, Math.min(0.92, 0.34 + psiValue * 0.08));
    const friction = Math.max(0.12, Math.min(0.82, 0.2 + psiValue * 0.03));
    const logic = Math.max(0.18, Math.min(0.94, 0.3 + psiValue * 0.09));
    const autonomy = Math.max(0.14, Math.min(0.9, 0.28 + psiValue * 0.07));
    return { stability, friction, logic, autonomy, psi: stability + friction + logic + autonomy };
  }

  function sessionAvec(session: GraphSessionDto) {
    return sessionAvecMap[sessionKey(session.id)] ?? fallbackAvec(session.avgPsi);
  }

  function graphNodeAvec(node: GraphNodeDto) {
    return nodeAvecMap[node.syntheticId] ?? sessionAvecMap[sessionKey(node.sessionId)] ?? fallbackAvec(node.psi);
  }

  function fieldAvecRgb(avec: { stability: number; friction: number; logic: number; autonomy: number; psi: number }) {
    const mixed = avecToRgb(avec);
    const ordered = ([
      ['stability', avec.stability],
      ['friction', avec.friction],
      ['logic', avec.logic],
      ['autonomy', avec.autonomy],
    ] as [AvecDim, number][]).sort((left, right) => right[1] - left[1]);
    const primary = AVEC_COLORS[ordered[0][0]];
    const secondary = AVEC_COLORS[ordered[1][0]];
    const dominance = ordered[0][1] - ordered[2][1];
    const primaryMix = 0.24 + dominance * 0.18;
    const secondaryMix = 0.1 + ordered[1][1] * 0.12;
    const baseMix = Math.max(0.42, 1 - primaryMix - secondaryMix);

    let r = mixed.r * baseMix + primary.r * primaryMix + secondary.r * secondaryMix;
    let g = mixed.g * baseMix + primary.g * primaryMix + secondary.g * secondaryMix;
    let b = mixed.b * baseMix + primary.b * primaryMix + secondary.b * secondaryMix;

    const avg = (r + g + b) / 3;
    const saturationBoost = 1.26 + dominance * 0.9 + avec.autonomy * 0.12;
    r = avg + (r - avg) * saturationBoost;
    g = avg + (g - avg) * (saturationBoost + 0.03);
    b = avg + (b - avg) * (saturationBoost - 0.01);

    const glowLift = 0.062 + avec.stability * 0.105;
    r += 255 * glowLift * 0.25;
    g += 255 * glowLift * 0.31;
    b += 255 * glowLift * 0.26;

    return {
      r: Math.max(0, Math.min(255, r)),
      g: Math.max(0, Math.min(255, g)),
      b: Math.max(0, Math.min(255, b)),
    };
  }

  function fieldAvecColor(
    avec: { stability: number; friction: number; logic: number; autonomy: number; psi: number },
    alpha = 1,
  ) {
    const { r, g, b } = fieldAvecRgb(avec);
    return `rgba(${Math.round(r)},${Math.round(g)},${Math.round(b)},${alpha})`;
  }

  function vectorFromAvec(avec: { stability: number; friction: number; logic: number; autonomy: number }) {
    return {
      stability: avec.stability,
      friction: avec.friction,
      logic: avec.logic,
      autonomy: avec.autonomy,
    };
  }

  function blendCalibrationVectors(left: CalibrationVector, right: CalibrationVector, mix: number): CalibrationVector {
    return {
      stability: left.stability * (1 - mix) + right.stability * mix,
      friction: left.friction * (1 - mix) + right.friction * mix,
      logic: left.logic * (1 - mix) + right.logic * mix,
      autonomy: left.autonomy * (1 - mix) + right.autonomy * mix,
    };
  }

  function rankedDimensionWeights(vector: CalibrationVector) {
    const sorted = [...AVEC_DIMS].sort((left, right) => vector[right] - vector[left]);
    const weights: Record<AvecDim, number> = {
      stability: 0.22,
      friction: 0.22,
      logic: 0.22,
      autonomy: 0.22,
    };
    [1, 0.78, 0.58, 0.38].forEach((weight, index) => {
      weights[sorted[index]] = weight;
    });
    return weights;
  }

  function resonanceAnchorVector() {
    return closestCalibrationProfile
      ? blendCalibrationVectors(currentCalibrationVector, closestCalibrationProfile.profile.values, 0.34)
      : currentCalibrationVector;
  }

  function avecSimilarity(
    left: { stability: number; friction: number; logic: number; autonomy: number },
    right: { stability: number; friction: number; logic: number; autonomy: number },
  ) {
    const distance = calibrationDistance(vectorFromAvec(left), vectorFromAvec(right));
    return clamp01(1 - distance / 2);
  }

  function edgeResonanceStrength(
    sourceAvec: { stability: number; friction: number; logic: number; autonomy: number; psi: number },
    targetAvec: { stability: number; friction: number; logic: number; autonomy: number; psi: number },
    kind: string,
  ) {
    const similarity = avecSimilarity(sourceAvec, targetAvec);
    const midpoint = {
      stability: (sourceAvec.stability + targetAvec.stability) / 2,
      friction: (sourceAvec.friction + targetAvec.friction) / 2,
      logic: (sourceAvec.logic + targetAvec.logic) / 2,
      autonomy: (sourceAvec.autonomy + targetAvec.autonomy) / 2,
    };
    const anchorMatch = avecSimilarity(midpoint, resonanceAnchorVector());
    const kindBias = kind === 'resonance' ? 1 : kind === 'temporal' ? 0.72 : 0.58;
    return clamp01((similarity * 0.62 + anchorMatch * 0.38) * kindBias);
  }

  function signatureForAvec(avec: { stability: number; friction: number; logic: number; autonomy: number; psi: number }) {
    const anchor = resonanceAnchorVector();
    const rankWeights = rankedDimensionWeights(anchor);
    const dims = AVEC_DIMS.map(dim => {
      const affinity = 1 - Math.abs(avec[dim] - anchor[dim]);
      const score = affinity * 0.56 + avec[dim] * 0.26 + rankWeights[dim] * 0.18;
      return { dim, score, affinity, value: avec[dim] };
    }).sort((left, right) => right.score - left.score);

    const euclidean = calibrationDistance(vectorFromAvec(avec), anchor);
    const resonance = clamp01(1 - euclidean / 2);
    const spread = Math.max(...dims.map(entry => entry.value)) - Math.min(...dims.map(entry => entry.value));

    return {
      primary: dims[0].dim,
      secondary: dims[1].dim,
      resonance,
      spread,
      primaryAffinity: dims[0].affinity,
      secondaryAffinity: dims[1].affinity,
    };
  }

  function signatureAngle(primary: AvecDim, secondary: AvecDim, seed: number) {
    const baseAngles: Record<AvecDim, number> = {
      stability: -Math.PI / 2,
      friction: Math.PI * 0.12,
      logic: Math.PI * 0.78,
      autonomy: Math.PI * 1.46,
    };
    const secondaryOffset: Record<AvecDim, number> = {
      stability: -0.26,
      friction: 0.18,
      logic: 0.34,
      autonomy: -0.34,
    };
    return baseAngles[primary] + secondaryOffset[secondary] + seed * 0.9;
  }

  function drawAvecWhisper(
    x: number,
    y: number,
    radius: number,
    lineWidth: number,
    avec: { stability: number; friction: number; logic: number; autonomy: number; psi: number },
    alpha: number,
    seed: number,
  ) {
    const signature = signatureForAvec(avec);
    const primaryCol = AVEC_COLORS[signature.primary];
    const secondaryCol = AVEC_COLORS[signature.secondary];
    const angle = signatureAngle(signature.primary, signature.secondary, seed);
    const accentAlpha = alpha * (0.54 + signature.resonance * 0.32);
    const shellTilt = angle + Math.sin(t * 0.3 + seed * 8) * 0.14;
    const shellW = radius * (1.06 + signature.resonance * 0.12);
    const shellH = radius * (0.75 + signature.spread * 0.22);
    const innerW = radius * (0.82 + signature.primaryAffinity * 0.12);
    const innerH = radius * (0.6 + signature.secondaryAffinity * 0.1);
    const moteAngle = angle + t * (0.16 + signature.secondaryAffinity * 0.16) + seed * Math.PI * 2;
    const moteRadius = radius + 2.6 + signature.resonance * 2.2;

    ctx.save();
    ctx.translate(x, y);
    ctx.rotate(shellTilt);

    ctx.beginPath();
    ctx.ellipse(0, 0, shellW, shellH, 0, 0, Math.PI * 2);
    ctx.strokeStyle = fieldAvecColor(avec, alpha * 0.2);
    ctx.lineWidth = lineWidth;
    ctx.stroke();

    ctx.beginPath();
    ctx.ellipse(0, 0, innerW, innerH, 0, 0, Math.PI * 2);
    ctx.strokeStyle = `rgba(${primaryCol.r},${primaryCol.g},${primaryCol.b},${accentAlpha * 0.6})`;
    ctx.lineWidth = lineWidth * 0.8;
    ctx.stroke();

    ctx.restore();

    const moteX = x + Math.cos(moteAngle) * moteRadius;
    const moteY = y + Math.sin(moteAngle) * (moteRadius * 0.72);
    ctx.beginPath();
    ctx.arc(moteX, moteY, Math.max(0.9, lineWidth * 0.95), 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${secondaryCol.r},${secondaryCol.g},${secondaryCol.b},${alpha * 0.48})`;
    ctx.fill();

    const traces = [
      {
        side: Math.sin(seed * Math.PI * 2) > 0 ? 1 : -1,
        offset: -radius * 0.2,
        amp: 1,
        lead: primaryCol,
        tail: secondaryCol,
        alphaScale: 1,
      },
      {
        side: Math.sin(seed * Math.PI * 2) > 0 ? -1 : 1,
        offset: radius * 0.14,
        amp: 0.82,
        lead: secondaryCol,
        tail: primaryCol,
        alphaScale: 0.72,
      },
    ];

    traces.forEach((trace, index) => {
      const startX = x + trace.side * (radius + 10 + signature.resonance * 7 + index * 2);
      const endX = x + trace.side * (radius * (0.8 + index * 0.04));
      const baseY = y + trace.offset + Math.sin(t * (0.36 + index * 0.06) + seed * (6 + index)) * (1.4 + signature.spread * 2.1);
      const midX = (startX + endX) / 2;
      const wave = Math.sin(t * (0.48 + signature.secondaryAffinity * 0.24 + index * 0.05) + seed * (9 + index * 2))
        * (2.8 + signature.primaryAffinity * 2.2) * trace.amp;
      const cp1x = midX;
      const cp1y = baseY + wave;
      const cp2x = endX + trace.side * (8 + index * 2);
      const cp2y = baseY - wave * 0.48;
      const grad = ctx.createLinearGradient(startX, baseY, endX, baseY);

      if (trace.side < 0) {
        grad.addColorStop(0, `rgba(${trace.tail.r},${trace.tail.g},${trace.tail.b},0)`);
        grad.addColorStop(0.4, `rgba(${trace.tail.r},${trace.tail.g},${trace.tail.b},${alpha * 0.16 * trace.alphaScale})`);
        grad.addColorStop(0.88, `rgba(${trace.lead.r},${trace.lead.g},${trace.lead.b},${accentAlpha * 0.28 * trace.alphaScale})`);
        grad.addColorStop(1, `rgba(${trace.lead.r},${trace.lead.g},${trace.lead.b},0)`);
      } else {
        grad.addColorStop(0, `rgba(${trace.tail.r},${trace.tail.g},${trace.tail.b},0)`);
        grad.addColorStop(0.16, `rgba(${trace.tail.r},${trace.tail.g},${trace.tail.b},${alpha * 0.14 * trace.alphaScale})`);
        grad.addColorStop(0.7, `rgba(${trace.lead.r},${trace.lead.g},${trace.lead.b},${accentAlpha * 0.26 * trace.alphaScale})`);
        grad.addColorStop(1, `rgba(${trace.lead.r},${trace.lead.g},${trace.lead.b},0)`);
      }

      ctx.beginPath();
      ctx.moveTo(startX, baseY);
      ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, endX, y + Math.sin(shellTilt) * radius * 0.14 + trace.offset * 0.18);
      ctx.strokeStyle = grad;
      ctx.lineWidth = Math.max(0.34, lineWidth * (0.56 - index * 0.1));
      ctx.lineCap = 'round';
      ctx.stroke();
      ctx.lineCap = 'butt';
    });
  }

  function layoutConstellation() {
    if (!graph) return;
    Object.keys(sessionPos).forEach(key => delete sessionPos[key]);
    Object.keys(nodePos).forEach(key => delete nodePos[key]);
    const cx = W() / 2, cy = H() / 2;
    const n  = graph.sessions.length;
    const aspect = W() / Math.max(H(), 1);
    const compact = compactViewport();
    const densityScale = 0.94 + Math.min(0.34, Math.sqrt(Math.max(1, n)) * 0.08);
    const baseSpread = Math.min(W(), H()) * (compact ? 0.45 : 0.4) * densityScale;
    const spreadX = baseSpread * Math.max(0.92, Math.min(1.34, aspect * (compact ? 1.06 : 1)));
    const spreadY = baseSpread * Math.max(0.9, Math.min(1.18, (1 / Math.max(aspect, 0.01)) * (compact ? 0.86 : 0.82)));
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));

    graph.sessions.forEach((s, i) => {
      const seed = hashUnit(s.id);
      const angle = i * goldenAngle + seed * 1.8;
      const radial = 0.24 + Math.sqrt((i + 0.5) / Math.max(1, n)) * 0.88;
      const bendX = Math.sin(angle * 1.4 + seed * 8) * spreadX * 0.14;
      const bendY = Math.cos(angle * 1.1 + seed * 6) * spreadY * 0.12;
      sessionPos[s.id] = {
        x: cx + Math.cos(angle) * spreadX * radial + bendX,
        y: cy + Math.sin(angle) * spreadY * radial + bendY,
      };
    });

      graph.sessions.forEach((session, sessionIndex) => {
        const sp = sessionPos[sessionKey(session.id)];
      if (!sp) return;

        const sessionNodes = graph!.nodes.filter(node => sessionKey(node.sessionId) === sessionKey(session.id));
      const orbitRadius = sessionOrbitRadius(session, sessionNodes.length);
      const seed = sessionIndex * 0.73 + session.id.length * 0.11;

      sessionNodes.forEach((node, nodeIndex) => {
        const progress = (nodeIndex + 1) / (sessionNodes.length + 1);
        const radial = orbitRadius * (0.34 + Math.sqrt(progress) * 0.88);
        const angle = seed + nodeIndex * 2.399963229728653;
        nodePos[node.id] = {
          x: sp.x + Math.cos(angle) * radial,
          y: sp.y + Math.sin(angle) * radial * 0.72,
        };
      });
    });
  }

  function sessionRadius(s: GraphSessionDto) { return 8 + s.nodeCount * 1.65; }
  function nodeRadius(n: GraphNodeDto)        { return 3.4 + n.psi * 1.08;    }

  function sessionOrbitRadius(session: GraphSessionDto, nodeCount?: number) {
    const count = nodeCount ?? graph?.nodes.filter(node => sessionKey(node.sessionId) === sessionKey(session.id)).length ?? session.nodeCount;
    return Math.max(66, sessionRadius(session) + 28 + Math.min(count, 10) * 2.4);
  }

  function waveCameraScale(session: GraphSessionDto) {
    const orbitRadius = sessionOrbitRadius(session);
    const worldHalfWidth = orbitRadius + 24;
    const worldHalfHeight = orbitRadius * 0.76 + 28;
    const fitX = (W() * (compactViewport() ? 0.46 : 0.4)) / Math.max(worldHalfWidth, 1);
    const fitY = (H() * (compactViewport() ? 0.38 : 0.32)) / Math.max(worldHalfHeight, 1);
    const minWaveScale = compactViewport() ? 1.58 : (mediumViewport() ? 1.9 : 2.05);
    return Math.max(minWaveScale, Math.min(WAVE_SCALE, Math.min(fitX, fitY)));
  }

  function collapseDescriptors(avec: { stability: number; friction: number; logic: number; autonomy: number }): string {
    const dims: [string, number][] = [
      ['stability', avec.stability],
      ['friction', avec.friction],
      ['logic', avec.logic],
      ['autonomy', avec.autonomy],
    ];

    return dims
      .sort((a, b) => b[1] - a[1])
      .slice(0, 3)
      .map(([label]) => label)
      .join(' · ');
  }

  function momentWhisperLabel(node: GraphNodeDto): string {
    const parts = node.label.split(' ');
    if (parts.length >= 2) return parts.slice(0, 2).join(' ');
    return shortLabel(node.label, 2);
  }

  function waveTitle(session: GraphSessionDto): string {
    return shortLabel(session.label, 3);
  }

  // ── Navigation ────────────────────────────────────────────────
  function descendToWave(s: GraphSessionDto) {
    noteInteraction();
    closeTransientUi();
    negativeLayerActive = false;
    selectedSession = s;
    selectedNode    = null;
    closeCard();
    level = 1;
    const sp = sessionRenderPos(s);
    if (sp) {
      targetCamX = sp.x;
      targetCamY = sp.y + sessionOrbitRadius(s) * 0.03;
      targetCamScale = waveCameraScale(s);
    }
  }

  async function descendToCollapse(n: GraphNodeDto) {
    noteInteraction();
    closeTransientUi();
    selectedNode = n;
    level        = 2;
    transmuteError = null;
    transmuting = false;
    const np = nodeRenderPos(n);
    if (np) { targetCamX = np.x; targetCamY = np.y; targetCamScale = collapseCameraScale(); }

    cardData = {
      node:            n,
      nodeDto:         null,
      relatedSessions: graph?.sessions.filter(s => s.id !== sessionKey(n.sessionId)).slice(0, 4) ?? [],
    };
    setTimeout(() => { cardVisible = true; }, 520);

    try {
      const res: ListNodesResponse = await resonantiaClient.listNodes(
        Math.max(selectedSession?.nodeCount ?? 50, 50),
        n.sessionId,
      );
      if (res.transport) {
        lastTransportLabel = res.transport;
      }
      applySourceBadge(res.source, res.transport ?? lastTransportLabel);
      const dto = res.nodes.find(node => matchesSelectedNode(n, node)) ?? null;
      if (dto && cardData) cardData = { ...cardData, nodeDto: dto };
    } catch { /* card shows what it has */ }
  }

  function surfaceToWave() {
    noteInteraction();
    closeTransientUi();
    selectedNode = null;
    closeCard();
    level = 1;
    if (selectedSession) {
      const sp = sessionPos[selectedSession.id];
      if (sp) {
        targetCamX = sp.x;
        targetCamY = sp.y + sessionOrbitRadius(selectedSession) * 0.03;
        targetCamScale = waveCameraScale(selectedSession);
      }
    }
  }

  function surfaceToConstellation() {
    noteInteraction();
    closeTransientUi();
    selectedSession = null;
    selectedNode    = null;
    negativeLayerActive = false;
    closeCard();
    level = 0;
    targetCamX = W() / 2; targetCamY = H() / 2; targetCamScale = constellationLayerScale();
  }

  function closeCard() {
    cardVisible = false;
    transmuting = false;
    transmuteError = null;
    setTimeout(() => { if (!cardVisible) cardData = null; }, 500);
  }

  async function transmuteCurrentNode() {
    if (!cardData?.nodeDto?.raw || !cardData?.node?.syntheticId || transmuting) return;

    const syntheticId = cardData.node.syntheticId;
    if (transmutationCache[syntheticId]) return;

    transmuting = true;
    transmuteError = null;
    try {
      const summary = await resonantiaClient.summarizeNode(cardData.nodeDto.raw);

      if (!summary) {
        transmuteError = 'The model answered, but the transmutation did not resolve into a readable form.';
        return;
      }

      transmutationCache = {
        ...transmutationCache,
        [syntheticId]: summary,
      };
    } catch (err) {
      transmuteError = String(err);
    } finally {
      transmuting = false;
    }
  }

  function closeTransientUi() {
    menuOpen = false;
    composeOpen = false;
    calibrateOpen = false;
    settingsOpen = false;
  }

  function handleNavigate(e: CustomEvent<{ sessionId: string }>) {
    const target = graph?.sessions.find(s => s.id === e.detail.sessionId);
    if (target) descendToWave(target);
  }

  async function openExternalUrl(url: string) {
    try {
      if (typeof window !== 'undefined' && ('__TAURI_INTERNALS__' in window || '__TAURI__' in window)) {
        const opener = await import('@tauri-apps/plugin-opener');
        await opener.openUrl(url);
        return;
      }
    } catch {
      // Fall through to browser navigation fallback.
    }

    if (typeof window !== 'undefined') {
      window.open(url, '_blank', 'noopener,noreferrer');
      return;
    }

    throw new Error('Unable to open external URL in this runtime.');
  }

  // ── Draw helpers ─────────────────────────────────────────────
  function drawStars() {
    const dim = level === 0 ? 1 : level === 1 ? 0.45 : 0.15;
    STATIC_STARS.forEach(s => {
      const a = (0.15 + Math.sin(t * 0.6 + s.phase) * 0.1) * dim;
      ctx.beginPath();
      ctx.arc(s.x * W(), s.y * H(), s.s, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(255,255,255,${a})`;
      ctx.fill();
    });
  }

  function drawEdges() {
    if (telescopeCameraEngaged || !graph || level > 0) return;
    const graphData = graph;
    graphData.edges.forEach(e => {
      const sourceSession = sessionById.get(e.source);
      const targetSession = sessionById.get(e.target);
      if (!sourceSession || !targetSession) return;
      const sp = sessionRenderPos(sourceSession);
      const tp = sessionRenderPos(targetSession);
      if (!sp || !tp) return;
      const sourceAvec = sessionAvec(sourceSession);
      const targetAvec = sessionAvec(targetSession);
      const strength = edgeResonanceStrength(sourceAvec, targetAvec, e.kind);
      const midpointAvec = {
        stability: (sourceAvec.stability + targetAvec.stability) / 2,
        friction: (sourceAvec.friction + targetAvec.friction) / 2,
        logic: (sourceAvec.logic + targetAvec.logic) / 2,
        autonomy: (sourceAvec.autonomy + targetAvec.autonomy) / 2,
        psi: (sourceAvec.psi + targetAvec.psi) / 2,
      };
      const mx = (sp.x + tp.x) / 2 + Math.sin(e.id.length * 0.7) * 30;
      const my = (sp.y + tp.y) / 2 + Math.cos(e.id.length * 0.5) * 20;
      const grad = ctx.createLinearGradient(sp.x, sp.y, tp.x, tp.y);
      grad.addColorStop(0, fieldAvecColor(sourceAvec, 0));
      grad.addColorStop(0.28, fieldAvecColor(sourceAvec, 0.05 + strength * 0.08));
      grad.addColorStop(0.5, fieldAvecColor(midpointAvec, 0.06 + strength * 0.1));
      grad.addColorStop(0.72, fieldAvecColor(targetAvec, 0.05 + strength * 0.08));
      grad.addColorStop(1, fieldAvecColor(targetAvec, 0));

      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.quadraticCurveTo(mx, my, tp.x, tp.y);
      ctx.strokeStyle = fieldAvecColor(midpointAvec, 0.01 + strength * 0.035);
      ctx.lineWidth = (0.8 + strength * 3.2) / camScale;
      ctx.stroke();

      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.quadraticCurveTo(mx, my, tp.x, tp.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth   = ((e.kind === 'resonance' ? 0.24 : 0.14) + strength * (e.kind === 'resonance' ? 0.72 : 0.42)) / camScale;
      ctx.setLineDash(e.kind === 'temporal' ? [3, 6] : []);
      ctx.stroke();
      ctx.setLineDash([]);
    });
  }

  function drawSessions() {
    if (telescopeCameraEngaged || !graph) return;

    graph.sessions.forEach(s => {
      const sp      = sessionRenderPos(s);
      if (!sp) return;
      const isFocus = selectedSession?.id === s.id;
      const av = sessionAvec(s);
      const seed = hashUnit(s.id);

      if (level > 0 && !isFocus) {
        ctx.beginPath();
        ctx.arc(sp.x, sp.y, 2, 0, Math.PI * 2);
        ctx.fillStyle = fieldAvecColor(av, 0.42);
        ctx.fill();
        return;
      }

      const r     = sessionRadius(s);
      const pulse = isFocus && level === 1
        ? Math.sin(t * 3) * 2.5
        : Math.sin(t * 1.1 + sp.x * 0.008) * 1.2;

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, r + pulse + 8, 0, Math.PI * 2);
      ctx.fillStyle = fieldAvecColor(av, isFocus ? 0.12 : 0.09);
      ctx.fill();

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, r + pulse, 0, Math.PI * 2);
      ctx.fillStyle   = fieldAvecColor(av, isFocus ? 0.45 : 0.29);
      ctx.fill();
      ctx.strokeStyle = fieldAvecColor(av, isFocus ? 0.9 : 0.62);
      ctx.lineWidth   = (isFocus ? 0.82 : 0.56) / camScale;
      ctx.stroke();

      drawAvecWhisper(sp.x, sp.y, r + pulse + 3.8, (isFocus ? 1.45 : 0.78) / camScale, av, isFocus ? 0.44 : 0.3, seed);

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, 2.5, 0, Math.PI * 2);
      ctx.fillStyle = fieldAvecColor(av, 0.98);
      ctx.fill();

      if (level === 0 && r > 18) {
        ctx.fillStyle = fieldAvecColor(av, 0.92);
        ctx.font      = `10px ${FONT_MONO}`;
        ctx.textAlign = 'center';
        ctx.fillText(shortLabel(s.label, 3), sp.x, sp.y + r + 15);
      }
    });
  }

  function drawWaveBoundary() {
    if (telescopeCameraEngaged || level !== 1 || !selectedSession) return;
    const sp = sessionPos[selectedSession.id];
    if (!sp) return;
    const av = sessionAvec(selectedSession);
    const orbit = sessionOrbitRadius(selectedSession, selectedSessionNodes.length);
    const rx  = orbit + 18;
    const ry  = (orbit + 18) * 0.74;
    const oda = 4 + Math.sin(t * 0.5);

    ctx.beginPath();
    ctx.ellipse(sp.x, sp.y, rx + 22, ry + 16, 0, 0, Math.PI * 2);
    ctx.strokeStyle = fieldAvecColor(av, 0.24);
    ctx.lineWidth   = 12 / camScale;
    ctx.setLineDash([]);
    ctx.stroke();

    ctx.beginPath();
    ctx.ellipse(sp.x, sp.y, rx, ry, 0, 0, Math.PI * 2);
    ctx.strokeStyle = fieldAvecColor(av, 0.68);
    ctx.lineWidth   = 0.5 / camScale;
    ctx.setLineDash([oda, oda * 1.6]);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  function drawWaveThreads() {
    if (telescopeCameraEngaged || level !== 1 || !selectedSession || !graph) return;
    const session = selectedSession;
    const sp = sessionPos[session.id];
    if (!sp) return;

    const sessionNodes = selectedSessionNodes;
    if (sessionNodes.length === 0) return;

    const baseAvec = sessionAvec(session);
    ctx.lineWidth = 0.6 / camScale;

    sessionNodes.forEach((node, index) => {
      const np = nodeRenderPos(node);
      if (!np) return;
      const nav = graphNodeAvec(node);
      const strength = avecSimilarity(baseAvec, nav);
      const grad = ctx.createLinearGradient(sp.x, sp.y, np.x, np.y);
      grad.addColorStop(0, fieldAvecColor(baseAvec, 0.16));
      grad.addColorStop(0.54, fieldAvecColor(nav, 0.14 + strength * 0.16));
      grad.addColorStop(1, fieldAvecColor(nav, 0.04));

      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.lineTo(np.x, np.y);
      ctx.strokeStyle = fieldAvecColor(nav, 0.01 + strength * 0.028);
      ctx.lineWidth = (0.55 + strength * 1.4) / camScale;
      ctx.stroke();

      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.lineTo(np.x, np.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth = (0.18 + strength * 0.48) / camScale;
      ctx.stroke();

      if (index > 0) {
        const prev = nodeRenderPos(sessionNodes[index - 1]);
        if (!prev) return;
        const prevAvec = graphNodeAvec(sessionNodes[index - 1]);
        const linkStrength = avecSimilarity(prevAvec, nav);
        const linkGrad = ctx.createLinearGradient(prev.x, prev.y, np.x, np.y);
        linkGrad.addColorStop(0, fieldAvecColor(prevAvec, 0.23));
        linkGrad.addColorStop(1, fieldAvecColor(nav, 0.12 + linkStrength * 0.12));
        ctx.beginPath();
        ctx.moveTo(prev.x, prev.y);
        ctx.lineTo(np.x, np.y);
        ctx.strokeStyle = fieldAvecColor(nav, 0.008 + linkStrength * 0.022);
        ctx.lineWidth = (0.42 + linkStrength * 0.9) / camScale;
        ctx.stroke();
        ctx.beginPath();
        ctx.moveTo(prev.x, prev.y);
        ctx.lineTo(np.x, np.y);
        ctx.strokeStyle = linkGrad;
        ctx.lineWidth = (0.14 + linkStrength * 0.34) / camScale;
        ctx.stroke();
      }
    });
  }

  function drawNodes() {
    if (telescopeCameraEngaged || !graph || level < 1 || !selectedSession) return;
    const sessionNodes = selectedSessionNodes;

    sessionNodes.forEach(n => {
      const np = nodeRenderPos(n);
      if (!np) return;
      const r          = nodeRadius(n);
      const isSelected = selectedNode?.id === n.id;
      const av = graphNodeAvec(n);
      const seed = hashUnit(n.syntheticId || n.id);

      if (level === 2 && !isSelected) {
        ctx.beginPath();
        ctx.arc(np.x, np.y, 2.6, 0, Math.PI * 2);
        ctx.fillStyle = fieldAvecColor(av, 0.32);
        ctx.fill();
        return;
      }

      const pulse = Math.sin(t * 1.8 + np.x * 0.04);
      const tierAlpha = n.tier === 'daily' ? 0.32 : n.tier === 'weekly' ? 0.28 : 0.24;

      ctx.beginPath();
      ctx.arc(np.x, np.y, r + pulse + 4.5, 0, Math.PI * 2);
      ctx.fillStyle = fieldAvecColor(av, isSelected ? 0.28 : 0.18);
      ctx.fill();

      ctx.beginPath();
      ctx.arc(np.x, np.y, r + pulse, 0, Math.PI * 2);
      ctx.fillStyle   = fieldAvecColor(av, isSelected ? 0.92 : tierAlpha * 1.34);
      ctx.fill();
      ctx.strokeStyle = fieldAvecColor(av, isSelected ? 1 : 0.68);
      ctx.lineWidth   = (isSelected ? 0.9 : 0.6) / camScale;
      ctx.stroke();

      drawAvecWhisper(np.x, np.y, r + pulse + 2.6, (isSelected ? 1.06 : 0.58) / camScale, av, isSelected ? 0.5 : 0.3, seed);

      ctx.beginPath();
      ctx.arc(np.x, np.y, Math.max(2.2, r * 0.34), 0, Math.PI * 2);
      ctx.fillStyle = fieldAvecColor(av, 0.94);
      ctx.fill();

    });
  }

  function drawWaveLabels() {
    if (telescopeCameraEngaged || level !== 1 || !graph || !selectedSession) return;
    const session = selectedSession;
    const sp = sessionPos[session.id];
    if (!sp) return;

    const labeledMoments = selectedSessionTopMoments;

    labeledMoments.forEach(node => {
      const np = nodeRenderPos(node);
      if (!np) return;
      const nav = graphNodeAvec(node);
      const sc = toScreen(np.x, np.y);
      ctx.textAlign = 'center';
      ctx.font = `600 8px ${FONT_MONO}`;
      ctx.strokeStyle = 'rgba(7,10,13,0.85)';
      ctx.lineWidth = 2.4;
      ctx.strokeText(momentWhisperLabel(node), sc.x, sc.y + 22);
      ctx.fillStyle = fieldAvecColor(nav, 0.9);
      ctx.fillText(momentWhisperLabel(node), sc.x, sc.y + 22);
    });

    const center = toScreen(sp.x, sp.y);
    const av = sessionAvec(session);
    ctx.textAlign = 'center';
    ctx.font = `600 9px ${FONT_MONO}`;
    ctx.strokeStyle = 'rgba(7,10,13,0.9)';
    ctx.lineWidth = 2.8;
    ctx.strokeText(waveTitle(session), center.x, center.y + 28);
    ctx.fillStyle = fieldAvecColor(av, 0.94);
    ctx.fillText(waveTitle(session), center.x, center.y + 28);
  }

  function drawCollapseOrb() {
    if (level !== 2 || !selectedNode) return;
    const np = nodePos[selectedNode.id];
    if (!np) return;
    const sc = toScreen(np.x, np.y);

    const avec = cardData?.nodeDto?.userAvec ?? {
      stability:0.75, friction:0.18, logic:0.82, autonomy:0.88, psi:2.63,
    };
    const { r, g, b } = avecToRgb(avec);
    const col = `${Math.round(r)},${Math.round(g)},${Math.round(b)}`;
    const psiVal = avec.stability + avec.friction + avec.logic + avec.autonomy;
    const pulseRate = 0.3 + (1 - psiVal / 4) * 1.2;
    const pulseAmt = 0.04 + (1 - psiVal / 4) * 0.08;
    const driftLift = Math.sin(t * 0.48 + psiVal) * 7;
    const driftSway = Math.cos(t * 0.26 + psiVal * 0.7) * 4.5;
    const orbX = sc.x + driftSway;
    const orbY = sc.y - 34 + driftLift;
    const baseRadius = 78;
    const scale = baseRadius * (1 + Math.sin(t * pulseRate) * pulseAmt);
    const rot = {
      x: t * 0.15 + Math.sin(t * 0.22) * 0.08,
      y: t * 0.2 + Math.cos(t * 0.17) * 0.09,
      z: t * 0.06 + Math.sin(t * 0.13 + psiVal) * 0.04,
    };
    const glowRadius = scale * 1.42;
    const streams = buildCollapseStreams(avec);
    const shellTilt = Math.sin(t * 0.19 + psiVal) * 0.08;

    ctx.save();

    ctx.save();
    ctx.translate(orbX, orbY + 16);
    ctx.rotate(shellTilt);
    ctx.beginPath();
    ctx.ellipse(0, 0, 112, 84, 0, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(255,255,255,0.035)';
    ctx.lineWidth = 0.8;
    ctx.setLineDash([4, 8]);
    ctx.stroke();
    ctx.setLineDash([]);
    ctx.restore();

    ctx.save();
    ctx.translate(orbX, orbY + 8);
    ctx.rotate(-shellTilt * 1.6);
    ctx.beginPath();
    ctx.ellipse(0, 0, 86, 62, 0, 0, Math.PI * 2);
    ctx.strokeStyle = `rgba(${col},0.07)`;
    ctx.lineWidth = 0.7;
    ctx.stroke();
    ctx.restore();

    streams.forEach(stream => {
      const startX = stream.side < 0 ? orbX - 178 : orbX + 178;
      const endX = stream.side < 0 ? orbX - scale * 0.92 : orbX + scale * 0.92;
      const baseY = orbY + stream.offset + Math.sin(t * 0.42 + stream.phase) * 1.8;
      const midX = (startX + endX) / 2;
      const wave = Math.sin(t * stream.speed + stream.phase) * (11 + Math.abs(stream.offset) * 0.36);
      const cp1x = midX;
      const cp1y = baseY + wave;
      const cp2x = endX + (stream.side < 0 ? 18 : -18);
      const cp2y = baseY - wave * 0.55;
      const grad = ctx.createLinearGradient(startX, baseY, endX, baseY);

      if (stream.side < 0) {
        grad.addColorStop(0, `rgba(${stream.col.r},${stream.col.g},${stream.col.b},0)`);
        grad.addColorStop(0.42, `rgba(${stream.col.r},${stream.col.g},${stream.col.b},0.26)`);
        grad.addColorStop(0.86, `rgba(${stream.destCol.r},${stream.destCol.g},${stream.destCol.b},0.18)`);
        grad.addColorStop(1, `rgba(${stream.destCol.r},${stream.destCol.g},${stream.destCol.b},0)`);
      } else {
        grad.addColorStop(0, `rgba(${stream.col.r},${stream.col.g},${stream.col.b},0)`);
        grad.addColorStop(0.16, `rgba(${stream.col.r},${stream.col.g},${stream.col.b},0.18)`);
        grad.addColorStop(0.62, `rgba(${stream.destCol.r},${stream.destCol.g},${stream.destCol.b},0.28)`);
        grad.addColorStop(1, `rgba(${stream.destCol.r},${stream.destCol.g},${stream.destCol.b},0)`);
      }

      ctx.beginPath();
      ctx.moveTo(startX, baseY);
      ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, endX, baseY + stream.offset * 0.08);
      ctx.strokeStyle = grad;
      ctx.lineWidth = stream.thickness;
      ctx.lineCap = 'round';
      ctx.stroke();
    });

    const halo = ctx.createRadialGradient(orbX, orbY, 0, orbX, orbY, glowRadius);
    const haloAlpha = 0.08 + Math.sin(t * pulseRate) * 0.022;
    halo.addColorStop(0, `rgba(${col},${haloAlpha * 2})`);
    halo.addColorStop(0.5, `rgba(${col},${haloAlpha * 0.82})`);
    halo.addColorStop(1, `rgba(${col},0)`);
    ctx.fillStyle = halo;
    ctx.beginPath();
    ctx.arc(orbX, orbY, glowRadius, 0, Math.PI * 2);
    ctx.fill();

    const projected = COLLAPSE_ICO_VERTS.map(v =>
      projectCollapseOrb(v, rot.x, rot.y, rot.z, scale, orbX, orbY)
    );

    COLLAPSE_ICO_FACES
      .map((face, index) => ({
        face,
        index,
        z: (projected[face[0]].z + projected[face[1]].z + projected[face[2]].z) / 3,
      }))
      .sort((a, b) => a.z - b.z)
      .forEach(({ face, index, z }) => {
        const [a, b, c] = face.map(vertexIndex => projected[vertexIndex]);
        const brightness = 0.18 + ((z + 1) / 2) * 0.5;

        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.lineTo(c.x, c.y);
        ctx.closePath();
        ctx.fillStyle = getCollapseOrbFaceColor(index, avec, brightness);
        ctx.fill();
        ctx.strokeStyle = `rgba(255,255,255,${0.08 + ((z + 1) / 2) * 0.14})`;
        ctx.lineWidth = 0.62;
        ctx.stroke();
      });

    const coreRadius = scale * 0.22;
    const core = ctx.createRadialGradient(orbX - coreRadius * 0.3, orbY - coreRadius * 0.3, 0, orbX, orbY, coreRadius);
    const coreAlpha = 0.74 + Math.sin(t * pulseRate * 2) * 0.14;
    core.addColorStop(0, `rgba(255,255,255,${coreAlpha})`);
    core.addColorStop(0.4, `rgba(${col},${coreAlpha * 0.82})`);
    core.addColorStop(1, `rgba(${col},0)`);
    ctx.fillStyle = core;
    ctx.beginPath();
    ctx.arc(orbX, orbY, coreRadius, 0, Math.PI * 2);
    ctx.fill();

    ctx.beginPath();
    ctx.arc(orbX - 8, orbY - 8, 5.8, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${Math.min(255, Math.round(r + 38))},${Math.min(255, Math.round(g + 38))},${Math.min(255, Math.round(b + 38))},0.5)`;
    ctx.fill();

    [
      { x: -42, y: -22, radius: 1.8, alpha: 0.16, speed: 1.1 },
      { x: 34, y: 18, radius: 1.5, alpha: 0.13, speed: 0.9 },
      { x: 46, y: -12, radius: 1.25, alpha: 0.1, speed: 1.3 },
    ].forEach(mote => {
      const mx = orbX + mote.x + Math.cos(t * mote.speed + mote.y * 0.02) * 3;
      const my = orbY + mote.y + Math.sin(t * mote.speed + mote.x * 0.02) * 2.4;
      ctx.beginPath();
      ctx.arc(mx, my, mote.radius + Math.sin(t * 1.35 + mote.x * 0.03) * 0.2, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(${col},${mote.alpha})`;
      ctx.fill();
    });

    ctx.restore();

  }

  function drawHints() {
    if (loading || telescopeCameraEngaged) return;
    ctx.textAlign = 'center';
    ctx.font      = `10px ${FONT_MONO}`;
    const fade = 0.12 + Math.sin(t * 0.7) * 0.06;
    if (level === 0) {
      ctx.fillStyle = `rgba(255,255,255,${fade})`;
      ctx.fillText('pan to explore · tap a wave to enter', W() / 2, H() - 30);
    } else if (level === 1) {
      ctx.fillStyle = `rgba(255,255,255,${fade})`;
      ctx.fillText('tap a moment to descend · tap empty to surface', W() / 2, H() - 30);
    } else if (level === 2 && !cardVisible) {
      ctx.fillStyle = `rgba(255,255,255,${fade * 0.7})`;
      ctx.fillText('tap anywhere to surface', W() / 2, H() - 30);
    }
  }

  function drawNegativeLayerVignette() {
    if (!negativeLayerActive || level !== 0 || telescopeCameraEngaged) return;

    const centerX = W() / 2;
    const centerY = H() / 2;
    const radius = Math.max(W(), H()) * 0.84;
    const pulse = 0.012 + Math.sin(t * 0.34) * 0.004;

    const vignette = ctx.createRadialGradient(centerX, centerY, radius * 0.38, centerX, centerY, radius);
    vignette.addColorStop(0, 'rgba(2, 4, 8, 0)');
    vignette.addColorStop(0.7, 'rgba(4, 8, 16, 0.08)');
    vignette.addColorStop(1, `rgba(3, 7, 14, ${0.19 + pulse})`);
    ctx.fillStyle = vignette;
    ctx.fillRect(0, 0, W(), H());

    const halo = ctx.createRadialGradient(centerX, centerY, radius * 0.5, centerX, centerY, radius);
    halo.addColorStop(0, 'rgba(140, 184, 245, 0)');
    halo.addColorStop(0.88, 'rgba(140, 184, 245, 0.022)');
    halo.addColorStop(1, 'rgba(140, 184, 245, 0.055)');
    ctx.fillStyle = halo;
    ctx.fillRect(0, 0, W(), H());
  }

  function cameraIsMoving() {
    return Math.abs(targetCamX - camX) > CAMERA_POS_EPSILON
      || Math.abs(targetCamY - camY) > CAMERA_POS_EPSILON
      || Math.abs(targetCamScale - camScale) > CAMERA_SCALE_EPSILON;
  }

  function frameStepForNow(now: number) {
    const recentlyInteractive = now - lastInteractionAt < RECENT_INTERACTION_WINDOW_MS;
    const highActivity = recentlyInteractive
      || dragging
      || telescopeDragY !== null
      || telescopePhase !== 'idle'
      || cameraIsMoving();

    return highActivity ? ACTIVE_FRAME_STEP_MS : IDLE_FRAME_STEP_MS;
  }

  function handleVisibilityChange() {
    if (typeof document === 'undefined') {
      return;
    }

    if (document.hidden) {
      stopRenderLoop();
      return;
    }

    noteInteraction();
    lastFrameDrawAt = 0;
    startRenderLoop();
  }

  // ── Main render loop ────────────────────────────────────────────
  function draw(frameTime = 0) {
    raf = 0;

    if (!ctx) {
      startRenderLoop();
      return;
    }

    const now = frameTime > 0 ? frameTime : nowMs();
    const frameStep = frameStepForNow(now);
    if (lastFrameDrawAt > 0 && now - lastFrameDrawAt < frameStep) {
      startRenderLoop();
      return;
    }

    const elapsed = lastFrameDrawAt > 0 ? now - lastFrameDrawAt : frameStep;
    const tickScale = Math.max(0.5, Math.min(2.5, elapsed / ACTIVE_FRAME_STEP_MS));
    lastFrameDrawAt = now;

    camX     += (targetCamX     - camX)     * LERP;
    camY     += (targetCamY     - camY)     * LERP;
    camScale += (targetCamScale - camScale) * LERP;

    const driftEnabled = negativeLayerActive && level === 0 && !telescopeCameraEngaged && !dragging;
    const driftPx = compactViewport() ? 7.5 : 10.5;
    const targetDriftX = driftEnabled
      ? Math.sin(t * 0.16 + 0.8) * (driftPx / Math.max(camScale, 0.0001))
      : 0;
    const targetDriftY = driftEnabled
      ? Math.cos(t * 0.12 + 1.7) * ((driftPx * 0.58) / Math.max(camScale, 0.0001))
      : 0;
    const driftLerp = driftEnabled ? 0.045 : 0.08;
    negativeLayerDriftX += (targetDriftX - negativeLayerDriftX) * driftLerp;
    negativeLayerDriftY += (targetDriftY - negativeLayerDriftY) * driftLerp;

    if (!driftEnabled && Math.abs(negativeLayerDriftX) < 0.0008 && Math.abs(negativeLayerDriftY) < 0.0008) {
      negativeLayerDriftX = 0;
      negativeLayerDriftY = 0;
    }

    settleTelescopeTransition();

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(deviceScale, 0, 0, deviceScale, 0, 0);
    ctx.fillStyle = '#0a0b0e';
    ctx.fillRect(0, 0, W(), H());

    drawStars();

    ctx.save();
    ctx.translate(W() / 2, H() / 2);
    ctx.scale(camScale, camScale);
    ctx.translate(-activeCamX(), -activeCamY());
    drawEdges();
    drawSessions();
    drawWaveBoundary();
    drawWaveThreads();
    drawNodes();
    ctx.restore();

    drawWaveLabels();
    drawCollapseOrb();
  drawNegativeLayerVignette();
    drawHints();

    if (loading) {
      ctx.textAlign = 'center';
      ctx.font      = `11px ${FONT_MONO}`;
      ctx.fillStyle = `rgba(255,255,255,${0.2 + Math.sin(t * 2) * 0.07})`;
      ctx.fillText('reading the terrain…', W() / 2, H() / 2);
    }
    if (error && !loading) {
      ctx.textAlign = 'center';
      ctx.font      = `11px ${FONT_MONO}`;
      ctx.fillStyle = 'rgba(233,148,58,0.65)';
      ctx.fillText(error.slice(0, 80), W() / 2, H() / 2);
    }

    t += 0.01 * tickScale;
    startRenderLoop();
  }

  // ── Resize ───────────────────────────────────────────────────
  function resize() {
    if (!canvas || !container) return;
    noteInteraction();
    const rect = container.getBoundingClientRect();
    const w = Math.round(Math.max(rect.width, window.innerWidth));
    const h = Math.round(Math.max(rect.height, window.innerHeight));
    if (w === 0 || h === 0) return;
    viewportWidth = w;
    viewportHeight = h;
    deviceScale = Math.max(window.devicePixelRatio || 1, 1);
    canvas.style.width = `${w}px`;
    canvas.style.height = `${h}px`;
    canvas.width  = Math.round(w * deviceScale);
    canvas.height = Math.round(h * deviceScale);
    ctx?.setTransform(deviceScale, 0, 0, deviceScale, 0, 0);
    layoutConstellation();
    if (telescopePhase === 'entering' && telescopeCameraBefore) {
      const focus = telescopeFocusTargetFrom(telescopeCameraBefore, level);
      targetCamX = focus.x;
      targetCamY = focus.y;
      targetCamScale = focus.scale;
    }

    if (!telescopeCameraEngaged && level === 0) {
      camX = targetCamX = W() / 2;
      camY = targetCamY = H() / 2;
      camScale = targetCamScale = constellationLayerScale();
    }
  }

  // ── Pointer events ─────────────────────────────────────────────
  function onPointerDown(e: PointerEvent) {
    if (telescopeCameraEngaged || level !== 0) return;
    if (e.pointerType === 'mouse' && e.button !== 0) return;

    noteInteraction();

    if (negativeLayerDriftX !== 0 || negativeLayerDriftY !== 0) {
      camX += negativeLayerDriftX;
      camY += negativeLayerDriftY;
      targetCamX += negativeLayerDriftX;
      targetCamY += negativeLayerDriftY;
      negativeLayerDriftX = 0;
      negativeLayerDriftY = 0;
    }

    dragging    = true;
    didDrag     = false;
    activePanPointerId = e.pointerId;
    dragStart   = { x: e.clientX, y: e.clientY };
    panCamStart = { x: camX, y: camY };
    canvas.setPointerCapture?.(e.pointerId);
    e.preventDefault();
  }

  function onPointerMove(e: PointerEvent) {
    if (activePanPointerId !== null && e.pointerId !== activePanPointerId) return;
    if (telescopeCameraEngaged || !dragging || level !== 0) return;

    noteInteraction();
    const dx = e.clientX - dragStart.x;
    const dy = e.clientY - dragStart.y;
    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) didDrag = true;
    camX = targetCamX = panCamStart.x - dx / camScale;
    camY = targetCamY = panCamStart.y - dy / camScale;
    e.preventDefault();
  }

  function onPointerUp(e: PointerEvent) {
    if (activePanPointerId !== null && e.pointerId !== activePanPointerId) return;
    if (activePanPointerId !== null && canvas.hasPointerCapture?.(activePanPointerId)) {
      canvas.releasePointerCapture(activePanPointerId);
    }
    activePanPointerId = null;
    dragging = false;
    noteInteraction();
    if (telescopeCameraEngaged) return;
    if (didDrag) return;

    const { x: sx, y: sy } = canvasXY(e);

    if (level === 2) { surfaceToWave(); return; }

    if (level === 1 && selectedSession) {
      for (const n of selectedSessionNodes) {
        const np = nodeRenderPos(n);
        if (!np) continue;
        const sc = toScreen(np.x, np.y);
        if (Math.hypot(sx - sc.x, sy - sc.y) < nodeHitRadius(n)) {
          descendToCollapse(n);
          return;
        }
      }
      surfaceToConstellation();
      return;
    }

    if (level === 0 && graph) {
      for (const s of graph.sessions) {
        const sp = sessionRenderPos(s);
        if (!sp) continue;
        const sc = toScreen(sp.x, sp.y);
        if (Math.hypot(sx - sc.x, sy - sc.y) < sessionHitRadius(s)) {
          descendToWave(s);
          return;
        }
      }
    }
  }

  function onPointerCancel(e: PointerEvent) {
    if (activePanPointerId !== null && e.pointerId !== activePanPointerId) return;
    if (activePanPointerId !== null && canvas.hasPointerCapture?.(activePanPointerId)) {
      canvas.releasePointerCapture(activePanPointerId);
    }
    activePanPointerId = null;
    noteInteraction();
    dragging = false;
    didDrag = false;
  }

  // ── Health ───────────────────────────────────────────────────
  let healthy = false;

  function sourceFromTransport(transport: string): { tone: 'unknown' | 'local' | 'cloud' | 'mem'; label: string } {
    const normalized = transport.toLowerCase();
    if (normalized.includes('mem fallback') || normalized.includes('in-memory fallback')) {
      return { tone: 'mem', label: 'source: mem fallback' };
    }
    if (normalized.includes('surrealdb')) {
      return { tone: 'local', label: 'source: local db' };
    }

    return { tone: 'unknown', label: 'source: unknown' };
  }

  function applySourceBadge(source?: string | null, transport?: string | null) {
    const normalizedSource = (source ?? '').trim().toLowerCase();
    const normalizedTransport = (transport ?? '').trim();

    if (normalizedSource === 'cloud-gateway') {
      sourceBadgeTone = 'cloud';
      sourceBadgeLabel = 'source: cloud';
    } else if (normalizedSource === 'fallback-cache') {
      sourceBadgeTone = 'mem';
      sourceBadgeLabel = 'source: cache fallback';
    } else if (normalizedSource === 'surrealdb-mem') {
      sourceBadgeTone = 'mem';
      sourceBadgeLabel = 'source: mem fallback';
    } else if (normalizedSource === 'surrealdb-local') {
      sourceBadgeTone = 'local';
      sourceBadgeLabel = 'source: local db';
    } else if (normalizedTransport) {
      const inferred = sourceFromTransport(normalizedTransport);
      sourceBadgeTone = inferred.tone;
      sourceBadgeLabel = inferred.label;
    } else {
      sourceBadgeTone = 'unknown';
      sourceBadgeLabel = 'source: unknown';
    }

    sourceBadgeTitle = `read path: ${source ?? 'n/a'} · transport: ${transport ?? 'n/a'}`;
  }

  async function checkHealth() {
    try {
      const health: HealthResponse = await resonantiaClient.getHealth();
      healthy = true;
      lastTransportLabel = health.transport;
      if (sourceBadgeTone === 'unknown' || sourceBadgeLabel === 'source: pending') {
        applySourceBadge(undefined, health.transport);
      }
    }
    catch { healthy = false; }
  }

  // ── Menu / Settings ───────────────────────────────────────
  let menuOpen = false;
  let settingsOpen = false;
  let settingsLoading = false;
  let settingsSaving = false;
  let settingsError: string | null = null;
  let settingsSaved = false;
  let ollamaBaseUrl = '';
  let ollamaModel = '';
  let gatewayBaseUrl = '';
  let gatewayAuthToken = '';
  let cloudAuthAvailable = false;
  let cloudAuthSignedIn = false;
  let cloudAuthBusy = false;
  let cloudAuthStatus = 'cloud account disconnected';
  let cloudAuthError: string | null = null;
  let cloudAccountTier: string | null = null;
  let cloudAccountMemberSince: string | null = null;
  let advancedOpen = false;
  let localModelOriginWarning: string | null = null;
  const MANAGED_GATEWAY_BASE_URL = getManagedGatewayBaseUrl().trim();

  function normalizeGatewayForCompare(value: string) {
    return value.trim().replace(/\/+$/, '');
  }

  function isManagedGatewayBaseUrl(value: string) {
    const normalized = normalizeGatewayForCompare(value);
    const managed = normalizeGatewayForCompare(MANAGED_GATEWAY_BASE_URL);
    if (!managed) {
      return normalized.length === 0;
    }
    return normalized.length === 0 || normalized === managed;
  }

  function resolveEffectiveGatewayBaseUrl(inputValue: string) {
    const trimmed = inputValue.trim();
    return trimmed || MANAGED_GATEWAY_BASE_URL;
  }

  function displayGatewayInputFromConfig(configured: string) {
    return isManagedGatewayBaseUrl(configured) ? '' : configured;
  }

  function hasPaidCloudTier(tier: string | null) {
    return tier === 'resonant' || tier === 'soulful';
  }

  function isLoopbackHostName(hostname: string) {
    const normalized = hostname.trim().toLowerCase();
    return normalized === 'localhost' || normalized === '127.0.0.1' || normalized === '::1' || normalized === '[::1]';
  }

  function localModelWarningForCurrentOrigin(baseUrl: string): string | null {
    if (typeof window === 'undefined') {
      return null;
    }

    const trimmed = baseUrl.trim();
    if (!trimmed) {
      return null;
    }

    try {
      const target = new URL(trimmed);
      if (!isLoopbackHostName(target.hostname) || isLoopbackHostName(window.location.hostname)) {
        return null;
      }

      const mixedContentBlocked = window.location.protocol === 'https:' && target.protocol === 'http:';
      const mixedContentNote = mixedContentBlocked ? ' HTTPS pages also block plain-http localhost requests.' : '';
      return `Hosted origin ${window.location.origin} may not reach local model endpoint ${target.origin} due browser security policy.${mixedContentNote}`;
    } catch {
      return null;
    }
  }

  $: localModelOriginWarning = localModelWarningForCurrentOrigin(ollamaBaseUrl);

  async function openSettings() {
    markWalkthroughStepSatisfied('settings');
    menuOpen = false;
    settingsOpen = true;
    settingsLoading = true;
    settingsError = null;
    settingsSaved = false;

    try {
      const config = await resonantiaClient.getConfig();
      gatewayBaseUrl = displayGatewayInputFromConfig(config.gatewayBaseUrl ?? '');
      gatewayAuthToken = config.gatewayAuthToken ?? '';
      ollamaBaseUrl = config.ollamaBaseUrl;
      ollamaModel = config.ollamaModel;
      advancedOpen = false;
      await refreshCloudAuthState();
    } catch (err) {
      settingsError = String(err);
    } finally {
      settingsLoading = false;
    }
  }

  async function refreshCloudAuthState() {
    try {
      cloudAuthError = null;
      const status = await getCloudAuthStatus();
      cloudAuthAvailable = status.available;
      cloudAuthSignedIn = status.signedIn;

      if (!status.available) {
        cloudAuthStatus = 'cloud connect is not enabled in this build';
        return;
      }

      if (status.signedIn) {
        const marker = status.username ?? (status.userId ? status.userId.slice(0, 8) : null);
        cloudAuthStatus = `cloud account connected${marker ? ` · ${marker}` : ''}`;
        const effectiveGatewayBaseUrl = resolveEffectiveGatewayBaseUrl(gatewayBaseUrl);
        const account = await getCloudAccount(effectiveGatewayBaseUrl, gatewayAuthToken).catch(() => null);
        cloudAccountTier = account?.tier ?? null;
        cloudAccountMemberSince = account?.memberSince ?? null;
      } else {
        cloudAuthStatus = 'cloud account disconnected';
        cloudAccountTier = null;
        cloudAccountMemberSince = null;
      }
    } catch (err) {
      cloudAuthAvailable = false;
      cloudAuthSignedIn = false;
      cloudAuthStatus = 'cloud auth unavailable';
      cloudAuthError = String(err);
    }
  }

  async function connectCloudAccount() {
    if (cloudAuthBusy) {
      return;
    }

    cloudAuthBusy = true;
    cloudAuthError = null;
    try {
      // Redirect to Clerk's hosted sign-in; returns to current URL after auth.
      // (openSignIn modal requires the full Clerk UI bundle which is unavailable
      // in the @clerk/clerk-js npm/ESM build — only the CDN script includes it.)
      await redirectToCloudSignIn(window.location.href);
    } catch (err) {
      cloudAuthError = String(err);
      cloudAuthBusy = false;
    }
    // page navigates on success — finally would reset busy state prematurely
  }

  async function refreshGatewayAuthToken() {
    if (cloudAuthBusy) {
      return;
    }

    cloudAuthBusy = true;
    cloudAuthError = null;
    settingsSaved = false;
    try {
      const token = await getGatewayAuthToken();
      gatewayAuthToken = token;
      await resonantiaClient.setGatewayAuthToken(token);
      await refreshCloudAuthState();
      settingsSaved = true;
    } catch (err) {
      cloudAuthError = String(err);
    } finally {
      cloudAuthBusy = false;
    }
  }

  async function refreshGatewayAuthTokenForSync() {
    const status = await getCloudAuthStatus();
    if (!status.available || !status.signedIn) {
      return;
    }

    const token = await getGatewayAuthToken();
    if (!token || token === gatewayAuthToken) {
      return;
    }

    gatewayAuthToken = token;
    await resonantiaClient.setGatewayAuthToken(token);
  }

  async function clearGatewayAuthToken() {
    if (cloudAuthBusy) {
      return;
    }

    cloudAuthBusy = true;
    cloudAuthError = null;
    settingsSaved = false;
    try {
      gatewayAuthToken = '';
      await resonantiaClient.setGatewayAuthToken('');
      await signOutCloud();
      await refreshCloudAuthState();
      settingsSaved = true;
    } catch (err) {
      cloudAuthError = String(err);
    } finally {
      cloudAuthBusy = false;
    }
  }

  async function saveSettings() {
    settingsSaving = true;
    settingsError = null;
    settingsSaved = false;

    try {
      const gatewayInput = gatewayBaseUrl.trim();
      await resonantiaClient.setOllamaConfig(ollamaBaseUrl.trim(), ollamaModel.trim());
      await resonantiaClient.setGatewayBaseUrl(gatewayInput);
      await resonantiaClient.setGatewayAuthToken(gatewayAuthToken.trim());

      settingsSaved = true;
      gatewayBaseUrl = displayGatewayInputFromConfig(gatewayInput);
      await refreshCloudAuthState();
      await checkHealth();
      await loadGraph();
    } catch (err) {
      settingsError = String(err);
    } finally {
      settingsSaving = false;
    }
  }

  // ── Compose ──────────────────────────────────────────────────
  let composeOpen     = false;
  let composeModeMenuOpen = false;
  let composeMode: 'live' | 'importare' = 'live';
  let composeDraft    = '';
  type ComposeMessage = {
    role: 'user' | 'assistant';
    content: string;
    at: string;
  };
  let composeMessages: ComposeMessage[] = [];
  let composeSessionId = '';
  let composeLoading  = false;
  let composeReplyLoading = false;
  let composeEncodePromptSent = false;
  let composeError: string | null = null;
  let composeResult: { psi: number; duplicateSkipped: boolean; status: 'created' | 'updated' | 'duplicate' | 'skipped' } | null = null;
  let composePromptCopyLoading = false;
  let composePromptCopied = false;
  let composePromptCopiedTimer: ReturnType<typeof setTimeout> | null = null;
  let composePromptCopyError: string | null = null;
  let composePasteNodeOpen = false;
  let composePasteNodeDraft = '';
  let composePasteNodeLoading = false;
  let syncPullLoading = false;
  let syncPullError: string | null = null;
  let syncPullResult: SyncNowResponse | null = null;
  let syncButtonAria = 'sync with cloud';
  let syncVisualState: 'idle' | 'syncing' | 'success' | 'error' = 'idle';
  let syncDetailHover = false;
  let syncDetailAutoOpen = false;
  let syncDetailVisible = false;
  let syncDetailTitle = 'standing by';
  let syncDetailSubtitle = '';
  let syncDetailTimeLabel = 'never';
  let syncDetailTimestamp: Date | null = null;
  let syncDetailTimer: ReturnType<typeof setTimeout> | null = null;

  type TelescopeRange = {
    label: string;
    days: number;
  };

  const TELESCOPE_RANGES: TelescopeRange[] = [
    { label: 'last 7 days', days: 7 },
    { label: 'last 2 weeks', days: 14 },
    { label: 'last month', days: 30 },
    { label: 'last 3 months', days: 90 },
    { label: 'all time', days: Number.MAX_SAFE_INTEGER },
  ];

  const TELESCOPE_DIAL_MAX = 95;
  const TELESCOPE_DIAL_STEP = 20;
  const DAY_MS = 24 * 60 * 60 * 1000;
  const TELESCOPE_CONSTELLATION_SCALE = 1.04;
  const TELESCOPE_WAVE_SCALE = 2.05;
  const TELESCOPE_CAMERA_POS_EPSILON = 1.5;
  const TELESCOPE_CAMERA_SCALE_EPSILON = 0.035;

  type TelescopePhase = 'idle' | 'entering' | 'exiting';
  type CameraState = { x: number; y: number; scale: number };

  let telescopeOpen = false;
  let telescopeDialPct = 0;
  let telescopeDragY: number | null = null;
  let telescopeDragBasePct = 0;
  let telescopeRangeIndex = 0;
  let telescopeRangeLabel = TELESCOPE_RANGES[0].label;
  let telescopeDialOffsetY = 0;
  let telescopeTimelineSessions: GraphSessionDto[] = [];
  let telescopePhase: TelescopePhase = 'idle';
  let telescopeCameraBefore: CameraState | null = null;

  $: telescopeCanAccess = level === 0 || level === 1;
  $: telescopeCameraEngaged = telescopeOpen || telescopePhase !== 'idle';

  function clearSyncDetailTimer() {
    if (syncDetailTimer !== null) {
      clearTimeout(syncDetailTimer);
      syncDetailTimer = null;
    }
  }

  function scheduleSyncDetailClose(delayMs = 5200) {
    clearSyncDetailTimer();
    syncDetailTimer = setTimeout(() => {
      syncDetailAutoOpen = false;
      syncDetailTimer = null;
    }, delayMs);
  }

  function compactSyncError(message: string) {
    const clean = message.trim();
    return clean.length > 92 ? `${clean.slice(0, 89)}...` : clean;
  }

  function openSyncDetailHover() {
    syncDetailHover = true;
    clearSyncDetailTimer();
  }

  function closeSyncDetailHover() {
    syncDetailHover = false;
    if (!syncPullLoading && syncDetailAutoOpen) {
      scheduleSyncDetailClose(1200);
    }
  }

  $: syncDetailVisible = syncDetailHover || syncDetailAutoOpen;
  $: syncDetailTimeLabel = syncDetailTimestamp
    ? syncDetailTimestamp.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    : 'never';

  $: {
    if (syncPullLoading) {
      syncVisualState = 'syncing';
      syncButtonAria = 'syncing with cloud';
      syncDetailTitle = 'syncing';
      syncDetailSubtitle = 'aligning local + cloud';
    } else if (syncPullError) {
      syncVisualState = 'error';
      syncButtonAria = 'sync failed';
      syncDetailTitle = 'sync issue';
      syncDetailSubtitle = compactSyncError(syncPullError);
    } else if (syncPullResult) {
      syncVisualState = 'success';
      syncButtonAria = 'sync complete';
      syncDetailTitle = 'synced';
      syncDetailSubtitle = syncPullResult.sessionId === 'all'
        ? 'all waves'
        : syncPullResult.sessionId;
    } else {
      syncVisualState = 'idle';
      syncButtonAria = 'sync with cloud';
      syncDetailTitle = 'standing by';
      syncDetailSubtitle = '';
    }
  }

  function telescopeRangeIndexForDial(dialPct: number) {
    return Math.max(0, Math.min(TELESCOPE_RANGES.length - 1, Math.floor(dialPct / TELESCOPE_DIAL_STEP)));
  }

  function snapTelescopeDialPct(dialPct: number) {
    const snapped = Math.round(dialPct / TELESCOPE_DIAL_STEP) * TELESCOPE_DIAL_STEP;
    return Math.max(0, Math.min(TELESCOPE_DIAL_STEP * (TELESCOPE_RANGES.length - 1), snapped));
  }

  function daysAgoFromTimestamp(timestamp: string) {
    const parsed = Date.parse(timestamp);
    if (!Number.isFinite(parsed)) {
      return Number.MAX_SAFE_INTEGER;
    }

    const diffMs = Math.max(0, Date.now() - parsed);
    return Math.floor(diffMs / DAY_MS);
  }

  function listTelescopeSessions(sessions: GraphSessionDto[], maxDays: number) {
    return sessions
      .filter((session) => daysAgoFromTimestamp(session.lastModified) <= maxDays)
      .sort((left, right) => right.lastModified.localeCompare(left.lastModified));
  }

  function telescopeSessionDateLabel(timestamp: string) {
    const ageDays = daysAgoFromTimestamp(timestamp);
    if (ageDays === 0) return 'today';
    if (ageDays === 1) return 'yesterday';

    const parsed = Date.parse(timestamp);
    if (!Number.isFinite(parsed)) {
      return 'unknown';
    }

    return new Date(parsed).toLocaleDateString([], { month: 'short', day: 'numeric' });
  }

  function telescopeSessionMeta(session: GraphSessionDto) {
    return `${session.nodeCount} nodes · Ψ ${session.avgPsi.toFixed(2)}`;
  }

  function telescopeSessionColor(session: GraphSessionDto, alpha = 0.9) {
    return fieldAvecColor(sessionAvec(session), alpha);
  }

  function cameraAtTarget(positionEpsilon = TELESCOPE_CAMERA_POS_EPSILON, scaleEpsilon = TELESCOPE_CAMERA_SCALE_EPSILON) {
    return Math.hypot(targetCamX - camX, targetCamY - camY) <= positionEpsilon
      && Math.abs(targetCamScale - camScale) <= scaleEpsilon;
  }

  function telescopeAnchorScreenPoint() {
    const compact = W() <= 520;
    return {
      x: W() / 2,
      y: H() / 2 + (compact ? 10 : 16),
    };
  }

  function worldAtScreenForCamera(screenX: number, screenY: number, camera: CameraState) {
    return {
      x: (screenX - W() / 2) / camera.scale + camera.x,
      y: (screenY - H() / 2) / camera.scale + camera.y,
    };
  }

  function telescopeFocusScaleFor(levelValue: number, cameraScale: number) {
    const compact = compactViewport();

    if (levelValue === 1) {
      // Entering telescope from wave should pull back for context, not punch in.
      return Math.max(compact ? 1.55 : 1.82, Math.min(TELESCOPE_WAVE_SCALE, cameraScale * (compact ? 0.66 : 0.72)));
    }

    // In constellation, use a subtle pull-back so the telescope stage feels cinematic.
    return Math.max(compact ? 0.84 : 0.92, Math.min(TELESCOPE_CONSTELLATION_SCALE, cameraScale * (compact ? 0.82 : 0.86)));
  }

  function telescopeFocusTargetFrom(camera: CameraState, levelValue: number) {
    const anchor = telescopeAnchorScreenPoint();
    const world = worldAtScreenForCamera(anchor.x, anchor.y, camera);
    const scale = telescopeFocusScaleFor(levelValue, camera.scale);
    return {
      x: world.x,
      y: world.y,
      scale,
    };
  }

  function beginTelescopeEnterTransition() {
    const startCamera = { x: camX, y: camY, scale: camScale };
    telescopeCameraBefore = startCamera;
    const focus = telescopeFocusTargetFrom(startCamera, level);
    targetCamX = focus.x;
    targetCamY = focus.y;
    targetCamScale = focus.scale;
    telescopePhase = 'entering';
  }

  function beginTelescopeExitTransition() {
    telescopeOpen = false;
    telescopeDragY = null;

    if (!telescopeCameraBefore) {
      telescopePhase = 'idle';
      return;
    }

    targetCamX = telescopeCameraBefore.x;
    targetCamY = telescopeCameraBefore.y;
    targetCamScale = telescopeCameraBefore.scale;
    telescopePhase = 'exiting';
  }

  function settleTelescopeTransition() {
    if (telescopePhase === 'entering' && cameraAtTarget()) {
      telescopePhase = 'idle';
      telescopeOpen = true;
      return;
    }

    if (telescopePhase === 'exiting' && cameraAtTarget()) {
      telescopePhase = 'idle';
      telescopeCameraBefore = null;
    }
  }

  function toggleNegativeLayer() {
    if (level !== 0 || telescopeCameraEngaged) {
      return;
    }

    noteInteraction();
    negativeLayerActive = !negativeLayerActive;
    targetCamScale = constellationLayerScale();
  }

  function openTelescope() {
    if (!telescopeCanAccess || telescopeCameraEngaged) {
      return;
    }

    noteInteraction();
    markWalkthroughStepSatisfied('telescope');

    closeTransientUi();
    syncDetailAutoOpen = false;
    syncDetailHover = false;
    beginTelescopeEnterTransition();
  }

  function closeTelescope() {
    if (!telescopeCameraEngaged) {
      return;
    }

    noteInteraction();
    beginTelescopeExitTransition();
  }

  function handleTelescopeClosePointerDown(event: PointerEvent) {
    event.preventDefault();
    closeTelescope();
  }

  function selectTelescopeSession(session: GraphSessionDto) {
    telescopeOpen = false;
    telescopePhase = 'idle';
    telescopeDragY = null;
    telescopeCameraBefore = null;
    descendToWave(session);
  }

  function beginTelescopeDial(clientY: number) {
    noteInteraction();
    telescopeDragY = clientY;
    telescopeDragBasePct = telescopeDialPct;
  }

  function updateTelescopeDial(clientY: number) {
    if (telescopeDragY === null) {
      return;
    }

    noteInteraction();
    const delta = telescopeDragY - clientY;
    telescopeDialPct = Math.max(0, Math.min(TELESCOPE_DIAL_MAX, telescopeDragBasePct + delta * 0.55));
  }

  function endTelescopeDial() {
    if (telescopeDragY === null) {
      return;
    }

    telescopeDragY = null;
    telescopeDialPct = snapTelescopeDialPct(telescopeDialPct);
  }

  function handleTelescopeDialMouseDown(event: MouseEvent) {
    event.preventDefault();
    beginTelescopeDial(event.clientY);
  }

  function handleTelescopeDialTouchStart(event: TouchEvent) {
    const touch = event.touches[0];
    if (!touch) {
      return;
    }

    event.preventDefault();
    beginTelescopeDial(touch.clientY);
  }

  function handleTelescopeDialMouseMove(event: MouseEvent) {
    updateTelescopeDial(event.clientY);
  }

  function handleTelescopeDialTouchMove(event: TouchEvent) {
    if (telescopeDragY === null) {
      return;
    }

    const touch = event.touches[0];
    if (!touch) {
      return;
    }

    event.preventDefault();
    updateTelescopeDial(touch.clientY);
  }

  function handleTelescopeDialKeydown(event: KeyboardEvent) {
    if (event.key === 'ArrowUp') {
      event.preventDefault();
      telescopeDialPct = Math.min(TELESCOPE_DIAL_MAX, telescopeDialPct + 8);
      return;
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault();
      telescopeDialPct = Math.max(0, telescopeDialPct - 8);
      return;
    }

    if (event.key === 'Home') {
      event.preventDefault();
      telescopeDialPct = 0;
      return;
    }

    if (event.key === 'End') {
      event.preventDefault();
      telescopeDialPct = TELESCOPE_DIAL_STEP * (TELESCOPE_RANGES.length - 1);
    }
  }

  function handleTelescopeEyeKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      closeTelescope();
    }
  }

  $: telescopeRangeIndex = telescopeRangeIndexForDial(telescopeDialPct);
  $: telescopeRangeLabel = TELESCOPE_RANGES[telescopeRangeIndex].label;
  $: telescopeDialOffsetY = -4 + (telescopeDialPct / TELESCOPE_DIAL_MAX) * 14;
  $: telescopeTimelineSessions = listTelescopeSessions(graph?.sessions ?? [], TELESCOPE_RANGES[telescopeRangeIndex].days);
  $: if (!telescopeCanAccess && telescopeCameraEngaged && telescopePhase !== 'exiting') {
    beginTelescopeExitTransition();
  }

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

  const CALIBRATION_PROFILES: CalibrationProfile[] = [
    {
      id: 'explorer',
      label: 'Explorer',
      blurb: 'Curious, open, and led by discovery.',
      trigger: 'guided_explorer',
      values: { stability: 0.42, friction: 0.22, logic: 0.48, autonomy: 0.84 },
    },
    {
      id: 'planner',
      label: 'Planner',
      blurb: 'Organizing the field into clear next steps.',
      trigger: 'guided_planner',
      values: { stability: 0.76, friction: 0.34, logic: 0.86, autonomy: 0.58 },
    },
    {
      id: 'actor',
      label: 'Actor',
      blurb: 'Ready to commit, move, and adjust in motion.',
      trigger: 'guided_actor',
      values: { stability: 0.56, friction: 0.72, logic: 0.52, autonomy: 0.9 },
    },
    {
      id: 'anchor',
      label: 'Anchor',
      blurb: 'Settling the field before reaching outward.',
      trigger: 'guided_anchor',
      values: { stability: 0.88, friction: 0.18, logic: 0.64, autonomy: 0.36 },
    },
  ];

  const CALIBRATION_QUESTIONS: CalibrationQuestion[] = [
    {
      prompt: 'What would help most right now?',
      options: [
        {
          label: 'Room to explore',
          note: 'I need space to notice what is emerging.',
          values: { stability: 0.34, friction: 0.14, logic: 0.4, autonomy: 0.78 },
        },
        {
          label: 'A simple plan',
          note: 'I want the next steps to feel clear.',
          values: { stability: 0.72, friction: 0.28, logic: 0.86, autonomy: 0.5 },
        },
        {
          label: 'A push forward',
          note: 'I need momentum more than more thinking.',
          values: { stability: 0.48, friction: 0.78, logic: 0.5, autonomy: 0.9 },
        },
      ],
    },
    {
      prompt: 'When things feel unclear, what helps you most?',
      options: [
        {
          label: 'I can stay open',
          note: 'Uncertainty still feels creative and useful.',
          values: { stability: 0.44, friction: 0.2, logic: 0.42, autonomy: 0.8 },
        },
        {
          label: 'I want more clarity',
          note: 'Things will move once the picture sharpens.',
          values: { stability: 0.78, friction: 0.3, logic: 0.88, autonomy: 0.56 },
        },
        {
          label: 'I would rather act',
          note: 'The answer will show up once I begin.',
          values: { stability: 0.52, friction: 0.74, logic: 0.54, autonomy: 0.88 },
        },
      ],
    },
    {
      prompt: 'How do you want to make decisions today?',
      options: [
        {
          label: 'Follow what feels alive',
          note: 'I want curiosity to lead for a bit.',
          values: { stability: 0.38, friction: 0.24, logic: 0.46, autonomy: 0.82 },
        },
        {
          label: 'Compare and choose',
          note: 'I want to weigh things before committing.',
          values: { stability: 0.74, friction: 0.36, logic: 0.9, autonomy: 0.54 },
        },
        {
          label: 'Commit and adjust',
          note: 'I want to learn by moving.',
          values: { stability: 0.58, friction: 0.7, logic: 0.48, autonomy: 0.92 },
        },
      ],
    },
  ];

  const DEFAULT_CALIBRATION_ANSWERS = CALIBRATION_QUESTIONS.map(() => 0);

  function clamp01(value: number) {
    return Math.max(0, Math.min(1, value));
  }

  function calibrationDistance(left: CalibrationVector, right: CalibrationVector) {
    return Math.sqrt(
      (left.stability - right.stability) ** 2 +
      (left.friction - right.friction) ** 2 +
      (left.logic - right.logic) ** 2 +
      (left.autonomy - right.autonomy) ** 2,
    );
  }

  function averageCalibrationVectors(vectors: CalibrationVector[]): CalibrationVector {
    if (!vectors.length) {
      return { stability: 0.5, friction: 0.5, logic: 0.5, autonomy: 0.5 };
    }

    const totals = vectors.reduce(
      (acc, vector) => ({
        stability: acc.stability + vector.stability,
        friction: acc.friction + vector.friction,
        logic: acc.logic + vector.logic,
        autonomy: acc.autonomy + vector.autonomy,
      }),
      { stability: 0, friction: 0, logic: 0, autonomy: 0 },
    );

    return {
      stability: clamp01(totals.stability / vectors.length),
      friction: clamp01(totals.friction / vectors.length),
      logic: clamp01(totals.logic / vectors.length),
      autonomy: clamp01(totals.autonomy / vectors.length),
    };
  }

  function asAvecState(values: CalibrationVector) {
    return {
      ...values,
      psi: values.stability + values.friction + values.logic + values.autonomy,
    };
  }

  function calibrationSurfaceStyle(values: CalibrationVector, intensity = 1) {
    const avec = asAvecState(values);
    const glow = avecColor(avec, 0.16 * intensity);
    const edge = avecColor(avec, 0.3 * intensity);
    const wash = avecColor(avec, 0.07 * intensity);
    return `background: radial-gradient(circle at top left, ${glow}, transparent 62%), linear-gradient(160deg, ${wash}, rgba(255,255,255,0.02)); border-color: ${edge}; box-shadow: inset 0 0 0 1px ${avecColor(avec, 0.05 * intensity)};`;
  }

  function calibrationSpectrumStyle(values: CalibrationVector) {
    const total = Math.max(values.stability + values.friction + values.logic + values.autonomy, 0.001);
    const stabilityStop = (values.stability / total) * 100;
    const frictionStop = stabilityStop + (values.friction / total) * 100;
    const logicStop = frictionStop + (values.logic / total) * 100;
    return `background: linear-gradient(90deg, ${AVEC_HEX.stability} 0%, ${AVEC_HEX.stability} ${stabilityStop}%, ${AVEC_HEX.friction} ${stabilityStop}%, ${AVEC_HEX.friction} ${frictionStop}%, ${AVEC_HEX.logic} ${frictionStop}%, ${AVEC_HEX.logic} ${logicStop}%, ${AVEC_HEX.autonomy} ${logicStop}%, ${AVEC_HEX.autonomy} 100%);`;
  }

  function calibrationAuraStyle(values: CalibrationVector) {
    const avec = asAvecState(values);
    return `background: linear-gradient(135deg, ${avecColor(avec, 1)}, ${avecColor({
      stability: values.autonomy,
      friction: values.stability,
      logic: values.friction,
      autonomy: values.logic,
      psi: values.autonomy + values.stability + values.friction + values.logic,
    }, 1)}); box-shadow: 0 0 18px ${avecColor(avec, 0.24)};`;
  }

  function openCompose(mode: 'live' | 'importare' = 'live') {
    composeModeMenuOpen = false;
    composeMode = mode;
    composeSessionId = selectedSession?.label ?? '';
    composeDraft = '';
    composeMessages = [];
    composeError = null;
    composeResult = null;
    composeLoading = false;
    composeReplyLoading = false;
    composeEncodePromptSent = false;
    composePromptCopyLoading = false;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;
    composePasteNodeOpen = mode === 'importare';
    composePasteNodeDraft = '';
    composePasteNodeLoading = false;
    composeOpen = true;
  }

  function toggleComposeModeMenu() {
    composeModeMenuOpen = !composeModeMenuOpen;
  }

  function openComposeLive() {
    markWalkthroughStepSatisfied('live');
    composeModeMenuOpen = false;
    openCompose('live');
  }

  function openComposeImportare() {
    markWalkthroughStepSatisfied('importare');
    composeModeMenuOpen = false;
    openCompose('importare');
  }

  function switchComposeToLive() {
    markWalkthroughStepSatisfied('live');
    composeMode = 'live';
    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
  }

  function clearComposeConversation() {
    composeDraft = '';
    composeMessages = [];
    composeError = null;
    composeResult = null;
    composeEncodePromptSent = false;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;
    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
  }

  function clearComposePromptCopiedTimer() {
    if (composePromptCopiedTimer !== null) {
      clearTimeout(composePromptCopiedTimer);
      composePromptCopiedTimer = null;
    }
  }

  function composeApiMessages(messages: ComposeMessage[]): ChatMessage[] {
    return messages.map((message) => ({
      role: message.role,
      content: message.content,
    }));
  }

  async function copyTextToClipboard(text: string) {
    if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }

    if (typeof document === 'undefined') {
      throw new Error('clipboard unavailable in this environment');
    }

    const bridge = document.createElement('textarea');
    bridge.value = text;
    bridge.setAttribute('readonly', '');
    bridge.style.position = 'fixed';
    bridge.style.opacity = '0';
    bridge.style.pointerEvents = 'none';
    document.body.appendChild(bridge);
    bridge.focus();
    bridge.select();

    const copied = document.execCommand('copy');
    document.body.removeChild(bridge);

    if (!copied) {
      throw new Error('copy command failed');
    }
  }

  async function copyComposeEncodePrompt() {
    if (composePromptCopyLoading) {
      return;
    }

    composePromptCopyLoading = true;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;

    try {
      // Keep distill prompt copy aligned to the exact encode preamble source.
      const prompt = await resonantiaClient.getComposeEncodePreamble();
      await copyTextToClipboard(prompt);
      composePromptCopied = true;
      composePromptCopiedTimer = setTimeout(() => {
        composePromptCopied = false;
        composePromptCopiedTimer = null;
      }, 2200);
    } catch (err) {
      composePromptCopied = false;
      composePromptCopyError = String(err);
    } finally {
      composePromptCopyLoading = false;
    }
  }

  function toggleComposePasteNode() {
    composePasteNodeOpen = !composePasteNodeOpen;
    if (!composePasteNodeOpen) {
      composePasteNodeDraft = '';
    }
  }

  async function sendComposeMessage() {
    const text = composeDraft.trim();
    if (!text || composeReplyLoading || composeLoading) {
      return;
    }

    const sessionId = composeSessionId.trim();
    if (!sessionId) {
      composeError = 'session id is required';
      return;
    }

    composeError = null;
    composeResult = null;

    const nextMessages: ComposeMessage[] = [
      ...composeMessages,
      {
        role: 'user',
        content: text,
        at: new Date().toISOString(),
      },
    ];

    composeMessages = nextMessages;
    composeDraft = '';
    composeReplyLoading = true;

    try {
      const reply = await resonantiaClient.chatCompose({
        sessionId,
        messages: composeApiMessages(nextMessages),
      });

      if (reply?.trim()) {
        composeMessages = [
          ...nextMessages,
          {
            role: 'assistant',
            content: reply.trim(),
            at: new Date().toISOString(),
          },
        ];
      }
    } catch (err) {
      composeError = String(err);
    } finally {
      composeReplyLoading = false;
    }
  }

  async function saveComposePastedNode() {
    const rawNode = composePasteNodeDraft.trim();
    if (!rawNode || composePasteNodeLoading || composeLoading || composeReplyLoading) {
      return;
    }

    const sessionId = composeSessionId.trim();
    if (!sessionId) {
      composeError = 'session id is required';
      return;
    }

    composePasteNodeLoading = true;
    composeError = null;
    composeResult = null;

    try {
      const res = await resonantiaClient.storeContext({
        node: rawNode,
        sessionId,
      });

      if (!res.valid) {
        composeError = res.validationError ?? 'store rejected by local policy';
        return;
      }

      const status = res.upsertStatus ?? (res.duplicateSkipped ? 'duplicate' : 'created');
      composeResult = {
        psi: res.psi,
        duplicateSkipped: Boolean(res.duplicateSkipped),
        status,
      };

      composePasteNodeDraft = '';
      composePasteNodeOpen = false;
      await loadGraph();
    } catch (err) {
      composeError = String(err);
    } finally {
      composePasteNodeLoading = false;
    }
  }

  async function runSyncPull() {
    if (syncPullLoading) {
      return;
    }

    syncPullLoading = true;
    syncPullError = null;
    syncPullResult = null;
    syncDetailAutoOpen = true;
    clearSyncDetailTimer();

    try {
      const config = await resonantiaClient.getConfig();
      const configuredGateway = (config.gatewayBaseUrl ?? '').trim();
      const usingManagedGateway = isManagedGatewayBaseUrl(configuredGateway);

      if (usingManagedGateway) {
        await refreshCloudAuthState();
      }

      if (usingManagedGateway && !hasPaidCloudTier(cloudAccountTier)) {
        if (!cloudAuthSignedIn) {
          throw new Error('cloud sync on the managed gateway requires sign-in and a resonant/soulful plan');
        }

        throw new Error('cloud sync on the managed gateway is available on resonant or soulful tier; add your own gateway URL to use BYO sync on free tier');
      }

      let syncAuthToken = (config.gatewayAuthToken ?? '').trim();

      if (usingManagedGateway) {
        try {
          await refreshGatewayAuthTokenForSync();
          const refreshed = await resonantiaClient.getConfig();
          syncAuthToken = (refreshed.gatewayAuthToken ?? '').trim();
        } catch (tokenError) {
          cloudAuthError = String(tokenError);
        }
      }

      syncPullResult = await resonantiaClient.syncNow({
        pageSize: 200,
        gatewayBaseUrl: configuredGateway || undefined,
        gatewayAuthToken: syncAuthToken || undefined,
      });
      await loadGraph();
    } catch (err) {
      syncPullError = String(err);
    } finally {
      if (!syncPullResult && !syncPullError) {
        syncPullError = 'sync did not complete';
      }
      syncPullLoading = false;
      menuOpen = false;
      syncDetailTimestamp = new Date();
      scheduleSyncDetailClose(syncPullError ? 7200 : 5200);
    }
  }

  async function submitCompose() {
    if (composeLoading || composeMessages.length === 0 || composeReplyLoading) return;
    const sessionId = composeSessionId.trim();
    if (!sessionId) {
      composeError = 'session id is required';
      return;
    }

    composeLoading = true; composeError = null; composeResult = null; composeEncodePromptSent = false;
    try {
      const messages = composeApiMessages(composeMessages);
      const maxEncodeAttempts = 2;
      let parserErrorHint: string | undefined;
      let previousNodeCandidate: string | undefined;
      let res: StoreContextResponse | null = null;

      for (let attempt = 0; attempt < maxEncodeAttempts; attempt += 1) {
        composeEncodePromptSent = true;
        const encodedNode = await resonantiaClient.encodeCompose({
          sessionId,
          messages,
          parserErrorHint,
          previousNodeCandidate,
        });

        res = await resonantiaClient.storeContext({
          node: encodedNode,
          sessionId,
        });

        if (res.valid) {
          break;
        }

        const validationError = (res.validationError ?? '').trim();
        const retryableParseError = /ParseFailure|MissingLayer|missing required layer|missing layer/i.test(validationError);
        if (!retryableParseError || attempt >= maxEncodeAttempts - 1) {
          composeError = validationError || 'store rejected by local policy';
          return;
        }

        parserErrorHint = validationError;
        previousNodeCandidate = encodedNode;
      }

      if (!res || !res.valid) {
        composeError = res?.validationError ?? 'store rejected by local policy';
        return;
      }

      const status = res.upsertStatus ?? (res.duplicateSkipped ? 'duplicate' : 'created');
      composeResult = {
        psi: res.psi,
        duplicateSkipped: Boolean(res.duplicateSkipped),
        status,
      };
      composeDraft = '';
      composeMessages = [];
      await loadGraph();
    } catch (err) { composeError = String(err); }
    finally      { composeLoading = false; composeEncodePromptSent = false; }
  }

  // ── Calibrate ───────────────────────────────────────────────
  let calibrateOpen  = false;
  let calibSessionId = '';
  let calibStability = 0.5;
  let calibFriction  = 0.2;
  let calibLogic     = 0.8;
  let calibAutonomy  = 0.9;
  let calibTrigger   = 'manual';
  let calibLoading   = false;
  let calibError: string | null = null;
  let guideOpen = false;
  let guideAnswers = [...DEFAULT_CALIBRATION_ANSWERS];

  $: currentCalibrationVector = {
    stability: calibStability,
    friction: calibFriction,
    logic: calibLogic,
    autonomy: calibAutonomy,
  };

  $: closestCalibrationProfile = CALIBRATION_PROFILES.reduce((best, profile) => {
    const distance = calibrationDistance(profile.values, currentCalibrationVector);
    if (!best || distance < best.distance) {
      return { profile, distance };
    }
    return best;
  }, null as { profile: CalibrationProfile; distance: number } | null);

  $: calibrationPsi = calibStability + calibFriction + calibLogic + calibAutonomy;

  function setCalibration(values: CalibrationVector, trigger = calibTrigger) {
    calibStability = clamp01(values.stability);
    calibFriction = clamp01(values.friction);
    calibLogic = clamp01(values.logic);
    calibAutonomy = clamp01(values.autonomy);
    calibTrigger = trigger;
  }

  function applyCalibrationProfile(profile: CalibrationProfile) {
    setCalibration(profile.values, profile.trigger);
  }

  function resetCalibrationGuide() {
    guideAnswers = [...DEFAULT_CALIBRATION_ANSWERS];
  }

  function selectGuideAnswer(questionIndex: number, optionIndex: number) {
    guideAnswers = guideAnswers.map((answer, index) => (index === questionIndex ? optionIndex : answer));
  }

  function applyCalibrationGuide() {
    const vectors = CALIBRATION_QUESTIONS.map((question, index) => question.options[guideAnswers[index]].values);
    setCalibration(averageCalibrationVectors(vectors), 'guided_reflection');
  }

  function openCalibrate() {
    markWalkthroughStepSatisfied('checkin');
    menuOpen = false;
    calibSessionId = selectedSession?.label ?? '';
    calibError = null;
    guideOpen = false;
    resetCalibrationGuide();
    calibrateOpen = true;
  }

  async function submitCalibrate() {
    const sessionId = calibSessionId.trim();
    if (!sessionId) {
      calibError = 'session id is required';
      return;
    }

    calibLoading = true; calibError = null;
    try {
      await resonantiaClient.calibrateSession({
        sessionId,
        stability: calibStability,
        friction: calibFriction,
        logic: calibLogic,
        autonomy: calibAutonomy,
        trigger: calibTrigger,
      });
      calibrateOpen = false;
    } catch (err) { calibError = String(err); }
    finally      { calibLoading = false; }
  }

  type AdventureChoiceStageId = 'decision1' | 'decision2' | 'decision3' | 'cta';

  type AdventureStageSignal = {
    stage: AdventureChoiceStageId;
    selectedOption: string | null;
    stareMs: number;
    decisionMs: number;
    hoverMsByOption: Record<string, number>;
    hoverTotalMs: number;
    switches: number;
    optionVisits: number;
    moveCount: number;
    style: 'decisive' | 'explored' | 'wavering';
    hesitation: number;
  };

  type AdventureSignalMetrics = {
    totalDurationMs: number;
    explorationScore: number;
    hesitationScore: number;
    decisivenessScore: number;
    engagementScore: number;
    weights: {
      stability: number;
      friction: number;
      logic: number;
      autonomy: number;
    };
    stages: Record<AdventureChoiceStageId, AdventureStageSignal>;
    naming: {
      dwellMs: number;
      firstInputLatencyMs: number;
      inputChanges: number;
      backspaces: number;
    };
  };

  type AdventureCompleteDetail = {
    name: string;
    d1: string;
    d2: string;
    d3: string;
    avec: AvecState;
    avecBase?: AvecState;
    metrics?: AdventureSignalMetrics;
  };

  function clampRange(value: number, min: number, max: number) {
    return Math.max(min, Math.min(max, value));
  }

  function round2(value: number) {
    return Number(value.toFixed(2));
  }

  function format2(value: number) {
    return round2(value).toFixed(2);
  }

  function wait(ms: number) {
    return new Promise<void>((resolve) => {
      setTimeout(() => resolve(), ms);
    });
  }

  function safeToken(value: string, fallback = 'na') {
    const normalized = value
      .trim()
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '_')
      .replace(/^_+|_+$/g, '');

    return normalized || fallback;
  }

  function signedShiftToken(value: number) {
    const shifted = Math.round(value * 100);
    if (shifted >= 0) return `p${shifted}`;
    return `m${Math.abs(shifted)}`;
  }

  function toSessionSeed(name: string) {
    const token = safeToken(name, 'unnamed');
    if (token === 'unnamed') {
      return ADVENTURE_SESSION_FALLBACK;
    }

    return token.slice(0, 64);
  }

  function avecPsi(avec: Pick<AvecState, 'stability' | 'friction' | 'logic' | 'autonomy'>) {
    return round2(avec.stability + avec.friction + avec.logic + avec.autonomy);
  }

  function normalizeAvec(input: Partial<AvecState> | undefined): AvecState {
    const stability = clamp01(Number.isFinite(input?.stability) ? Number(input?.stability) : 0.5);
    const friction = clamp01(Number.isFinite(input?.friction) ? Number(input?.friction) : 0.5);
    const logic = clamp01(Number.isFinite(input?.logic) ? Number(input?.logic) : 0.5);
    const autonomy = clamp01(Number.isFinite(input?.autonomy) ? Number(input?.autonomy) : 0.5);

    return {
      stability: round2(stability),
      friction: round2(friction),
      logic: round2(logic),
      autonomy: round2(autonomy),
      psi: avecPsi({ stability, friction, logic, autonomy }),
    };
  }

  function emptyAdventureStage(stage: AdventureChoiceStageId): AdventureStageSignal {
    return {
      stage,
      selectedOption: null,
      stareMs: 0,
      decisionMs: 0,
      hoverMsByOption: {},
      hoverTotalMs: 0,
      switches: 0,
      optionVisits: 0,
      moveCount: 0,
      style: 'decisive',
      hesitation: 0,
    };
  }

  function readAdventureStage(metrics: AdventureSignalMetrics | undefined, stage: AdventureChoiceStageId) {
    return metrics?.stages?.[stage] ?? emptyAdventureStage(stage);
  }

  function buildAdventureSttpNode(sessionId: string, detail: AdventureCompleteDetail) {
    const timestamp = new Date().toISOString();
    const nameToken = safeToken(detail.name || 'unnamed', 'unnamed');
    const d1Token = safeToken(detail.d1 || 'light', 'light');
    const d2Token = safeToken(detail.d2 || 'complete', 'complete');
    const d3Token = safeToken(detail.d3 || 'merge', 'merge');
    const decisionPath = `d1_${d1Token}_d2_${d2Token}_d3_${d3Token}`;
    const metrics = detail.metrics;
    const d1Stage = readAdventureStage(metrics, 'decision1');
    const d2Stage = readAdventureStage(metrics, 'decision2');
    const d3Stage = readAdventureStage(metrics, 'decision3');
    const ctaStage = readAdventureStage(metrics, 'cta');

    const exploration = clamp01(metrics?.explorationScore ?? 0.34);
    const hesitation = clamp01(metrics?.hesitationScore ?? 0.32);
    const decisiveness = clamp01(metrics?.decisivenessScore ?? (1 - hesitation));
    const engagement = clamp01(metrics?.engagementScore ?? 0.38);
    const weights = metrics?.weights ?? { stability: 0, friction: 0, logic: 0, autonomy: 0 };

    const userAvec = normalizeAvec(detail.avec);
    const baseAvec = normalizeAvec(detail.avecBase ?? detail.avec);
    const modelAvec = normalizeAvec({
      stability: clamp01(userAvec.stability * 0.72 + baseAvec.stability * 0.28 + decisiveness * 0.04 - hesitation * 0.02),
      friction: clamp01(userAvec.friction * 0.72 + baseAvec.friction * 0.28 + hesitation * 0.05 - decisiveness * 0.03),
      logic: clamp01(userAvec.logic * 0.72 + baseAvec.logic * 0.28 + engagement * 0.03),
      autonomy: clamp01(userAvec.autonomy * 0.72 + baseAvec.autonomy * 0.28 + exploration * 0.04),
    });

    const compressionAvec = normalizeAvec({
      stability: (userAvec.stability + modelAvec.stability) / 2,
      friction: (userAvec.friction + modelAvec.friction) / 2,
      logic: (userAvec.logic + modelAvec.logic) / 2,
      autonomy: (userAvec.autonomy + modelAvec.autonomy) / 2,
    });

    const hoverLightMs = Math.round(d1Stage.hoverMsByOption.light ?? 0);
    const hoverSoundMs = Math.round(d1Stage.hoverMsByOption.sound ?? 0);

    const hesitationToken = `d1_${Math.round(d1Stage.hesitation * 100)}_d2_${Math.round(d2Stage.hesitation * 100)}_d3_${Math.round(d3Stage.hesitation * 100)}_cta_${Math.round(ctaStage.hesitation * 100)}`;
    const latencyToken = `d1_${Math.round(d1Stage.decisionMs)}_d2_${Math.round(d2Stage.decisionMs)}_d3_${Math.round(d3Stage.decisionMs)}_cta_${Math.round(ctaStage.decisionMs)}`;
    const stareToken = `d1_${Math.round(d1Stage.stareMs)}_d2_${Math.round(d2Stage.stareMs)}_d3_${Math.round(d3Stage.stareMs)}_cta_${Math.round(ctaStage.stareMs)}`;
    const switchToken = `d1_${d1Stage.switches}_d2_${d2Stage.switches}_d3_${d3Stage.switches}_cta_${ctaStage.switches}`;
    const styleToken = `d1_${safeToken(d1Stage.style)}_d2_${safeToken(d2Stage.style)}_d3_${safeToken(d3Stage.style)}_cta_${safeToken(ctaStage.style)}`;
    const engagementToken = `explore_${Math.round(exploration * 100)}_hesitate_${Math.round(hesitation * 100)}_decide_${Math.round(decisiveness * 100)}_engage_${Math.round(engagement * 100)}`;
    const namingToken = `dwell_${Math.round(metrics?.naming?.dwellMs ?? 0)}_first_input_${Math.round(metrics?.naming?.firstInputLatencyMs ?? 0)}_changes_${Math.round(metrics?.naming?.inputChanges ?? 0)}_backspaces_${Math.round(metrics?.naming?.backspaces ?? 0)}`;
    const shiftToken = `grounding_${signedShiftToken(weights.stability)}_wear_${signedShiftToken(weights.friction)}_clarity_${signedShiftToken(weights.logic)}_self_trust_${signedShiftToken(weights.autonomy)}`;

    const rho = clampRange(0.74 + engagement * 0.14 + decisiveness * 0.08 + (1 - hesitation) * 0.04, 0.7, 0.98);
    const kappa = clampRange(0.72 + exploration * 0.12 + (1 - hesitation) * 0.08 + Math.min(1, d1Stage.optionVisits / 2) * 0.05, 0.7, 0.97);

    return [
      '⏣',
      `⊕⟨ ⏣0{ trigger: manual, response_format: temporal_node, origin_session: ${sessionId}, compression_depth: 3, parent_node: null, prime: { attractor_config: { stability: ${format2(modelAvec.stability)}, friction: ${format2(modelAvec.friction)}, logic: ${format2(modelAvec.logic)}, autonomy: ${format2(modelAvec.autonomy)} }, context_summary: onboarding_behavioral_memory_${decisionPath}, relevant_tier: raw, retrieval_budget: 10 } } ⟩`,
      `⦿⟨ ⏣0{ timestamp: ${timestamp}, tier: raw, session_id: ${sessionId}, schema_version: sttp-1.0, user_avec: { stability: ${format2(userAvec.stability)}, friction: ${format2(userAvec.friction)}, logic: ${format2(userAvec.logic)}, autonomy: ${format2(userAvec.autonomy)}, psi: ${format2(userAvec.psi)} }, model_avec: { stability: ${format2(modelAvec.stability)}, friction: ${format2(modelAvec.friction)}, logic: ${format2(modelAvec.logic)}, autonomy: ${format2(modelAvec.autonomy)}, psi: ${format2(modelAvec.psi)} } } ⟩`,
      `◈⟨ ⏣0{ decision_path(.98): ${decisionPath}, chosen_name(.95): ${nameToken}, hover_split_decision1_ms(.94): light_${hoverLightMs}_sound_${hoverSoundMs}, switch_pattern(.92): ${switchToken}, stage_styles(.92): ${styleToken}, stare_latency_ms(.91): ${stareToken}, decision_latency_ms(.91): ${latencyToken}, naming_signal(.9): ${namingToken}, engagement_profile(.9): ${engagementToken}, weighting_shift(.91): ${shiftToken}, total_journey_ms(.88): ${Math.round(metrics?.totalDurationMs ?? 0)} } ⟩`,
      `⍉⟨ ⏣0{ rho: ${format2(rho)}, kappa: ${format2(kappa)}, psi: ${format2(modelAvec.psi)}, compression_avec: { stability: ${format2(compressionAvec.stability)}, friction: ${format2(compressionAvec.friction)}, logic: ${format2(compressionAvec.logic)}, autonomy: ${format2(compressionAvec.autonomy)}, psi: ${format2(compressionAvec.psi)} } } ⟩`,
    ].join('\n');
  }

  function buildAdventureFallbackNode(sessionId: string, detail: AdventureCompleteDetail) {
    const timestamp = new Date().toISOString();
    const nameToken = safeToken(detail.name || 'unnamed', 'unnamed');
    const d1Token = safeToken(detail.d1 || 'light', 'light');
    const d2Token = safeToken(detail.d2 || 'complete', 'complete');
    const d3Token = safeToken(detail.d3 || 'merge', 'merge');
    const userAvec = normalizeAvec(detail.avec);
    const modelAvec = normalizeAvec(detail.avecBase ?? detail.avec);

    return [
      '⏣',
      `⊕⟨ ⏣0{ trigger: manual, response_format: temporal_node, origin_session: ${sessionId}, compression_depth: 2, parent_node: null, prime: { attractor_config: { stability: ${format2(modelAvec.stability)}, friction: ${format2(modelAvec.friction)}, logic: ${format2(modelAvec.logic)}, autonomy: ${format2(modelAvec.autonomy)} }, context_summary: onboarding_fallback_memory, relevant_tier: raw, retrieval_budget: 8 } } ⟩`,
      `⦿⟨ ⏣0{ timestamp: ${timestamp}, tier: raw, session_id: ${sessionId}, schema_version: sttp-1.0, user_avec: { stability: ${format2(userAvec.stability)}, friction: ${format2(userAvec.friction)}, logic: ${format2(userAvec.logic)}, autonomy: ${format2(userAvec.autonomy)}, psi: ${format2(userAvec.psi)} }, model_avec: { stability: ${format2(modelAvec.stability)}, friction: ${format2(modelAvec.friction)}, logic: ${format2(modelAvec.logic)}, autonomy: ${format2(modelAvec.autonomy)}, psi: ${format2(modelAvec.psi)} } } ⟩`,
      `◈⟨ ⏣0{ decision_path(.95): d1_${d1Token}_d2_${d2Token}_d3_${d3Token}, chosen_name(.93): ${nameToken}, source(.9): onboarding_adventure_weighted_capture } ⟩`,
      `⍉⟨ ⏣0{ rho: 0.86, kappa: 0.84, psi: ${format2(modelAvec.psi)}, compression_avec: { stability: ${format2(modelAvec.stability)}, friction: ${format2(modelAvec.friction)}, logic: ${format2(modelAvec.logic)}, autonomy: ${format2(modelAvec.autonomy)}, psi: ${format2(modelAvec.psi)} } } ⟩`,
    ].join('\n');
  }

  async function persistAdventureMemory(detail: AdventureCompleteDetail): Promise<boolean> {
    const sessionId = toSessionSeed(detail.name);
    composeSessionId = sessionId;
    calibSessionId = sessionId;

    try {
      let node = buildAdventureSttpNode(sessionId, detail);
      let res = await resonantiaClient.storeContext({
        node,
        sessionId,
      });

      if (!res.valid) {
        node = buildAdventureFallbackNode(sessionId, detail);
        res = await resonantiaClient.storeContext({
          node,
          sessionId,
        });

        if (!res.valid) {
          console.error('adventure memory rejected', res.validationError ?? 'store rejected by local policy');
          return false;
        }
      }

      await loadGraph();
      return true;
    } catch (err) {
      console.error('adventure memory store failed', err);
      return false;
    }
  }

  async function continueIntoWalkthroughAfterAdventure(detail: AdventureCompleteDetail) {
    const stored = await persistAdventureMemory(detail);
    if (!stored) {
      return;
    }

    onboardingDismissed = false;
    persistOnboardingDismissedState(false);

    await wait(320);
    if (!onboardingOpen) {
      openWalkthrough('first-run');
    }
  }

  function handleAdventureComplete(e: CustomEvent<AdventureCompleteDetail>) {
    const detail = e.detail;
    adventureOpen = false;
    adventureCompleted = true;
    persistAdventureCompleted();
    void continueIntoWalkthroughAfterAdventure(detail);
  }

  function handleAdventureSkip() {
    adventureOpen = false;
    adventureCompleted = true;
    persistAdventureCompleted();
    // onboardingDismissed stays false → reactive fires openWalkthrough('first-run')
  }

  // ── Lifecycle ─────────────────────────────────────────────────
  onMount(() => {
    onboardingDismissed = readOnboardingDismissedState();
    adventureCompleted  = readAdventureCompleted();
    adventureHydrated   = true;
    onboardingHydrated  = true;
    ctx = canvas.getContext('2d')!;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    noteInteraction();
    startRenderLoop();
    requestAnimationFrame(() => { resize(); loadGraph(); });
    checkHealth();

    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', handleVisibilityChange);
    }

    window.addEventListener('mousemove', handleTelescopeDialMouseMove);
    window.addEventListener('mouseup', endTelescopeDial);
    window.addEventListener('touchmove', handleTelescopeDialTouchMove, { passive: false });
    window.addEventListener('touchend', endTelescopeDial);

    const ro = new ResizeObserver(resize);
    ro.observe(container);
    return () => {
      ro.disconnect();
      if (typeof document !== 'undefined') {
        document.removeEventListener('visibilitychange', handleVisibilityChange);
      }
      window.removeEventListener('mousemove', handleTelescopeDialMouseMove);
      window.removeEventListener('mouseup', endTelescopeDial);
      window.removeEventListener('touchmove', handleTelescopeDialTouchMove);
      window.removeEventListener('touchend', endTelescopeDial);
    };
  });

  onDestroy(() => {
    stopRenderLoop();
    clearSyncDetailTimer();
    clearComposePromptCopiedTimer();
    clearWalkthroughCueTimer();
    clearWalkthroughAdvanceTimer();
  });
</script>

<div class="weaver-root" class:walkthrough-compact={walkthroughCompact} bind:this={container}>
  <canvas
    bind:this={canvas}
    on:pointerdown={onPointerDown}
    on:pointermove={onPointerMove}
    on:pointerup={onPointerUp}
    on:pointercancel={onPointerCancel}
    on:lostpointercapture={onPointerCancel}
    class:grabbing={dragging}
    class:telescope-open={telescopeCameraEngaged}
  ></canvas>

  <nav class="navbar" class:faded={telescopeCameraEngaged}>
    <div class="nav-left">
      {#if level > 0}
        <button class="back-btn" on:click={level === 2 ? surfaceToWave : surfaceToConstellation}>
          ← {level === 2 ? 'wave' : 'constellation'}
        </button>
      {:else}
        <span class="brand">resonantia</span>
      {/if}
    </div>
    <div class="nav-right">
      <span class="status-dot" class:healthy></span>
      <span
        class="source-dot"
        class:local={sourceBadgeTone === 'local'}
        class:cloud={sourceBadgeTone === 'cloud'}
        class:mem={sourceBadgeTone === 'mem'}
        title={sourceBadgeTitle}
        aria-label={sourceBadgeLabel}
      ></span>
      <SyncCloudStatus
        visualState={syncVisualState}
        pullLoading={syncPullLoading}
        buttonAria={syncButtonAria}
        detailVisible={syncDetailVisible}
        detailTitle={syncDetailTitle}
        detailTimeLabel={syncDetailTimeLabel}
        detailSubtitle={syncDetailSubtitle}
        pullResult={syncPullResult}
        pullError={syncPullError}
        accountTier={cloudAccountTier}
        openSyncDetailHover={openSyncDetailHover}
        closeSyncDetailHover={closeSyncDetailHover}
        runSyncPull={runSyncPull}
      />
      <TourActionMenu
        open={menuOpen}
        on:toggle={toggleMenu}
        on:refresh={() => { menuOpen = false; loadGraph(); }}
        on:demo={openCinematicDemo}
        on:checkin={openCalibrate}
        on:settings={openSettings}
      />
    </div>
  </nav>

  <CollapseCard
    data={cardData}
    visible={cardVisible}
    summary={currentTransmutation}
    transmuting={transmuting}
    transmuteError={transmuteError}
    openExternalUrl={openExternalUrl}
    on:close={closeCard}
    on:navigate={handleNavigate}
    on:transmute={transmuteCurrentNode}
  />

  <ComposeLauncher
    faded={telescopeCameraEngaged}
    hidden={composeOpen}
    menuOpen={composeModeMenuOpen}
    on:toggle={toggleComposeModeMenu}
    on:live={openComposeLive}
    on:importare={openComposeImportare}
  />

  <AdventureOnboarding
    open={adventureOpen}
    on:complete={handleAdventureComplete}
    on:skip={handleAdventureSkip}
  />

  <WalkthroughGuide
    open={onboardingOpen}
    mode={walkthroughMode}
    phase={walkthroughStep}
    cueVisible={walkthroughCueVisible}
    targetSelector={walkthroughTargetSelector}
    allowedSelectors={walkthroughAllowedSelectors}
    on:start={handleWalkthroughStart}
    on:next={handleWalkthroughNext}
    on:dismiss={handleWalkthroughDismiss}
  />
  <TelescopePanel
    telescopeCameraEngaged={telescopeCameraEngaged}
    level={level}
    telescopeCanAccess={telescopeCanAccess}
    negativeLayerActive={negativeLayerActive}
    telescopeOpen={telescopeOpen}
    telescopePhase={telescopePhase}
    telescopeDialOffsetY={telescopeDialOffsetY}
    telescopeRangeLabel={telescopeRangeLabel}
    telescopeTimelineSessions={telescopeTimelineSessions}
    toggleNegativeLayer={toggleNegativeLayer}
    openTelescope={openTelescope}
    handleTelescopeClosePointerDown={handleTelescopeClosePointerDown}
    handleTelescopeDialMouseDown={handleTelescopeDialMouseDown}
    handleTelescopeDialTouchStart={handleTelescopeDialTouchStart}
    handleTelescopeDialKeydown={handleTelescopeDialKeydown}
    handleTelescopeEyeKeydown={handleTelescopeEyeKeydown}
    selectTelescopeSession={selectTelescopeSession}
    telescopeSessionColor={telescopeSessionColor}
    telescopeSessionMeta={telescopeSessionMeta}
    telescopeSessionDateLabel={telescopeSessionDateLabel}
    shortLabel={shortLabel}
  />

  <ComposeDrawer
    open={composeOpen}
    mode={composeMode}
    bind:sessionId={composeSessionId}
    bind:draft={composeDraft}
    messages={composeMessages}
    loading={composeLoading}
    replyLoading={composeReplyLoading}
    encodePromptSent={composeEncodePromptSent}
    error={composeError}
    result={composeResult}
    promptCopyLoading={composePromptCopyLoading}
    promptCopied={composePromptCopied}
    promptCopyError={composePromptCopyError}
    pasteNodeOpen={composePasteNodeOpen}
    bind:pasteNodeDraft={composePasteNodeDraft}
    pasteNodeLoading={composePasteNodeLoading}
    onClose={() => (composeOpen = false)}
    onSessionInput={() => {
      if (composeError && /session id is required/i.test(composeError) && composeSessionId.trim()) {
        composeError = null;
      }
    }}
    sendComposeMessage={sendComposeMessage}
    copyComposeEncodePrompt={copyComposeEncodePrompt}
    toggleComposePasteNode={toggleComposePasteNode}
    clearComposeConversation={clearComposeConversation}
    switchComposeToLive={switchComposeToLive}
    saveComposePastedNode={saveComposePastedNode}
    submitCompose={submitCompose}
  />

  <CalibrateDrawer
    open={calibrateOpen}
    bind:sessionId={calibSessionId}
    bind:stability={calibStability}
    bind:friction={calibFriction}
    bind:logic={calibLogic}
    bind:autonomy={calibAutonomy}
    bind:guideOpen
    guideAnswers={guideAnswers}
    trigger={calibTrigger}
    loading={calibLoading}
    error={calibError}
    calibrationPsi={calibrationPsi}
    currentCalibrationVector={currentCalibrationVector}
    closestCalibrationProfile={closestCalibrationProfile}
    calibrationProfiles={CALIBRATION_PROFILES}
    calibrationQuestions={CALIBRATION_QUESTIONS}
    calibrationSurfaceStyle={calibrationSurfaceStyle}
    calibrationSpectrumStyle={calibrationSpectrumStyle}
    calibrationAuraStyle={calibrationAuraStyle}
    applyCalibrationProfile={applyCalibrationProfile}
    selectGuideAnswer={selectGuideAnswer}
    applyCalibrationGuide={applyCalibrationGuide}
    submitCalibrate={submitCalibrate}
    onClose={() => (calibrateOpen = false)}
    onSessionInput={() => {
      if (calibError && /session id is required/i.test(calibError) && calibSessionId.trim()) {
        calibError = null;
      }
    }}
  />

  <SettingsDrawer
    open={settingsOpen}
    loading={settingsLoading}
    saving={settingsSaving}
    error={settingsError}
    saved={settingsSaved}
    localModelOriginWarning={localModelOriginWarning}
    bind:ollamaBaseUrl
    bind:ollamaModel
    bind:gatewayBaseUrl
    bind:gatewayAuthToken
    cloudAuthAvailable={cloudAuthAvailable}
    cloudAuthSignedIn={cloudAuthSignedIn}
    cloudAuthBusy={cloudAuthBusy}
    cloudAuthStatus={cloudAuthStatus}
    cloudAuthError={cloudAuthError}
    accountTier={cloudAccountTier}
    accountMemberSince={cloudAccountMemberSince}
    bind:advancedOpen
    on:close={() => (settingsOpen = false)}
    on:save={saveSettings}
    on:demo={openOnboardingTutorial}
    on:connectCloud={connectCloudAccount}
    on:refreshCloudToken={refreshGatewayAuthToken}
    on:clearCloudToken={clearGatewayAuthToken}
  />
</div>

<style>
  .weaver-root {
    --safe-top: env(safe-area-inset-top, 0px);
    --safe-right: env(safe-area-inset-right, 0px);
    --safe-bottom: env(safe-area-inset-bottom, 0px);
    --safe-left: env(safe-area-inset-left, 0px);
    position: fixed;
    inset: 0;
    overflow: hidden;
    background: #0a0b0e;
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    touch-action: none;
    cursor: grab;
    display: block;
  }
  canvas.grabbing { cursor: grabbing; }
  canvas.telescope-open { pointer-events: none; }

  .navbar {
    position: absolute;
    top: var(--safe-top); left: 0; right: 0;
    height: 46px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 calc(22px + var(--safe-right)) 0 calc(22px + var(--safe-left));
    background: linear-gradient(to bottom, rgba(10,11,14,0.88) 0%, transparent 100%);
    z-index: 10;
    pointer-events: none;
    transition: opacity 0.24s ease, transform 0.24s ease;
  }

  .navbar.faded {
    opacity: 0;
    transform: translateY(-6px);
    pointer-events: none;
  }

  .nav-left, .nav-right {
    display: flex;
    align-items: center;
    gap: 8px;
    pointer-events: all;
  }

  .brand {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 15px;
    color: rgba(255, 255, 255, 0.4);
    letter-spacing: 0.04em;
  }

  .back-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.35);
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s;
  }
  .back-btn:hover {
    color: rgba(255, 255, 255, 0.75);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .status-dot {
    width: 6px; height: 6px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.15);
    transition: background 0.4s;
  }
  .status-dot.healthy { background: #7aaa7a; }

  .source-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    border: 0.5px solid rgba(255, 255, 255, 0.14);
    background: rgba(255, 255, 255, 0.16);
    box-shadow: 0 0 6px rgba(255, 255, 255, 0.18);
  }

  .source-dot.local {
    border-color: rgba(91, 155, 213, 0.52);
    background: rgba(91, 155, 213, 0.9);
    box-shadow: 0 0 8px rgba(91, 155, 213, 0.42);
  }

  .source-dot.cloud {
    border-color: rgba(122, 170, 122, 0.56);
    background: rgba(122, 170, 122, 0.92);
    box-shadow: 0 0 8px rgba(122, 170, 122, 0.44);
  }

  .source-dot.mem {
    border-color: rgba(233, 148, 58, 0.58);
    background: rgba(233, 148, 58, 0.94);
    box-shadow: 0 0 8px rgba(233, 148, 58, 0.46);
  }
</style>
