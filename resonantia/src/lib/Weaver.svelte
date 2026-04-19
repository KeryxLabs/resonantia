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
  import AlkahestPanel from './components/AlkahestPanel.svelte';
  import { getCloudAuthStatus, getGatewayAuthToken, signOutCloud, redirectToCloudSignIn, getCloudAccount } from './cloudAuth';
  import { getGatewayBaseUrl as getManagedGatewayBaseUrl } from './config';
  import { computeComposeTokenUsage, warmupComposeTokenizer, type ComposeTokenUsage } from './composeTokenBudget';
  import type {
    ComposeCalibrationAvec,
    ComposeContextNode,
    ComposeContextSession,
    ComposeLiveUiProps,
    ComposeMessage,
    ComposeMode,
    ComposeProviderUsage,
    CrossSessionRoutingPreference,
  } from './components/compose/types';
  import type { WalkthroughMode, WalkthroughPhase } from './walkthrough';
  import { resonantiaClient } from './resonantiaClient';
  import type {
    AiSummary,
    AvecState,
    ChatMessage,
    ComposeChatResponse,
    GraphResponse,
    GraphSessionDto,
    GraphNodeDto,
    HealthResponse,
    ListNodesResponse,
    ModelProvider,
    NodeDto,
    OpenAiByoKeyStatus,
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
  let alkahestSessionOptions: GraphSessionDto[] = [];
  const ONBOARDING_DISMISSED_KEY = 'resonantia:onboarding-dismissed:v1';
  const ADVENTURE_COMPLETED_KEY  = 'resonantia:adventure-completed:v1';
  const COMPOSE_CONTINUE_ROUTING_KEY = 'resonantia:compose-continue-routing:v1';
  const COMPOSE_CHAT_SETTINGS_KEY = 'resonantia:compose-chat-settings:v1';
  const ADVENTURE_SESSION_FALLBACK = 'first-user-experience-resonantia';
  const WALKTHROUGH_SESSION_SEED = 'resonantia-demo';
  let onboardingOpen = false;
  let onboardingDismissed = false;
  let onboardingHydrated = false;
  let adventureReplayFollowup: WalkthroughMode | null = null;
  let adventureOpen = false;
  let adventureCompleted = false;
  let adventureHydrated = false;
  let hasGraphData = false;
  let renameSessionOpen = false;
  let renameSessionTargetId: string | null = null;
  let renameSessionFallback = '';
  let renameSessionDraft = '';
  let renameSessionError: string | null = null;
  let renameSessionLoading = false;

  type WalkthroughStep = WalkthroughPhase;

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

  function canonicalSessionId(session: GraphSessionDto | null | undefined): string {
    if (!session) {
      return '';
    }

    const label = session.label.trim();
    if (label) {
      return label;
    }

    return session.id.startsWith('s:') ? session.id.slice(2) : session.id;
  }

  function sessionTitle(session: GraphSessionDto): string {
    return canonicalSessionId(session);
  }

  function openRenameSessionDialog(session: GraphSessionDto) {
    noteInteraction();
    renameSessionTargetId = session.id;
    renameSessionFallback = canonicalSessionId(session);
    renameSessionDraft = renameSessionFallback;
    renameSessionError = null;
    renameSessionLoading = false;
    renameSessionOpen = true;
  }

  function closeRenameSessionDialog() {
    renameSessionOpen = false;
    renameSessionTargetId = null;
    renameSessionFallback = '';
    renameSessionDraft = '';
    renameSessionError = null;
    renameSessionLoading = false;
  }

  function sessionNotFoundError(error: unknown): boolean {
    return /source session not found/i.test(String(error ?? ''));
  }

  function friendlyGraphLoadError(reason: unknown): string {
    const message = String(reason ?? '').toLowerCase();

    if (
      message.includes('open db endpoint')
      || message.includes('timed out after')
      || message.includes('indexeddb')
      || message.includes('mem://')
    ) {
      return 'local memory is unavailable right now. retry in a moment.';
    }

    if (message.includes('failed to fetch') || message.includes('network')) {
      return 'network is unavailable right now. retry in a moment.';
    }

    return 'unable to load right now. retry in a moment.';
  }

  function noteDeferredCloudRename(reason: unknown) {
    const detail = String(reason ?? '').replace(/\s+/g, ' ').trim();
    if (detail) {
      console.warn('[rename] cloud rename deferred:', detail);
    } else {
      console.warn('[rename] cloud rename deferred');
    }
    syncPullResult = null;
    syncPullError = 'rename saved locally only; cloud currently unavailable';
    syncDetailTimestamp = new Date();
    syncDetailAutoOpen = true;
    scheduleSyncDetailClose(7200);
  }

  function uniqueSessionIdCandidates(values: string[]): string[] {
    const unique: string[] = [];
    const seen = new Set<string>();

    for (const value of values) {
      const trimmed = value.trim();
      if (!trimmed || seen.has(trimmed)) {
        continue;
      }
      seen.add(trimmed);
      unique.push(trimmed);
    }

    return unique;
  }

  async function renameSessionLocally(sourceCandidates: string[], targetSessionId: string) {
    let lastNotFoundError: unknown = null;

    for (const sourceSessionId of sourceCandidates) {
      try {
        await resonantiaClient.renameSession({
          sourceSessionId,
          targetSessionId,
          allowMerge: false,
        });
        return sourceSessionId;
      } catch (err) {
        if (sessionNotFoundError(err)) {
          lastNotFoundError = err;
          continue;
        }
        throw err;
      }
    }

    throw lastNotFoundError ?? new Error('source session not found');
  }

  async function submitRenameSessionDialog() {
    const targetId = renameSessionTargetId;
    if (!targetId) {
      closeRenameSessionDialog();
      return;
    }

    const fallback = renameSessionFallback || canonicalSessionId(sessionById.get(targetId) ?? null);
    const trimmed = renameSessionDraft.trim();

    if (trimmed.length > 96) {
      renameSessionError = 'session id max length is 96 characters';
      return;
    }

    if (!trimmed) {
      renameSessionError = 'target session id is required';
      return;
    }

    if (trimmed === fallback) {
      closeRenameSessionDialog();
      return;
    }

    renameSessionLoading = true;
    renameSessionError = null;

    try {
      const idFromGraph = targetId.startsWith('s:') ? targetId.slice(2) : targetId;
      const sourceCandidates = uniqueSessionIdCandidates([
        fallback,
        idFromGraph,
        targetId,
      ]);
      let config = await resonantiaClient.getConfig();
      const configuredGateway = (config.gatewayBaseUrl ?? '').trim();
      const usingManagedGateway = isManagedGatewayBaseUrl(configuredGateway);
      let syncAuthToken = (config.gatewayAuthToken ?? '').trim();

      const renamedSourceSessionId = await renameSessionLocally(sourceCandidates, trimmed);

      if (configuredGateway) {
        if (usingManagedGateway) {
          try {
            await refreshGatewayAuthTokenForSync();
            const refreshed = await resonantiaClient.getConfig();
            syncAuthToken = (refreshed.gatewayAuthToken ?? '').trim();
          } catch (tokenError) {
            cloudAuthError = String(tokenError);
          }
        }

        try {
          await renameSessionInGateway(configuredGateway, syncAuthToken, renamedSourceSessionId, trimmed);
        } catch (cloudError) {
          noteDeferredCloudRename(cloudError);
        }
      }

      if (composeSessionId.trim() === fallback || composeSessionId.trim() === renamedSourceSessionId) {
        composeSessionId = trimmed;
      }
      if (calibSessionId.trim() === fallback || calibSessionId.trim() === renamedSourceSessionId) {
        calibSessionId = trimmed;
      }

      await loadGraph();
      closeRenameSessionDialog();
    } catch (err) {
      renameSessionError = String(err);
    } finally {
      renameSessionLoading = false;
    }
  }

  $: sessionById = graph
    ? new Map(graph.sessions.map((session) => [session.id, session]))
    : new Map();

  $: alkahestSessionOptions = graph
    ? graph.sessions
        .map((session) => ({
          ...session,
          id: canonicalSessionId(session),
          label: sessionTitle(session),
        }))
        .sort((left, right) => right.lastModified.localeCompare(left.lastModified))
    : [];

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
      console.error('[graph] load failed', e);
      error = friendlyGraphLoadError(e);
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
      case 'alkahest':
        return '[data-tour-target="alkahest"]';
      case 'importare':
        return '[data-tour-target="alkahest-import"]';
      case 'live':
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
      case 'alkahest':
        return ['[data-tour-target="alkahest"]'];
      case 'importare':
        return ['[data-tour-target="alkahest-import"]', '[data-tour-target="alkahest"]'];
      case 'live':
        return ['[data-tour-target="compose-live"]', '[data-tour-target="compose-toggle"]'];
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
    && (settingsOpen || calibrateOpen || composeOpen || cameraOverlayEngaged);

  $: if (!onboardingOpen) {
    walkthroughStepSatisfied = false;
  }

  $: if (onboardingOpen && walkthroughStep === 'importare' && alkahestMode === 'import') {
    walkthroughStepSatisfied = true;
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
        queueWalkthroughAdvance('alkahest', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            closeTelescope();
            menuOpen = false;
          },
        });
        break;
      case 'alkahest':
        queueWalkthroughAdvance('importare', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            menuOpen = false;
          },
        });
        break;
      case 'importare':
        queueWalkthroughAdvance('live', {
          holdMs: 220,
          cueDelayMs: 260,
          before: () => {
            closeAlkahestPanel();
            composeModeMenuOpen = false;
          },
        });
        break;
      case 'live':
        queueWalkthroughAdvance('complete', {
          holdMs: 220,
          cueDelayMs: 0,
          before: () => {
            composeModeMenuOpen = false;
            closeComposeDrawer();
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
    openCinematicDemo();
  }

  function openCinematicDemo() {
    menuOpen = false;
    dismissOnboarding(false);
    closeTelescope();
    closeTransientUi();
    syncDetailAutoOpen = false;
    syncDetailHover = false;
    adventureReplayFollowup = 'demo';
    adventureOpen = true;
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
    return shortLabel(sessionTitle(session), 3);
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
    const nodeRaw = cardData.nodeDto.raw;
    if (transmutationCache[syntheticId]) return;

    transmuting = true;
    transmuteError = null;
    try {
      const summary = await runManagedAiWithTokenRetry(() => resonantiaClient.summarizeNode(nodeRaw));

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
    closeComposeDrawer();
    calibrateOpen = false;
    settingsOpen = false;

    if (alkahestCameraBefore) {
      camX = targetCamX = alkahestCameraBefore.x;
      camY = targetCamY = alkahestCameraBefore.y;
      camScale = targetCamScale = alkahestCameraBefore.scale;
    }

    alkahestOpen = false;
    alkahestPhase = 'idle';
    alkahestCameraBefore = null;
    closeRenameSessionDialog();
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
    if (cameraOverlayEngaged || !graph || level > 0) return;
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
    if (cameraOverlayEngaged || !graph) return;

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
    if (cameraOverlayEngaged || level !== 1 || !selectedSession) return;
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
    if (cameraOverlayEngaged || level !== 1 || !selectedSession || !graph) return;
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
    if (cameraOverlayEngaged || !graph || level < 1 || !selectedSession) return;
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
    if (cameraOverlayEngaged || level !== 1 || !graph || !selectedSession) return;
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
    if (loading || cameraOverlayEngaged) return;
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
    if (!negativeLayerActive || level !== 0 || cameraOverlayEngaged) return;

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
      || alkahestPhase !== 'idle'
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

    const driftEnabled = negativeLayerActive && level === 0 && !cameraOverlayEngaged && !dragging;
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
    settleAlkahestTransition();

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

    if (alkahestPhase === 'entering' && alkahestCameraBefore) {
      const focus = alkahestFocusTargetFrom(alkahestCameraBefore, level);
      targetCamX = focus.x;
      targetCamY = focus.y;
      targetCamScale = focus.scale;
    }

    if (!cameraOverlayEngaged && level === 0) {
      camX = targetCamX = W() / 2;
      camY = targetCamY = H() / 2;
      camScale = targetCamScale = constellationLayerScale();
    }
  }

  // ── Pointer events ─────────────────────────────────────────────
  function onPointerDown(e: PointerEvent) {
    if (cameraOverlayEngaged || level !== 0) return;
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
    if (cameraOverlayEngaged || !dragging || level !== 0) return;

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
    if (cameraOverlayEngaged) return;
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
  let modelProvider: ModelProvider = 'managed-gateway';
  let ollamaBaseUrl = '';
  let ollamaModel = '';
  let openaiBaseUrl = 'https://api.openai.com';
  let openaiModel = 'gpt-4o-mini';
  let gatewayBaseUrl = '';
  let gatewayAuthToken = '';
  let openaiByoKeyInput = '';
  let openaiByoKeyConfigured = false;
  let openaiByoKeySource: OpenAiByoKeyStatus['source'] = 'unsupported';
  let openaiByoKeyBusy = false;
  let openaiByoKeyError: string | null = null;
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
  const GATEWAY_RENAME_SESSION_PATHS = ['/api/v1/session/rename', '/api/session/rename', '/session/rename'];

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

  function displayGatewayInputFromConfig(configured: string) {
    return isManagedGatewayBaseUrl(configured) ? '' : configured;
  }

  function isManagedGatewayTokenExpiredError(error: unknown) {
    const message = String(error ?? '').toLowerCase();
    if (!message) {
      return false;
    }

    if (message.includes('expiredsignature')) {
      return true;
    }

    if (message.includes('token verification failed') && message.includes('exp=')) {
      return true;
    }

    return message.includes('gateway ai failed: 401') && message.includes('/ai/chat');
  }

  async function refreshGatewayAuthTokenForManagedAi(): Promise<boolean> {
    const config = await resonantiaClient.getConfig();
    const configuredGateway = (config.gatewayBaseUrl ?? '').trim();
    if (!isManagedGatewayBaseUrl(configuredGateway)) {
      return false;
    }

    const status = await getCloudAuthStatus();
    if (!status.available || !status.signedIn) {
      return false;
    }

    const token = await getGatewayAuthToken();
    if (!token) {
      return false;
    }

    gatewayAuthToken = token;
    await resonantiaClient.setGatewayAuthToken(token);
    return true;
  }

  async function runManagedAiWithTokenRetry<T>(operation: () => Promise<T>): Promise<T> {
    try {
      await refreshGatewayAuthTokenForManagedAi();
    } catch (tokenError) {
      cloudAuthError = String(tokenError);
    }

    try {
      return await operation();
    } catch (error) {
      if (!isManagedGatewayTokenExpiredError(error)) {
        throw error;
      }

      const refreshed = await refreshGatewayAuthTokenForManagedAi().catch((tokenError) => {
        cloudAuthError = String(tokenError);
        return false;
      });

      if (!refreshed) {
        throw error;
      }

      return operation();
    }
  }

  function hasPaidCloudTier(tier: string | null) {
    return tier === 'resonant' || tier === 'soulful' || tier === 'subscriber';
  }

  function isLoopbackHostName(hostname: string) {
    const normalized = hostname.trim().toLowerCase();
    return normalized === 'localhost' || normalized === '127.0.0.1' || normalized === '::1' || normalized === '[::1]';
  }

  function normalizeGatewayBaseUrl(value: string) {
    return value.trim().replace(/\/+$/, '');
  }

  function gatewayRenameUrls(baseUrl: string) {
    const normalized = normalizeGatewayBaseUrl(baseUrl);
    return GATEWAY_RENAME_SESSION_PATHS.map((path) => `${normalized}${path}`);
  }

  async function renameSessionInGateway(baseUrl: string, authToken: string, sourceSessionId: string, targetSessionId: string) {
    const urls = gatewayRenameUrls(baseUrl);
    const payload = {
      sourceSessionId,
      targetSessionId,
      allowMerge: false,
    };

    for (let index = 0; index < urls.length; index += 1) {
      const url = urls[index];
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(authToken.trim() ? { Authorization: `Bearer ${authToken.trim()}` } : {}),
        },
        body: JSON.stringify(payload),
      });

      if (response.status === 404 && index < urls.length - 1) {
        continue;
      }

      if (!response.ok) {
        const body = await response.text().catch(() => '');
        throw new Error(`gateway rename failed: ${response.status} ${body}`.trim());
      }

      return;
    }

    throw new Error('gateway rename failed: no compatible rename endpoint found');
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

  async function refreshOpenAiByoKeyStatus() {
    openaiByoKeyError = null;
    try {
      const status = await resonantiaClient.getOpenAiByoKeyStatus();
      openaiByoKeyConfigured = status.configured;
      openaiByoKeySource = status.source;
    } catch {
      openaiByoKeyConfigured = false;
      openaiByoKeySource = 'unsupported';
    }
  }

  async function openSettings() {
    markWalkthroughStepSatisfied('settings');
    menuOpen = false;
    settingsOpen = true;
    settingsLoading = true;
    settingsError = null;
    settingsSaved = false;
    openaiByoKeyInput = '';
    openaiByoKeyError = null;

    try {
      let config = await resonantiaClient.getConfig();
      modelProvider = config.modelProvider;
      gatewayBaseUrl = displayGatewayInputFromConfig(config.gatewayBaseUrl ?? '');
      gatewayAuthToken = config.gatewayAuthToken ?? '';
      ollamaBaseUrl = config.ollamaBaseUrl;
      ollamaModel = config.ollamaModel;
      openaiBaseUrl = config.openaiBaseUrl;
      openaiModel = config.openaiModel;
      advancedOpen = false;
      await refreshOpenAiByoKeyStatus();
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
        const account = await getCloudAccount(gatewayAuthToken).catch(() => null);
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
      const status = await getCloudAuthStatus();
      if (status.signedIn) {
        gatewayAuthToken = '';
        await resonantiaClient.setGatewayAuthToken('');
        await signOutCloud();
      }

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
      await resonantiaClient.setModelProvider(modelProvider);
      await resonantiaClient.setOpenAiConfig(openaiBaseUrl.trim(), openaiModel.trim());
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

  async function saveOpenAiByoKey() {
    const trimmed = openaiByoKeyInput.trim();
    if (!trimmed || openaiByoKeyBusy) {
      return;
    }

    openaiByoKeyBusy = true;
    openaiByoKeyError = null;
    settingsSaved = false;
    try {
      await resonantiaClient.setOpenAiByoKey(trimmed);
      const status = await resonantiaClient.getOpenAiByoKeyStatus();
      openaiByoKeyConfigured = status.configured;
      openaiByoKeySource = status.source;
      if (!status.configured) {
        throw new Error(
          'key saved but could not be loaded from secure storage. please verify your system keychain is available and unlocked.'
        );
      }
      openaiByoKeyInput = '';
      settingsSaved = true;
    } catch (err) {
      openaiByoKeyConfigured = false;
      openaiByoKeyError = String(err);
    } finally {
      openaiByoKeyBusy = false;
    }
  }

  async function clearOpenAiByoKey() {
    if (openaiByoKeyBusy) {
      return;
    }

    openaiByoKeyBusy = true;
    openaiByoKeyError = null;
    settingsSaved = false;
    try {
      await resonantiaClient.clearOpenAiByoKey();
      openaiByoKeyConfigured = false;
      openaiByoKeySource = 'os-keyring';
      openaiByoKeyInput = '';
      settingsSaved = true;
    } catch (err) {
      openaiByoKeyError = String(err);
    } finally {
      openaiByoKeyBusy = false;
    }
  }

  // ── Compose ──────────────────────────────────────────────────
  let composeOpen     = false;
  let composeModeMenuOpen = false;
  let composeMode: ComposeMode = 'live';
  let composeDraft    = '';
  type ComposeContextNodeItem = ComposeContextNode & {
    raw: string;
  };
  type ComposeTabState = {
    id: string;
    title: string;
    sessionId: string;
    originSessionId: string;
    draft: string;
    messages: ComposeMessage[];
    contextSessions: ComposeContextSession[];
    browseSessionId: string;
    injectedNodes: ComposeContextNodeItem[];
    providerUsage: ComposeProviderUsage;
  };
  type ContinueInAppPayload = {
    sessionId: string;
    prompt: string;
    sourceNodeRaw: string;
    threadCandidates: ComposeContextSession[];
  };
  type ComposeContinueTargetMode = 'match-session' | 'active-tab';
  type ComposeContinueRoutingPreference = CrossSessionRoutingPreference;
  type ComposeEncodeSaveOrigin = 'manual' | 'auto';
  const COMPOSE_AUTO_ENCODE_THRESHOLD_MIN = 60;
  const COMPOSE_AUTO_ENCODE_THRESHOLD_MAX = 85;
  const COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT = 72;
  const COMPOSE_MAX_TABS = 3;
  const COMPOSE_RECENT_CONTEXT_SESSION_LIMIT = 5;
  const COMPOSE_CONTEXT_NODE_FETCH_LIMIT = 240;
  const COMPOSE_PROTOCOL_INTRO = [
    'STTP protocol introduction: the following full STTP nodes are active memory context for this chat.',
    'Please gently avoid explaining the protocol unless the user explicitly asks, and otherwise interact directly with the user.',
  ].join('\n');
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
  let composeTabs: ComposeTabState[] = [];
  let composeActiveTabId = '';
  let composeContextSessions: ComposeContextSession[] = [];
  let composeContextOriginSessionId = '';
  let composeContextBrowseSessionId = '';
  let composeContextNodes: ComposeContextNodeItem[] = [];
  let composeContextNodesCache: Record<string, ComposeContextNodeItem[]> = {};
  let composeContextNodesLoading = false;
  let composeContextNodesError: string | null = null;
  let composeInjectedNodes: ComposeContextNodeItem[] = [];
  let composeContinueChooserOpen = false;
  let composeContinueChooserPayload: ContinueInAppPayload | null = null;
  let composeContinueChooserSourceSessionId = '';
  let composeContinueChooserActiveSessionId = '';
  let composeContinueChooserRemember = false;
  let composeContinueRoutingPreference: ComposeContinueRoutingPreference = 'ask';
  let composeAutoEncodeEnabled = false;
  let composeAutoEncodeThresholdPercent = COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT;
  let composeAutoEncodeLastTriggerKey = '';
  let composeProviderUsage: ComposeProviderUsage = {
    promptTokens: 0,
    completionTokens: 0,
    totalTokens: 0,
    responseCount: 0,
    provider: '',
    model: '',
    hasUsageData: false,
  };
  let composeTokenUsage: ComposeTokenUsage = {
    contextTokens: 0,
    draftTokens: 0,
    projectedTurnTokens: 0,
    contextWindowTokens: 128_000,
    usagePercent: 0,
    thresholdPercent: COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT,
    thresholdTokens: Math.round((128_000 * COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT) / 100),
    remainingTokens: 128_000,
  };
  let composeTokenizerRevision = 0;
  let composeLiveUiProps = {} as ComposeLiveUiProps;
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

  function readComposeContinueRoutingPreference(): ComposeContinueRoutingPreference {
    if (typeof localStorage === 'undefined') {
      return 'ask';
    }

    try {
      const raw = localStorage.getItem(COMPOSE_CONTINUE_ROUTING_KEY);
      return raw === 'match-session' || raw === 'active-tab' ? raw : 'ask';
    } catch {
      return 'ask';
    }
  }

  function persistComposeContinueRoutingPreference(value: ComposeContinueRoutingPreference) {
    if (typeof localStorage === 'undefined') {
      return;
    }

    try {
      if (value === 'ask') {
        localStorage.removeItem(COMPOSE_CONTINUE_ROUTING_KEY);
      } else {
        localStorage.setItem(COMPOSE_CONTINUE_ROUTING_KEY, value);
      }
    } catch {
      // Ignore storage failures in strict/private browser contexts.
    }
  }

  function setComposeContinueRoutingPreference(value: ComposeContinueRoutingPreference) {
    composeContinueRoutingPreference = value;
    persistComposeContinueRoutingPreference(value);
  }

  function clearComposeContinueRoutingPreference() {
    setComposeContinueRoutingPreference('ask');
  }

  function clampComposeAutoEncodeThreshold(value: number): number {
    const rounded = Math.round(value);
    return Math.min(COMPOSE_AUTO_ENCODE_THRESHOLD_MAX, Math.max(COMPOSE_AUTO_ENCODE_THRESHOLD_MIN, rounded));
  }

  function readComposeChatSettings(): { autoEncodeEnabled: boolean; autoEncodeThresholdPercent: number } {
    if (typeof localStorage === 'undefined') {
      return {
        autoEncodeEnabled: false,
        autoEncodeThresholdPercent: COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT,
      };
    }

    try {
      const raw = localStorage.getItem(COMPOSE_CHAT_SETTINGS_KEY);
      if (!raw) {
        return {
          autoEncodeEnabled: false,
          autoEncodeThresholdPercent: COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT,
        };
      }

      const parsed = JSON.parse(raw) as {
        autoEncodeEnabled?: unknown;
        autoEncodeThresholdPercent?: unknown;
      };
      const autoEncodeEnabled = parsed.autoEncodeEnabled === true;
      const rawThreshold = Number(parsed.autoEncodeThresholdPercent);
      const thresholdPercent = Number.isFinite(rawThreshold)
        ? clampComposeAutoEncodeThreshold(rawThreshold)
        : COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT;

      return {
        autoEncodeEnabled,
        autoEncodeThresholdPercent: thresholdPercent,
      };
    } catch {
      return {
        autoEncodeEnabled: false,
        autoEncodeThresholdPercent: COMPOSE_AUTO_ENCODE_THRESHOLD_DEFAULT,
      };
    }
  }

  function persistComposeChatSettings() {
    if (typeof localStorage === 'undefined') {
      return;
    }

    try {
      localStorage.setItem(
        COMPOSE_CHAT_SETTINGS_KEY,
        JSON.stringify({
          autoEncodeEnabled: composeAutoEncodeEnabled,
          autoEncodeThresholdPercent: clampComposeAutoEncodeThreshold(composeAutoEncodeThresholdPercent),
        }),
      );
    } catch {
      // Ignore storage failures in strict/private browser contexts.
    }
  }

  function setComposeAutoEncodeEnabled(value: boolean) {
    composeAutoEncodeEnabled = value;
    composeAutoEncodeLastTriggerKey = '';
    persistComposeChatSettings();
  }

  function setComposeAutoEncodeThresholdPercent(value: number) {
    composeAutoEncodeThresholdPercent = clampComposeAutoEncodeThreshold(value);
    composeAutoEncodeLastTriggerKey = '';
    persistComposeChatSettings();
  }

  $: {
    composeTokenizerRevision;
    const outboundMessages = composeApiMessages(composeMessages);
    const systemContext = buildComposeInjectedContextSystemMessage();
    if (systemContext) {
      outboundMessages.unshift(systemContext);
    }

    composeTokenUsage = computeComposeTokenUsage({
      messages: outboundMessages,
      draft: composeDraft,
      modelProvider,
      openaiModel,
      ollamaModel,
      thresholdPercent: composeAutoEncodeThresholdPercent,
    });
  }

  $: composeLiveUiProps = {
    tabs: composeTabs.map((tab) => ({ id: tab.id, title: tab.title, sessionId: tab.sessionId })),
    activeTabId: composeActiveTabId,
    maxTabs: COMPOSE_MAX_TABS,
    contextSessions: composeContextSessions,
    contextOriginSessionId: composeContextOriginSessionId,
    contextBrowseSessionId: composeContextBrowseSessionId,
    contextNodes: composeContextNodes,
    contextNodesLoading: composeContextNodesLoading,
    contextNodesError: composeContextNodesError,
    injectedNodes: composeInjectedNodes.map((node) => ({
      key: node.key,
      title: node.title,
      sessionId: node.sessionId,
      timestamp: node.timestamp,
    })),
    onDraftInput: handleComposeDraftInput,
    selectComposeTab: selectComposeLiveTab,
    createComposeTab: createComposeLiveTab,
    closeComposeTab: closeComposeLiveTab,
    selectContextSession: selectComposeContextSession,
    injectContextNode: injectComposeContextNode,
    removeInjectedNode: removeComposeInjectedNode,
    crossSessionRoutingPreference: composeContinueRoutingPreference,
    clearCrossSessionRoutingPreference: clearComposeContinueRoutingPreference,
    tokenUsage: composeTokenUsage,
    providerUsage: composeProviderUsage,
    calibrationAvec: {
      stability: calibStability,
      friction: calibFriction,
      logic: calibLogic,
      autonomy: calibAutonomy,
      psi: calibrationPsi,
    } as ComposeCalibrationAvec,
    autoEncodeEnabled: composeAutoEncodeEnabled,
    autoEncodeThresholdPercent: composeAutoEncodeThresholdPercent,
    setAutoEncodeEnabled: setComposeAutoEncodeEnabled,
    setAutoEncodeThresholdPercent: setComposeAutoEncodeThresholdPercent,
  };

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
  type AlkahestPhase = 'idle' | 'entering' | 'exiting';
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

  function alkahestAnchorScreenPoint() {
    const compact = W() <= 520;
    return {
      x: W() / 2 - (compact ? 8 : 14),
      y: H() / 2 + (compact ? 16 : 22),
    };
  }

  function alkahestFocusScaleFor(levelValue: number, cameraScale: number) {
    const compact = compactViewport();

    if (levelValue === 1) {
      return Math.max(compact ? 1.42 : 1.64, Math.min(TELESCOPE_WAVE_SCALE, cameraScale * (compact ? 0.63 : 0.69)));
    }

    return Math.max(compact ? 0.82 : 0.9, Math.min(TELESCOPE_CONSTELLATION_SCALE, cameraScale * (compact ? 0.8 : 0.85)));
  }

  function alkahestFocusTargetFrom(camera: CameraState, levelValue: number) {
    const anchor = alkahestAnchorScreenPoint();
    const world = worldAtScreenForCamera(anchor.x, anchor.y, camera);
    const scale = alkahestFocusScaleFor(levelValue, camera.scale);
    return {
      x: world.x,
      y: world.y,
      scale,
    };
  }

  function beginAlkahestEnterTransition() {
    const startCamera = { x: camX, y: camY, scale: camScale };
    alkahestCameraBefore = startCamera;
    const focus = alkahestFocusTargetFrom(startCamera, level);
    targetCamX = focus.x;
    targetCamY = focus.y;
    targetCamScale = focus.scale;
    alkahestPhase = 'entering';
  }

  function beginAlkahestExitTransition() {
    alkahestOpen = false;

    if (!alkahestCameraBefore) {
      alkahestPhase = 'idle';
      return;
    }

    targetCamX = alkahestCameraBefore.x;
    targetCamY = alkahestCameraBefore.y;
    targetCamScale = alkahestCameraBefore.scale;
    alkahestPhase = 'exiting';
  }

  function settleAlkahestTransition() {
    if (alkahestPhase === 'entering' && cameraAtTarget()) {
      alkahestPhase = 'idle';
      alkahestOpen = true;
      return;
    }

    if (alkahestPhase === 'exiting' && cameraAtTarget()) {
      alkahestPhase = 'idle';
      alkahestCameraBefore = null;
    }
  }

  function toggleNegativeLayer() {
    if (level !== 0 || cameraOverlayEngaged) {
      return;
    }

    noteInteraction();
    negativeLayerActive = !negativeLayerActive;
    targetCamScale = constellationLayerScale();
  }

  function openTelescope() {
    if (!telescopeCanAccess || cameraOverlayEngaged) {
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
  $: if (onboardingOpen && walkthroughStep !== 'alkahest' && alkahestCameraEngaged && alkahestPhase !== 'exiting') {
    beginAlkahestExitTransition();
  }
  $: if (level > 1 && alkahestCameraEngaged && alkahestPhase !== 'exiting') {
    beginAlkahestExitTransition();
  }

  type AlkahestScope = 'session' | 'sessions' | 'timeline' | 'resonance';
  type AlkahestMode = 'export' | 'distill' | 'both' | 'import';
  type ResonanceDim = 'stability' | 'friction' | 'logic' | 'autonomy';

  type AlkahestScopeScan = {
    nodes: NodeDto[];
    modelNodes: NodeDto[];
    clipped: boolean;
    sessionCount: number;
    windowLabel: string;
  };

  type AlkahestBundle = {
    kind: 'resonantia-alkahest-bundle';
    version: '1.0';
    exportedAt: string;
    scope: {
      type: AlkahestScope;
      sessionId?: string;
      sessionIds?: string[];
      timelineDays?: number;
      resonanceDim?: ResonanceDim;
      psiMin?: number;
    };
    stats: {
      nodeCount: number;
      sessionCount: number;
      windowLabel: string;
      clippedForModel: boolean;
    };
    model: {
      provider: ModelProvider;
      promptIncluded: boolean;
      targetSessionId: string;
    };
    nodes: Array<{
      sessionId: string;
      timestamp: string;
      tier: string;
      psi: number;
      syncKey: string;
      syntheticId: string;
      raw: string;
    }>;
    superNode: string | null;
  };

  const ALKAHEST_MODEL_NODE_LIMIT = 140;
  const ALKAHEST_DISTILL_CHAR_BUDGET = 220_000;
  const ALKAHEST_DEFAULT_PROMPT = [
    'You are the Alkahest memory distiller for STTP.',
    'Read the full source nodes and extract their most durable meaning into one super node.',
    'Preserve chronology, unresolved threads, and emotional/decision context.',
    'Do not summarize loosely. Encode with concrete technical detail and confidence-weighted fields.',
    'Return exactly one valid STTP node and nothing else.',
  ].join('\n');
  const ALKAHEST_TIMELINE_OPTIONS = TELESCOPE_RANGES.map((range) => ({
    label: range.label,
    days: range.days,
  }));

  let alkahestOpen = false;
  let alkahestScope: AlkahestScope = 'session';
  let alkahestMode: AlkahestMode = 'both';
  let alkahestSessionId = '';
  let alkahestSessionIds: string[] = [];
  let alkahestImportNodeDraft = '';
  let alkahestTimelineDays = 30;
  let alkahestResonanceDim: ResonanceDim = 'logic';
  let alkahestPsiMin = 2.2;
  let alkahestPrompt = '';
  let alkahestTargetSessionId = '';
  let alkahestStoreDistilledNode = true;

  let alkahestLoading = false;
  let alkahestScopeScanning = false;
  let alkahestError: string | null = null;
  let alkahestStatus: string | null = null;
  let alkahestPreflightNodeCount = 0;
  let alkahestPreflightSessionCount = 0;
  let alkahestPreflightWindowLabel = 'scope not scanned yet';
  let alkahestPreflightClipped = false;
  let alkahestPreflightLastScannedAt: string | null = null;
  let alkahestSuperNodePreview = '';
  let alkahestPhase: AlkahestPhase = 'idle';
  let alkahestCameraBefore: CameraState | null = null;

  $: alkahestCameraEngaged = alkahestOpen || alkahestPhase !== 'idle';
  $: cameraOverlayEngaged = telescopeCameraEngaged || alkahestCameraEngaged;

  function defaultAlkahestTargetSessionId() {
    const now = new Date();
    const year = now.getUTCFullYear();
    const month = String(now.getUTCMonth() + 1).padStart(2, '0');
    return `alkahest-${year}-${month}`;
  }

  function shouldDistillForAlkahestMode(mode: AlkahestMode) {
    return mode === 'distill' || mode === 'both';
  }

  function shouldExportForAlkahestMode(mode: AlkahestMode) {
    return mode === 'export' || mode === 'both';
  }

  function dominantResonanceDim(node: NodeDto): ResonanceDim {
    const avec = node.userAvec;
    let bestDim: ResonanceDim = 'stability';
    let bestValue = avec.stability;

    if (avec.friction > bestValue) {
      bestDim = 'friction';
      bestValue = avec.friction;
    }
    if (avec.logic > bestValue) {
      bestDim = 'logic';
      bestValue = avec.logic;
    }
    if (avec.autonomy > bestValue) {
      bestDim = 'autonomy';
    }

    return bestDim;
  }

  function alkahestWindowLabel(nodes: NodeDto[]) {
    if (nodes.length === 0) {
      return 'no nodes matched this scope';
    }

    const sorted = [...nodes].sort((left, right) => left.timestamp.localeCompare(right.timestamp));
    const firstTs = Date.parse(sorted[0].timestamp);
    const lastTs = Date.parse(sorted[sorted.length - 1].timestamp);

    const firstLabel = Number.isFinite(firstTs)
      ? new Date(firstTs).toLocaleDateString([], { month: 'short', day: 'numeric' })
      : sorted[0].timestamp;
    const lastLabel = Number.isFinite(lastTs)
      ? new Date(lastTs).toLocaleDateString([], { month: 'short', day: 'numeric' })
      : sorted[sorted.length - 1].timestamp;

    if (firstLabel === lastLabel) {
      return firstLabel;
    }

    return `${firstLabel} → ${lastLabel}`;
  }

  async function fetchAlkahestSourceNodes(): Promise<NodeDto[]> {
    const requestedSession = alkahestScope === 'session'
      ? (alkahestSessionId.trim() || canonicalSessionId(selectedSession))
      : '';
    const requestedSessionIds = alkahestScope === 'sessions'
      ? Array.from(new Set(alkahestSessionIds.map((id) => id.trim()).filter(Boolean)))
      : [];

    if (alkahestScope === 'session' && !requestedSession) {
      throw new Error('choose a session before running Alkahest session scope');
    }

    if (alkahestScope === 'sessions' && requestedSessionIds.length === 0) {
      throw new Error('choose at least one session for multi-session scope');
    }

    if (requestedSession) {
      alkahestSessionId = requestedSession;
    }

    if (requestedSessionIds.length > 0) {
      alkahestSessionIds = requestedSessionIds;
    }

    if (alkahestScope === 'sessions') {
      const mergedNodes = new Map<string, NodeDto>();
      for (const requestedId of requestedSessionIds) {
        const listed = await resonantiaClient.listNodes(400, requestedId);
        if (listed.transport) {
          lastTransportLabel = listed.transport;
        }
        applySourceBadge(listed.source, listed.transport ?? lastTransportLabel);
        for (const node of listed.nodes) {
          const dedupeKey = `${node.sessionId}|${node.syntheticId}|${node.syncKey}|${node.timestamp}`;
          if (!mergedNodes.has(dedupeKey)) {
            mergedNodes.set(dedupeKey, node);
          }
        }
      }
      return [...mergedNodes.values()];
    }

    const listed = await resonantiaClient.listNodes(400, requestedSession || undefined);
    if (listed.transport) {
      lastTransportLabel = listed.transport;
    }
    applySourceBadge(listed.source, listed.transport ?? lastTransportLabel);
    return listed.nodes;
  }

  function filterAlkahestScopeNodes(nodes: NodeDto[]): NodeDto[] {
    const sessionId = alkahestSessionId.trim();
    const sessionIds = new Set(alkahestSessionIds.map((id) => id.trim()).filter(Boolean));

    return nodes.filter((node) => {
      if (alkahestScope === 'session') {
        return sessionId ? node.sessionId === sessionId : false;
      }

      if (alkahestScope === 'sessions') {
        return sessionIds.has(node.sessionId);
      }

      if (alkahestScope === 'timeline') {
        if (alkahestTimelineDays >= Number.MAX_SAFE_INTEGER) {
          return true;
        }
        return daysAgoFromTimestamp(node.timestamp) <= alkahestTimelineDays;
      }

      const dominant = dominantResonanceDim(node);
      return dominant === alkahestResonanceDim && node.psi >= alkahestPsiMin;
    });
  }

  function buildAlkahestDistillInput(nodes: NodeDto[], scan: AlkahestScopeScan) {
    const prime = (alkahestPrompt.trim() || ALKAHEST_DEFAULT_PROMPT).trim();
    const base = [
      prime,
      '',
      `scope_type: ${alkahestScope}`,
      `scope_node_count: ${scan.nodes.length}`,
      `scope_session_count: ${scan.sessionCount}`,
      `scope_window: ${scan.windowLabel}`,
      `clipped_for_model: ${scan.clipped ? 'true' : 'false'}`,
      '',
      'Source nodes follow.',
      'Use them as canonical memory and output one STTP node only.',
      '',
    ].join('\n');

    let consumed = base.length;
    const chunks: string[] = [base];

    for (let index = 0; index < nodes.length; index += 1) {
      const node = nodes[index];
      const chunk = [
        `--- node ${index + 1} ---`,
        `session_id: ${node.sessionId}`,
        `timestamp: ${node.timestamp}`,
        `tier: ${node.tier}`,
        `psi: ${node.psi.toFixed(4)}`,
        'raw:',
        node.raw,
        '',
      ].join('\n');

      if (consumed + chunk.length > ALKAHEST_DISTILL_CHAR_BUDGET) {
        break;
      }

      chunks.push(chunk);
      consumed += chunk.length;
    }

    chunks.push('Return only one valid STTP node. No commentary.');
    return chunks.join('\n');
  }

  function buildAlkahestBundle(scan: AlkahestScopeScan, superNode: string | null): AlkahestBundle {
    const sessionId = alkahestSessionId.trim();
    const sessionIds = Array.from(new Set(alkahestSessionIds.map((id) => id.trim()).filter(Boolean)));
    const targetSessionId = alkahestTargetSessionId.trim() || defaultAlkahestTargetSessionId();

    return {
      kind: 'resonantia-alkahest-bundle',
      version: '1.0',
      exportedAt: new Date().toISOString(),
      scope: {
        type: alkahestScope,
        ...(alkahestScope === 'session' ? { sessionId } : {}),
        ...(alkahestScope === 'sessions' ? { sessionIds } : {}),
        ...(alkahestScope === 'timeline' ? { timelineDays: alkahestTimelineDays } : {}),
        ...(alkahestScope === 'resonance'
          ? {
            resonanceDim: alkahestResonanceDim,
            psiMin: alkahestPsiMin,
          }
          : {}),
      },
      stats: {
        nodeCount: scan.nodes.length,
        sessionCount: scan.sessionCount,
        windowLabel: scan.windowLabel,
        clippedForModel: scan.clipped,
      },
      model: {
        provider: modelProvider,
        promptIncluded: shouldDistillForAlkahestMode(alkahestMode),
        targetSessionId,
      },
      nodes: scan.nodes.map((node) => ({
        sessionId: node.sessionId,
        timestamp: node.timestamp,
        tier: node.tier,
        psi: node.psi,
        syncKey: node.syncKey,
        syntheticId: node.syntheticId,
        raw: node.raw,
      })),
      superNode,
    };
  }

  function downloadAlkahestBundle(bundle: AlkahestBundle) {
    if (typeof document === 'undefined') {
      throw new Error('json export is unavailable in this runtime');
    }

    const stamp = bundle.exportedAt.slice(0, 10);
    const fileName = `alkahest-${bundle.scope.type}-${stamp}.json`;
    const blob = new Blob([JSON.stringify(bundle, null, 2)], { type: 'application/json' });
    const href = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = href;
    anchor.download = fileName;
    anchor.style.display = 'none';
    document.body.appendChild(anchor);
    anchor.click();
    document.body.removeChild(anchor);
    URL.revokeObjectURL(href);
  }

  async function collectAlkahestScopeScan(): Promise<AlkahestScopeScan> {
    const sourceNodes = await fetchAlkahestSourceNodes();
    const filtered = filterAlkahestScopeNodes(sourceNodes).sort((left, right) => left.timestamp.localeCompare(right.timestamp));
    const clipped = filtered.length > ALKAHEST_MODEL_NODE_LIMIT;
    const modelNodes = clipped ? filtered.slice(-ALKAHEST_MODEL_NODE_LIMIT) : filtered;
    const sessionCount = new Set(filtered.map((node) => node.sessionId)).size;
    const windowLabel = alkahestWindowLabel(filtered);

    alkahestPreflightNodeCount = filtered.length;
    alkahestPreflightSessionCount = sessionCount;
    alkahestPreflightWindowLabel = windowLabel;
    alkahestPreflightClipped = clipped;
    alkahestPreflightLastScannedAt = new Date().toISOString();

    return {
      nodes: filtered,
      modelNodes,
      clipped,
      sessionCount,
      windowLabel,
    };
  }

  function primeAlkahestDefaults() {
    if (!alkahestSessionId.trim() && selectedSession) {
      alkahestSessionId = canonicalSessionId(selectedSession);
    }
    if (alkahestSessionIds.length === 0 && selectedSession) {
      alkahestSessionIds = [canonicalSessionId(selectedSession)];
    }
    if (!alkahestTargetSessionId.trim()) {
      alkahestTargetSessionId = defaultAlkahestTargetSessionId();
    }
    if (!alkahestPrompt.trim()) {
      alkahestPrompt = ALKAHEST_DEFAULT_PROMPT;
    }
  }

  function toggleAlkahestOpen() {
    if (alkahestCameraEngaged) {
      closeAlkahestPanel();
      return;
    }

    if (level > 1 || cameraOverlayEngaged) {
      return;
    }

    noteInteraction();
    markWalkthroughStepSatisfied('alkahest');
    closeTransientUi();
    composeModeMenuOpen = false;
    syncDetailAutoOpen = false;
    syncDetailHover = false;

    primeAlkahestDefaults();
    beginAlkahestEnterTransition();
  }

  function closeAlkahestPanel() {
    if (!alkahestCameraEngaged) {
      alkahestOpen = false;
      alkahestPhase = 'idle';
      alkahestCameraBefore = null;
      return;
    }

    noteInteraction();
    beginAlkahestExitTransition();
  }

  function resetAlkahestFlow() {
    alkahestScope = 'session';
    alkahestMode = 'both';
    alkahestSessionId = selectedSession ? canonicalSessionId(selectedSession) : '';
    alkahestSessionIds = selectedSession ? [canonicalSessionId(selectedSession)] : [];
    alkahestImportNodeDraft = '';
    alkahestTimelineDays = 30;
    alkahestResonanceDim = 'logic';
    alkahestPsiMin = 2.2;
    alkahestPrompt = ALKAHEST_DEFAULT_PROMPT;
    alkahestTargetSessionId = defaultAlkahestTargetSessionId();
    alkahestStoreDistilledNode = true;

    alkahestError = null;
    alkahestStatus = null;
    alkahestPreflightNodeCount = 0;
    alkahestPreflightSessionCount = 0;
    alkahestPreflightWindowLabel = 'scope not scanned yet';
    alkahestPreflightClipped = false;
    alkahestPreflightLastScannedAt = null;
    alkahestSuperNodePreview = '';

    closeAlkahestPanel();
  }

  async function scanAlkahestScope() {
    if (alkahestLoading || alkahestScopeScanning) {
      return;
    }

    alkahestScopeScanning = true;
    alkahestError = null;
    alkahestStatus = null;
    try {
      const scan = await collectAlkahestScopeScan();
      alkahestStatus = `scope ready · ${scan.nodes.length} nodes`;
    } catch (err) {
      alkahestError = String(err);
    } finally {
      alkahestScopeScanning = false;
    }
  }

  async function runAlkahestFlow() {
    if (alkahestLoading || alkahestScopeScanning) {
      return;
    }

    alkahestLoading = true;
    alkahestError = null;
    alkahestStatus = null;

    try {
      let superNode: string | null = null;
      let storeStatus: string | null = null;
      const importMode = alkahestMode === 'import';
      const scan = importMode ? null : await collectAlkahestScopeScan();

      if (!importMode && scan && scan.nodes.length === 0) {
        throw new Error('no nodes matched this Alkahest scope');
      }

      if (importMode) {
        const targetSessionId = alkahestTargetSessionId.trim() || defaultAlkahestTargetSessionId();
        const rawNode = alkahestImportNodeDraft.trim();

        if (!rawNode) {
          throw new Error('paste one complete STTP node before importing');
        }

        alkahestTargetSessionId = targetSessionId;
        const stored = await resonantiaClient.storeContext({
          node: rawNode,
          sessionId: targetSessionId,
        });

        if (!stored.valid) {
          throw new Error(stored.validationError ?? 'imported node failed STTP validation');
        }

        const upsertStatus = stored.upsertStatus ?? (stored.duplicateSkipped ? 'duplicate' : 'stored');
        storeStatus = stored.duplicateSkipped ? 'duplicate skipped' : upsertStatus;
        superNode = rawNode;
        alkahestSuperNodePreview = rawNode;
        await loadGraph();
      }

      if (!importMode && shouldDistillForAlkahestMode(alkahestMode) && scan) {
        const targetSessionId = alkahestTargetSessionId.trim() || defaultAlkahestTargetSessionId();
        alkahestTargetSessionId = targetSessionId;

        const content = buildAlkahestDistillInput(scan.modelNodes, scan);
        const encodedNode = await runManagedAiWithTokenRetry(() =>
          resonantiaClient.encodeCompose({
            sessionId: targetSessionId,
            messages: [
              {
                role: 'user',
                content,
              },
            ],
          })
        );

        superNode = encodedNode.trim();
        alkahestSuperNodePreview = superNode;

        if (alkahestStoreDistilledNode) {
          const stored = await resonantiaClient.storeContext({
            node: superNode,
            sessionId: targetSessionId,
          });
          if (!stored.valid) {
            throw new Error(stored.validationError ?? 'distilled node failed STTP validation');
          }

          const upsertStatus = stored.upsertStatus ?? (stored.duplicateSkipped ? 'duplicate' : 'stored');
          storeStatus = stored.duplicateSkipped ? 'duplicate skipped' : upsertStatus;
          await loadGraph();
        }
      }

      if (!importMode && shouldExportForAlkahestMode(alkahestMode) && scan) {
        const bundle = buildAlkahestBundle(scan, superNode);
        downloadAlkahestBundle(bundle);
      }

      const outcomes: string[] = [];
      if (importMode) {
        outcomes.push('node imported');
      }
      if (!importMode && shouldDistillForAlkahestMode(alkahestMode)) {
        outcomes.push('super node distilled');
      }
      if (storeStatus) {
        outcomes.push(`stored ${storeStatus}`);
      }
      if (!importMode && shouldExportForAlkahestMode(alkahestMode)) {
        outcomes.push('bundle exported');
      }

      alkahestStatus = outcomes.join(' · ') || 'alkahest complete';
    } catch (err) {
      alkahestError = String(err);
    } finally {
      alkahestLoading = false;
    }
  }

  async function copyAlkahestSuperNode() {
    const source = alkahestSuperNodePreview.trim();
    if (!source) {
      return;
    }

    try {
      await copyTextToClipboard(source);
      alkahestStatus = 'super node copied';
      alkahestError = null;
    } catch (err) {
      alkahestError = String(err);
    }
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

  function composeSessionLabel(sessionId: string) {
    const normalized = sessionId.trim();
    if (!normalized) {
      return '';
    }

    const graphLabel = graph?.sessions.find((session) => session.id === sessionKey(normalized))?.label ?? '';
    const base = (graphLabel || normalized).trim();
    return base.replace(/_/g, ' ');
  }

  function composeContextSummary(rawNode: string) {
    const summaryMatch = rawNode.match(/context_summary(?:\.[0-9]+)?\s*:\s*([^,\n}⟩]+)/i);
    if (!summaryMatch) {
      return '';
    }

    const cleaned = summaryMatch[1]
      .trim()
      .replace(/^['"]|['"]$/g, '')
      .replace(/[_-]+/g, ' ')
      .replace(/\s+/g, ' ');

    return cleaned;
  }

  function composeNodePreview(rawNode: string) {
    const contextSummary = composeContextSummary(rawNode);
    if (contextSummary) {
      return contextSummary.length > 210 ? `${contextSummary.slice(0, 210)}...` : contextSummary;
    }

    const normalized = rawNode.replace(/\s+/g, ' ').trim();
    if (!normalized) {
      return '(empty node)';
    }

    return 'summary unavailable';
  }

  function normalizeComposeContextSessions(candidates: ComposeContextSession[]) {
    const bySessionId = new Map<string, ComposeContextSession>();

    for (const candidate of candidates) {
      const sessionId = candidate.sessionId.trim();
      if (!sessionId) {
        continue;
      }

      const existing = bySessionId.get(sessionId);
      if (existing) {
        continue;
      }

      bySessionId.set(sessionId, {
        sessionId,
        label: (candidate.label.trim() || composeSessionLabel(sessionId) || sessionId).replace(/_/g, ' '),
      });
    }

    return [...bySessionId.values()];
  }

  function buildComposeTabId() {
    return `compose-${Date.now()}-${Math.round(Math.random() * 1_000_000).toString(36)}`;
  }

  function buildComposeTabTitle(sessionId: string, fallbackIndex: number) {
    const label = composeSessionLabel(sessionId);
    return label || `thread ${fallbackIndex + 1}`;
  }

  function composeRecentTimelineContextSessions(limit = COMPOSE_RECENT_CONTEXT_SESSION_LIMIT): ComposeContextSession[] {
    if (!graph) {
      return [];
    }

    return graph.sessions
      .slice()
      .sort((left, right) => right.lastModified.localeCompare(left.lastModified))
      .map((session) => {
        const sessionId = canonicalSessionId(session).trim();
        return {
          sessionId,
          label: composeSessionLabel(sessionId) || sessionId,
        };
      })
      .filter((candidate) => Boolean(candidate.sessionId))
      .slice(0, limit);
  }

  function buildDefaultComposeTabState(
    seedSessionId: string,
    options: { includeRecentContext?: boolean } = {},
  ): ComposeTabState {
    const normalizedSession = seedSessionId.trim();
    const includeRecentContext = options.includeRecentContext === true;
    const seedContextSessions = normalizedSession
      ? [
        {
          sessionId: normalizedSession,
          label: composeSessionLabel(normalizedSession) || normalizedSession,
        },
      ]
      : [];
    const contextSessions = includeRecentContext
      ? normalizeComposeContextSessions([
        ...seedContextSessions,
        ...composeRecentTimelineContextSessions(),
      ])
      : seedContextSessions;

    return {
      id: buildComposeTabId(),
      title: buildComposeTabTitle(normalizedSession, composeTabs.length),
      sessionId: normalizedSession,
      originSessionId: normalizedSession,
      draft: '',
      messages: [],
      contextSessions,
      browseSessionId: '',
      injectedNodes: [],
      providerUsage: createEmptyComposeProviderUsage(),
    };
  }

  function snapshotActiveComposeTab() {
    if (composeMode !== 'live' || !composeActiveTabId) {
      return;
    }

    const activeIndex = composeTabs.findIndex((tab) => tab.id === composeActiveTabId);
    if (activeIndex < 0) {
      return;
    }

    const current = composeTabs[activeIndex];
    const normalizedSession = composeSessionId.trim();
    const nextTitle = composeSessionLabel(normalizedSession) || current.title || buildComposeTabTitle(normalizedSession, activeIndex);

    const snapshot: ComposeTabState = {
      ...current,
      title: nextTitle,
      sessionId: normalizedSession,
      originSessionId: composeContextOriginSessionId,
      draft: composeDraft,
      messages: [...composeMessages],
      contextSessions: [...composeContextSessions],
      browseSessionId: composeContextBrowseSessionId,
      injectedNodes: [...composeInjectedNodes],
      providerUsage: { ...composeProviderUsage },
    };

    composeTabs = [
      ...composeTabs.slice(0, activeIndex),
      snapshot,
      ...composeTabs.slice(activeIndex + 1),
    ];
  }

  function configureComposeContextSessions(
    candidates: ComposeContextSession[],
    preferredSessionId = '',
    autoSelectPreferred = false,
  ) {
    composeContextSessions = normalizeComposeContextSessions(candidates);

    if (composeContextSessions.length === 0) {
      composeContextBrowseSessionId = '';
      composeContextNodes = [];
      return;
    }

    const preferred = preferredSessionId.trim();
    if (
      autoSelectPreferred
      && preferred
      && composeContextSessions.some((candidate) => candidate.sessionId === preferred)
    ) {
      composeContextBrowseSessionId = preferred;
      return;
    }

    if (composeContextSessions.some((candidate) => candidate.sessionId === composeContextBrowseSessionId)) {
      return;
    }

    composeContextBrowseSessionId = '';
    composeContextNodes = [];
  }

  function ensureComposeContextSession(sessionId: string) {
    const normalized = sessionId.trim();
    if (!normalized) {
      return;
    }

    if (composeContextSessions.some((candidate) => candidate.sessionId === normalized)) {
      return;
    }

    configureComposeContextSessions(
      [
        ...composeContextSessions,
        {
          sessionId: normalized,
          label: composeSessionLabel(normalized) || normalized,
        },
      ],
      composeContextBrowseSessionId,
      Boolean(composeContextBrowseSessionId),
    );
  }

  async function loadComposeContextNodes(sessionId: string) {
    const normalized = sessionId.trim();
    if (!normalized) {
      composeContextNodes = [];
      composeContextNodesError = null;
      return;
    }

    const cached = composeContextNodesCache[normalized];
    if (cached) {
      composeContextNodes = cached;
      composeContextNodesError = null;
      return;
    }

    composeContextNodesLoading = true;
    composeContextNodesError = null;

    try {
      const listed = await resonantiaClient.listNodes(COMPOSE_CONTEXT_NODE_FETCH_LIMIT, normalized);
      if (listed.transport) {
        lastTransportLabel = listed.transport;
      }
      applySourceBadge(listed.source, listed.transport ?? lastTransportLabel);

      const mapped = listed.nodes
        .sort((left, right) => right.timestamp.localeCompare(left.timestamp))
        .map((node) => ({
          key: `${node.sessionId}|${node.syntheticId}|${node.syncKey}|${node.timestamp}`,
          sessionId: node.sessionId,
          title: `${node.tier} · Ψ ${node.psi.toFixed(2)}`,
          timestamp: node.timestamp,
          tier: node.tier,
          psi: node.psi,
          preview: composeNodePreview(node.raw),
          raw: node.raw,
        }));

      composeContextNodesCache = {
        ...composeContextNodesCache,
        [normalized]: mapped,
      };
      composeContextNodes = mapped;
    } catch (err) {
      composeContextNodes = [];
      composeContextNodesError = String(err);
    } finally {
      composeContextNodesLoading = false;
    }
  }

  function selectComposeContextSession(sessionId: string) {
    const normalized = sessionId.trim();
    if (!normalized) {
      return;
    }

    composeContextBrowseSessionId = normalized;
    void loadComposeContextNodes(normalized);
    snapshotActiveComposeTab();
  }

  function injectComposeContextNode(nodeKey: string) {
    const selected = composeContextNodes.find((node) => node.key === nodeKey);
    if (!selected) {
      return;
    }

    if (!composeInjectedNodes.some((node) => node.key === selected.key)) {
      composeInjectedNodes = [...composeInjectedNodes, selected];
    }

    composeContextNodesError = null;
    snapshotActiveComposeTab();

    void sendComposeMessage(buildComposeNodeInjectionPrompt(selected), {
      includeInjectedSystemContext: false,
    });
  }

  function removeComposeInjectedNode(nodeKey: string) {
    composeInjectedNodes = composeInjectedNodes.filter((node) => node.key !== nodeKey);
    snapshotActiveComposeTab();
  }

  function seedComposeInjectedNode(sessionId: string, rawNode: string) {
    const normalizedSession = sessionId.trim();
    const normalizedRaw = rawNode.trim();
    if (!normalizedSession || !normalizedRaw) {
      return;
    }

    const key = `${normalizedSession}:${normalizedRaw.length}:${Math.round(hashUnit(`${normalizedSession}:${normalizedRaw}`) * 1_000_000_000)}`;
    if (composeInjectedNodes.some((node) => node.key === key)) {
      return;
    }

    composeInjectedNodes = [
      ...composeInjectedNodes,
      {
        key,
        sessionId: normalizedSession,
        title: composeSessionLabel(normalizedSession) || normalizedSession,
        timestamp: new Date().toISOString(),
        tier: 'raw',
        psi: 0,
        preview: composeNodePreview(normalizedRaw),
        raw: normalizedRaw,
      },
    ];
  }

  function loadComposeTab(tabId: string) {
    const tab = composeTabs.find((entry) => entry.id === tabId);
    if (!tab) {
      return;
    }

    composeActiveTabId = tab.id;
    composeSessionId = tab.sessionId;
    composeContextOriginSessionId = tab.originSessionId || tab.sessionId;
    composeDraft = tab.draft;
    composeMessages = [...tab.messages];
    configureComposeContextSessions(tab.contextSessions, tab.browseSessionId, Boolean(tab.browseSessionId));
    composeInjectedNodes = [...tab.injectedNodes];
    composeProviderUsage = {
      ...createEmptyComposeProviderUsage(),
      ...(tab.providerUsage ?? {}),
    };
    composeContextNodesError = null;
    composeContextNodes = [];

    if (composeContextBrowseSessionId) {
      void loadComposeContextNodes(composeContextBrowseSessionId);
    }
  }

  function ensureComposeLiveTab() {
    if (composeTabs.length === 0) {
      const seeded = buildDefaultComposeTabState(canonicalSessionId(selectedSession), {
        includeRecentContext: true,
      });
      composeTabs = [seeded];
      loadComposeTab(seeded.id);
      return;
    }

    if (!composeActiveTabId || !composeTabs.some((tab) => tab.id === composeActiveTabId)) {
      loadComposeTab(composeTabs[0].id);
      return;
    }

    loadComposeTab(composeActiveTabId);
  }

  function createComposeLiveTab() {
    if (composeTabs.length >= COMPOSE_MAX_TABS) {
      composeContextNodesError = `up to ${COMPOSE_MAX_TABS} live threads are available`;
      return;
    }

    snapshotActiveComposeTab();
    const seedSession = composeSessionId.trim() || canonicalSessionId(selectedSession);
    const nextTab = buildDefaultComposeTabState(seedSession);
    composeTabs = [...composeTabs, nextTab];
    composeContextNodesError = null;
    loadComposeTab(nextTab.id);
  }

  function selectComposeLiveTab(tabId: string) {
    if (!tabId || tabId === composeActiveTabId) {
      return;
    }

    snapshotActiveComposeTab();
    loadComposeTab(tabId);
  }

  function closeComposeLiveTab(tabId: string) {
    if (composeTabs.length <= 1) {
      return;
    }

    snapshotActiveComposeTab();
    const closeIndex = composeTabs.findIndex((tab) => tab.id === tabId);
    if (closeIndex < 0) {
      return;
    }

    const wasActive = composeActiveTabId === tabId;
    const nextTabs = composeTabs.filter((tab) => tab.id !== tabId);
    composeTabs = nextTabs;

    if (wasActive) {
      const fallbackIndex = Math.max(0, closeIndex - 1);
      const fallback = nextTabs[fallbackIndex] ?? nextTabs[0];
      if (fallback) {
        loadComposeTab(fallback.id);
      }
      return;
    }

    if (!nextTabs.some((tab) => tab.id === composeActiveTabId) && nextTabs[0]) {
      loadComposeTab(nextTabs[0].id);
    }
  }

  function closeComposeDrawer() {
    snapshotActiveComposeTab();
    closeComposeContinueChooser();
    composeOpen = false;
  }

  function handleComposeDraftInput() {
    snapshotActiveComposeTab();
  }

  function buildComposeInjectedContextSystemMessage(): ChatMessage | null {
    if (composeInjectedNodes.length === 0) {
      return null;
    }

    const blocks: string[] = [];
    for (const node of composeInjectedNodes) {
      blocks.push(
        `context_session_id: ${node.sessionId}`,
        'full_node:',
        node.raw,
        '',
      );
    }

    return {
      role: 'system',
      content: [COMPOSE_PROTOCOL_INTRO, '', ...blocks].join('\n'),
    };
  }

  function buildComposeNodeInjectionPrompt(node: ComposeContextNodeItem): string {
    return [
      'continue this same thread with the following injected raw node as additional context.',
      `context_session_id: ${node.sessionId}`,
      'full_node:',
      node.raw,
    ].join('\n');
  }

  function openCompose(mode: 'live' = 'live') {
    snapshotActiveComposeTab();
    closeComposeContinueChooser();
    composeModeMenuOpen = false;
    composeMode = mode;

    if (mode === 'live') {
      ensureComposeLiveTab();
    }

    composeError = null;
    composeResult = null;
    composeLoading = false;
    composeReplyLoading = false;
    composeEncodePromptSent = false;
    composePromptCopyLoading = false;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;
    composeContextNodesError = null;
    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
    composePasteNodeLoading = false;
    composeOpen = true;

    if (mode === 'live' && composeContextBrowseSessionId) {
      void loadComposeContextNodes(composeContextBrowseSessionId);
    }
  }

  function closeComposeContinueChooser() {
    composeContinueChooserOpen = false;
    composeContinueChooserPayload = null;
    composeContinueChooserSourceSessionId = '';
    composeContinueChooserActiveSessionId = '';
    composeContinueChooserRemember = false;
  }

  function openComposeContinueChooser(detail: ContinueInAppPayload, activeSessionId: string) {
    composeContinueChooserPayload = detail;
    composeContinueChooserSourceSessionId = detail.sessionId;
    composeContinueChooserActiveSessionId = activeSessionId;
    composeContinueChooserRemember = false;
    composeContinueChooserOpen = true;
  }

  function resolveContinueTargetTabId(sourceSessionId: string, targetMode: ComposeContinueTargetMode): string {
    if (targetMode === 'active-tab') {
      if (composeActiveTabId && composeTabs.some((tab) => tab.id === composeActiveTabId)) {
        return composeActiveTabId;
      }

      if (composeTabs[0]) {
        return composeTabs[0].id;
      }

      if (composeTabs.length < COMPOSE_MAX_TABS) {
        const seedSession = sourceSessionId || canonicalSessionId(selectedSession);
        const nextTab = buildDefaultComposeTabState(seedSession);
        composeTabs = [...composeTabs, nextTab];
        return nextTab.id;
      }

      return '';
    }

    let targetTabId = composeTabs.find((tab) => tab.sessionId === sourceSessionId)?.id ?? '';
    if (!targetTabId && composeTabs.length < COMPOSE_MAX_TABS) {
      const nextTab = buildDefaultComposeTabState(sourceSessionId);
      composeTabs = [...composeTabs, nextTab];
      targetTabId = nextTab.id;
    }
    if (!targetTabId) {
      targetTabId = composeActiveTabId || composeTabs[0]?.id || '';
    }
    return targetTabId;
  }

  function applyContinueThreadInCompose(
    detail: ContinueInAppPayload,
    targetMode: ComposeContinueTargetMode,
  ) {
    const sourceSessionId = detail.sessionId.trim();
    const prompt = detail.prompt.trim();
    const sourceNodeRaw = detail.sourceNodeRaw.trim();
    const incomingSessions = normalizeComposeContextSessions(detail.threadCandidates ?? []);

    if (!sourceSessionId || !prompt || !sourceNodeRaw) {
      return;
    }

    closeCard();
    closeComposeContinueChooser();
    snapshotActiveComposeTab();

    const targetTabId = resolveContinueTargetTabId(sourceSessionId, targetMode);
    if (!targetTabId) {
      return;
    }

    composeModeMenuOpen = false;
    composeMode = 'live';
    composeError = null;
    composeResult = null;
    composeLoading = false;
    composeEncodePromptSent = false;
    composePromptCopyLoading = false;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;

    loadComposeTab(targetTabId);
    const targetSessionId = targetMode === 'active-tab'
      ? (composeSessionId.trim() || sourceSessionId)
      : sourceSessionId;

    composeSessionId = targetSessionId;
    composeContextOriginSessionId = sourceSessionId;
    configureComposeContextSessions(
      [
        ...composeContextSessions,
        {
          sessionId: targetSessionId,
          label: composeSessionLabel(targetSessionId) || targetSessionId,
        },
        {
          sessionId: sourceSessionId,
          label: composeSessionLabel(sourceSessionId) || sourceSessionId,
        },
        ...incomingSessions,
      ],
      '',
      false,
    );
    composeContextBrowseSessionId = '';
    composeContextNodes = [];
    seedComposeInjectedNode(sourceSessionId, sourceNodeRaw);

    if (composeContextBrowseSessionId) {
      void loadComposeContextNodes(composeContextBrowseSessionId);
    }

    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
    composePasteNodeLoading = false;
    composeOpen = true;
    snapshotActiveComposeTab();

    void sendComposeMessage(prompt, {
      includeInjectedSystemContext: false,
      crossSessionSourceSessionId: targetMode === 'active-tab' ? sourceSessionId : '',
    });
  }

  function chooseComposeContinueTarget(targetMode: ComposeContinueTargetMode) {
    const pending = composeContinueChooserPayload;
    if (!pending) {
      closeComposeContinueChooser();
      return;
    }

    if (composeContinueChooserRemember) {
      setComposeContinueRoutingPreference(targetMode);
    }

    applyContinueThreadInCompose(pending, targetMode);
  }

  function continueThreadInCompose(event: CustomEvent<ContinueInAppPayload>) {
    const sessionId = event.detail.sessionId.trim();
    const prompt = event.detail.prompt.trim();
    const sourceNodeRaw = event.detail.sourceNodeRaw.trim();
    const threadCandidates = normalizeComposeContextSessions(event.detail.threadCandidates ?? []);

    if (!sessionId || !prompt || !sourceNodeRaw) {
      return;
    }

    const normalizedDetail: ContinueInAppPayload = {
      sessionId,
      prompt,
      sourceNodeRaw,
      threadCandidates,
    };

    const activeSessionId = composeTabs.find((tab) => tab.id === composeActiveTabId)?.sessionId.trim() ?? '';
    if (activeSessionId && activeSessionId !== sessionId) {
      if (composeContinueRoutingPreference !== 'ask') {
        applyContinueThreadInCompose(normalizedDetail, composeContinueRoutingPreference);
        return;
      }

      openComposeContinueChooser(normalizedDetail, activeSessionId);
      return;
    }

    applyContinueThreadInCompose(normalizedDetail, 'match-session');
  }

  function toggleComposeModeMenu() {
    composeModeMenuOpen = !composeModeMenuOpen;
  }

  function openComposeLive() {
    markWalkthroughStepSatisfied('live');
    composeModeMenuOpen = false;
    openCompose('live');
  }

  function switchComposeToLive() {
    markWalkthroughStepSatisfied('live');
    composeMode = 'live';
    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
    ensureComposeLiveTab();

    if (composeContextBrowseSessionId) {
      void loadComposeContextNodes(composeContextBrowseSessionId);
    }
  }

  function handleComposeSessionInput() {
    if (composeError && /session id is required/i.test(composeError) && composeSessionId.trim()) {
      composeError = null;
    }

    snapshotActiveComposeTab();
  }

  function clearComposeConversation() {
    composeDraft = '';
    composeMessages = [];
    composeProviderUsage = createEmptyComposeProviderUsage();
    composeAutoEncodeLastTriggerKey = '';
    composeError = null;
    composeResult = null;
    composeEncodePromptSent = false;
    clearComposePromptCopiedTimer();
    composePromptCopied = false;
    composePromptCopyError = null;
    composeContextNodesError = null;
    composePasteNodeOpen = false;
    composePasteNodeDraft = '';
    snapshotActiveComposeTab();
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

  function composeAutoEncodeSignature(): string {
    if (composeMessages.length === 0) {
      return '';
    }

    const last = composeMessages[composeMessages.length - 1];
    return [
      composeActiveTabId,
      composeSessionId.trim(),
      String(composeMessages.length),
      last?.at ?? '',
      String(composeInjectedNodes.length),
    ].join('|');
  }

  function createEmptyComposeProviderUsage(): ComposeProviderUsage {
    return {
      promptTokens: 0,
      completionTokens: 0,
      totalTokens: 0,
      responseCount: 0,
      provider: '',
      model: '',
      hasUsageData: false,
    };
  }

  function normalizeTokenUsageValue(value: number | undefined): number | null {
    if (typeof value !== 'number' || !Number.isFinite(value) || value < 0) {
      return null;
    }

    return Math.round(value);
  }

  function applyComposeProviderUsage(response: ComposeChatResponse) {
    const provider = response.provider?.trim() ?? '';
    const model = response.model?.trim() ?? '';
    const promptTokens = normalizeTokenUsageValue(response.usage?.promptTokens);
    const completionTokens = normalizeTokenUsageValue(response.usage?.completionTokens);
    const explicitTotalTokens = normalizeTokenUsageValue(response.usage?.totalTokens);

    const hasUsageDelta = promptTokens !== null || completionTokens !== null || explicitTotalTokens !== null;
    const totalTokens = explicitTotalTokens ?? ((promptTokens ?? 0) + (completionTokens ?? 0));

    if (!hasUsageDelta && !provider && !model) {
      return;
    }

    composeProviderUsage = {
      promptTokens: composeProviderUsage.promptTokens + (hasUsageDelta ? (promptTokens ?? 0) : 0),
      completionTokens: composeProviderUsage.completionTokens + (hasUsageDelta ? (completionTokens ?? 0) : 0),
      totalTokens: composeProviderUsage.totalTokens + (hasUsageDelta ? totalTokens : 0),
      responseCount: composeProviderUsage.responseCount + (hasUsageDelta ? 1 : 0),
      provider: provider || composeProviderUsage.provider,
      model: model || composeProviderUsage.model,
      hasUsageData: composeProviderUsage.hasUsageData || hasUsageDelta,
    };
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

  function extractSessionIdFromPastedNode(rawNode: string): string | null {
    const source = rawNode.trim();
    if (!source) {
      return null;
    }

    const match = source.match(/(?:session_id|sessionId)\s*:\s*(?:"([^"]+)"|'([^']+)'|([^,\n}\s]+))/i);
    const parsed = (match?.[1] ?? match?.[2] ?? match?.[3] ?? '').trim();
    return parsed || null;
  }

  $: {
    const parsedSessionId = extractSessionIdFromPastedNode(composePasteNodeDraft);
    if (parsedSessionId && composeSessionId.trim() !== parsedSessionId) {
      composeSessionId = parsedSessionId;
      if (composeError && /session id is required/i.test(composeError)) {
        composeError = null;
      }
      ensureComposeContextSession(parsedSessionId);
      snapshotActiveComposeTab();
    }
  }

  async function sendComposeMessage(
    seedText?: string,
    options: {
      includeInjectedSystemContext?: boolean;
      crossSessionSourceSessionId?: string;
    } = {},
  ) {
    const text = (seedText ?? composeDraft).trim();
    if (!text || composeReplyLoading || composeLoading) {
      if (seedText?.trim() && composeReplyLoading) {
        composeError = 'wait for the current response before injecting another message';
      }
      return;
    }

    const sessionId = composeSessionId.trim();
    if (!sessionId) {
      composeError = 'session id is required';
      return;
    }

    composeError = null;
    composeResult = null;
    composeContextNodesError = null;
    ensureComposeContextSession(sessionId);

    const crossSessionSourceSessionId = options.crossSessionSourceSessionId?.trim() ?? '';
    const userMessage: ComposeMessage = {
      role: 'user',
      content: text,
      at: new Date().toISOString(),
    };
    if (crossSessionSourceSessionId && crossSessionSourceSessionId !== sessionId) {
      userMessage.crossSession = {
        sourceSessionId: crossSessionSourceSessionId,
        targetSessionId: sessionId,
      };
    }

    const nextMessages: ComposeMessage[] = [
      ...composeMessages,
      userMessage,
    ];

    composeMessages = nextMessages;
    if (!seedText) {
      composeDraft = '';
    }
    composeReplyLoading = true;

    try {
      const includeInjectedSystemContext = options.includeInjectedSystemContext ?? true;
      const systemContext = includeInjectedSystemContext
        ? buildComposeInjectedContextSystemMessage()
        : null;
      const outboundMessages = composeApiMessages(nextMessages);
      if (systemContext) {
        outboundMessages.unshift(systemContext);
      }

      const reply = await runManagedAiWithTokenRetry(() =>
        resonantiaClient.chatCompose({
          sessionId,
          messages: outboundMessages,
        })
      );

      applyComposeProviderUsage(reply);
      const replyContent = (reply.content ?? '').trim();

      if (replyContent) {
        composeMessages = [
          ...nextMessages,
          {
            role: 'assistant',
            content: replyContent,
            at: new Date().toISOString(),
          },
        ];
      }
      snapshotActiveComposeTab();
    } catch (err) {
      composeError = String(err);
    } finally {
      composeReplyLoading = false;
      snapshotActiveComposeTab();
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
      snapshotActiveComposeTab();
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
      let config = await resonantiaClient.getConfig();
      const configuredGateway = (config.gatewayBaseUrl ?? '').trim();
      const usingManagedGateway = isManagedGatewayBaseUrl(configuredGateway);

      if (usingManagedGateway) {
        try {
          await refreshGatewayAuthTokenForSync();
          const refreshed = await resonantiaClient.getConfig();
          config = refreshed;
        } catch (tokenError) {
          cloudAuthError = String(tokenError);
        }

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

  async function encodeSaveAndContinueComposeThread(origin: ComposeEncodeSaveOrigin): Promise<boolean> {
    if (composeLoading || composeMessages.length === 0 || composeReplyLoading) {
      return false;
    }

    const sessionId = composeSessionId.trim();
    if (!sessionId) {
      composeError = 'session id is required';
      return false;
    }

    composeLoading = true;
    composeError = null;
    composeResult = null;
    composeEncodePromptSent = false;

    try {
      const messages = composeApiMessages(composeMessages);
      const maxEncodeAttempts = 2;
      let parserErrorHint: string | undefined;
      let previousNodeCandidate: string | undefined;
      let encodedNode = '';
      let res: StoreContextResponse | null = null;

      for (let attempt = 0; attempt < maxEncodeAttempts; attempt += 1) {
        composeEncodePromptSent = true;
        encodedNode = await runManagedAiWithTokenRetry(() =>
          resonantiaClient.encodeCompose({
            sessionId,
            messages,
            parserErrorHint,
            previousNodeCandidate,
          })
        );

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
          return false;
        }

        parserErrorHint = validationError;
        previousNodeCandidate = encodedNode;
      }

      if (!res || !res.valid) {
        composeError = res?.validationError ?? 'store rejected by local policy';
        return false;
      }

      const status = res.upsertStatus ?? (res.duplicateSkipped ? 'duplicate' : 'created');
      composeResult = {
        psi: res.psi,
        duplicateSkipped: Boolean(res.duplicateSkipped),
        status,
      };

      if (encodedNode.trim()) {
        seedComposeInjectedNode(sessionId, encodedNode);
      }

      composeMessages = [];
      composeAutoEncodeLastTriggerKey = '';
      if (origin === 'manual') {
        composeDraft = '';
      }

      snapshotActiveComposeTab();
      await loadGraph();
      return true;
    } catch (err) {
      composeError = String(err);
      return false;
    } finally {
      composeLoading = false;
      composeEncodePromptSent = false;
      snapshotActiveComposeTab();
    }
  }

  async function submitCompose() {
    await encodeSaveAndContinueComposeThread('manual');
  }

  $: {
    const autoSignature = composeAutoEncodeSignature();
    if (
      composeMode === 'live'
      && composeAutoEncodeEnabled
      && autoSignature
      && composeDraft.trim().length === 0
      && !composeReplyLoading
      && !composeLoading
      && composeTokenUsage.usagePercent >= composeAutoEncodeThresholdPercent
      && autoSignature !== composeAutoEncodeLastTriggerKey
    ) {
      composeAutoEncodeLastTriggerKey = autoSignature;
      void encodeSaveAndContinueComposeThread('auto');
    }
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
    calibSessionId = canonicalSessionId(selectedSession);
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

    if (adventureReplayFollowup === 'demo') {
      adventureReplayFollowup = null;
      void persistAdventureMemory(detail);
      setTimeout(() => {
        openWalkthrough('demo');
      }, 320);
      return;
    }

    void continueIntoWalkthroughAfterAdventure(detail);
  }

  function handleAdventureSkip() {
    adventureOpen = false;
    adventureCompleted = true;
    persistAdventureCompleted();

    if (adventureReplayFollowup === 'demo') {
      adventureReplayFollowup = null;
      openWalkthrough('demo');
      return;
    }

    // onboardingDismissed stays false → reactive fires openWalkthrough('first-run')
  }

  // ── Lifecycle ─────────────────────────────────────────────────
  onMount(() => {
    onboardingDismissed = readOnboardingDismissedState();
    adventureCompleted  = readAdventureCompleted();
    composeContinueRoutingPreference = readComposeContinueRoutingPreference();
    const composeChatSettings = readComposeChatSettings();
    composeAutoEncodeEnabled = composeChatSettings.autoEncodeEnabled;
    composeAutoEncodeThresholdPercent = composeChatSettings.autoEncodeThresholdPercent;
    void warmupComposeTokenizer().then(() => {
      composeTokenizerRevision += 1;
    }).catch(() => {
      // Keep fallback token estimation when tokenizer warmup fails.
    });
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
    class:camera-overlay-open={cameraOverlayEngaged}
  ></canvas>

  <nav class="navbar" class:faded={cameraOverlayEngaged}>
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
    on:continueInApp={continueThreadInCompose}
  />

  <ComposeLauncher
    faded={cameraOverlayEngaged}
    hidden={composeOpen}
    menuOpen={composeModeMenuOpen}
    on:toggle={toggleComposeModeMenu}
    on:live={openComposeLive}
  />

  <AlkahestPanel
    hidden={telescopeCameraEngaged || level > 1}
    open={alkahestOpen}
    cameraEngaged={alkahestCameraEngaged}
    phase={alkahestPhase}
    loading={alkahestLoading}
    scopeScanning={alkahestScopeScanning}
    error={alkahestError}
    status={alkahestStatus}
    bind:scope={alkahestScope}
    bind:mode={alkahestMode}
    bind:sessionId={alkahestSessionId}
    bind:sessionIds={alkahestSessionIds}
    sessions={alkahestSessionOptions}
    bind:timelineDays={alkahestTimelineDays}
    timelineOptions={ALKAHEST_TIMELINE_OPTIONS}
    bind:resonanceDim={alkahestResonanceDim}
    bind:psiMin={alkahestPsiMin}
    bind:prompt={alkahestPrompt}
    bind:importNodeDraft={alkahestImportNodeDraft}
    bind:targetSessionId={alkahestTargetSessionId}
    bind:storeDistilledNode={alkahestStoreDistilledNode}
    preflightNodeCount={alkahestPreflightNodeCount}
    preflightWindowLabel={alkahestPreflightWindowLabel}
    preflightSessionCount={alkahestPreflightSessionCount}
    preflightClipped={alkahestPreflightClipped}
    preflightLastScannedAt={alkahestPreflightLastScannedAt}
    modelProvider={modelProvider}
    superNodePreview={alkahestSuperNodePreview}
    toggleOpen={toggleAlkahestOpen}
    closePanel={closeAlkahestPanel}
    cancelAndReset={resetAlkahestFlow}
    scanScope={scanAlkahestScope}
    runAlkahest={runAlkahestFlow}
    copySuperNode={copyAlkahestSuperNode}
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
    renameTelescopeSession={openRenameSessionDialog}
    telescopeSessionColor={telescopeSessionColor}
    telescopeSessionTitle={sessionTitle}
    telescopeSessionMeta={telescopeSessionMeta}
    telescopeSessionDateLabel={telescopeSessionDateLabel}
    shortLabel={shortLabel}
  />

  {#if renameSessionOpen}
    <div
      class="rename-session-overlay"
      role="presentation"
      on:pointerdown={(event) => {
        if (event.target === event.currentTarget) {
          closeRenameSessionDialog();
        }
      }}
    >
      <div class="rename-session-card" role="dialog" aria-modal="true" aria-label="rename wave">
        <h3 class="rename-session-title">rename wave</h3>
        <p class="rename-session-subtitle">persisted rename · updates local store and sync scope</p>
        <input
          class="rename-session-input"
          type="text"
          bind:value={renameSessionDraft}
          maxlength="96"
          disabled={renameSessionLoading}
          on:input={() => (renameSessionError = null)}
          on:keydown={(event) => {
            if (event.key === 'Escape') {
              event.preventDefault();
              closeRenameSessionDialog();
            }
            if (event.key === 'Enter') {
              event.preventDefault();
              void submitRenameSessionDialog();
            }
          }}
        />
        {#if renameSessionError}
          <p class="rename-session-error">{renameSessionError}</p>
        {/if}
        <div class="rename-session-actions">
          <button class="rename-session-btn ghost" type="button" on:click={closeRenameSessionDialog} disabled={renameSessionLoading}>cancel</button>
          <button class="rename-session-btn" type="button" on:click={() => void submitRenameSessionDialog()} disabled={renameSessionLoading}>
            {renameSessionLoading ? 'saving…' : 'save'}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if composeContinueChooserOpen}
    <div
      class="compose-continue-overlay"
      role="presentation"
      on:pointerdown={(event) => {
        if (event.target === event.currentTarget) {
          closeComposeContinueChooser();
        }
      }}
    >
      <div
        class="compose-continue-card"
        role="dialog"
        aria-modal="true"
        aria-label="choose continue target"
        tabindex="0"
        on:keydown={(event) => {
          if (event.key === 'Escape') {
            event.preventDefault();
            closeComposeContinueChooser();
          }
        }}
      >
        <h3 class="compose-continue-title">route this continuation</h3>
        <p class="compose-continue-subtitle">the node session differs from your active live thread</p>
        <p class="compose-continue-context">
          node session: {composeSessionLabel(composeContinueChooserSourceSessionId) || composeContinueChooserSourceSessionId}
          <br />
          active thread: {composeSessionLabel(composeContinueChooserActiveSessionId) || composeContinueChooserActiveSessionId}
        </p>

        <div class="compose-continue-options">
          <button
            class="compose-continue-option"
            type="button"
            on:click={() => chooseComposeContinueTarget('match-session')}
          >
            <span class="compose-continue-option-title">use node session thread</span>
            <span class="compose-continue-option-meta">switch to the node session and continue there</span>
          </button>

          <button
            class="compose-continue-option"
            type="button"
            on:click={() => chooseComposeContinueTarget('active-tab')}
          >
            <span class="compose-continue-option-title">insert into active chat</span>
            <span class="compose-continue-option-meta">keep your current thread and inject this node as cross-session context</span>
          </button>
        </div>

        <label class="compose-continue-remember">
          <input type="checkbox" bind:checked={composeContinueChooserRemember} />
          <span>remember this routing choice until reset in compose</span>
        </label>

        <div class="compose-continue-actions">
          <button class="compose-continue-cancel" type="button" on:click={closeComposeContinueChooser}>cancel</button>
        </div>
      </div>
    </div>
  {/if}

  <ComposeDrawer
    {...composeLiveUiProps}
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
    onClose={closeComposeDrawer}
    onSessionInput={handleComposeSessionInput}
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
    bind:modelProvider
    bind:ollamaBaseUrl
    bind:ollamaModel
    bind:openaiBaseUrl
    bind:openaiModel
    bind:gatewayBaseUrl
    bind:gatewayAuthToken
    bind:openaiByoKeyInput
    openaiByoKeyConfigured={openaiByoKeyConfigured}
    openaiByoKeySource={openaiByoKeySource}
    openaiByoKeyBusy={openaiByoKeyBusy}
    openaiByoKeyError={openaiByoKeyError}
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
    on:saveOpenAiKey={saveOpenAiByoKey}
    on:clearOpenAiKey={clearOpenAiByoKey}
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
  canvas.camera-overlay-open { pointer-events: none; }

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

  .rename-session-overlay {
    position: absolute;
    inset: 0;
    z-index: 24;
    background: radial-gradient(circle at 52% 44%, rgba(20, 26, 42, 0.26), rgba(5, 7, 11, 0.72));
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 18px;
  }

  .rename-session-card {
    width: min(420px, calc(100vw - 36px));
    border-radius: 14px;
    padding: 18px;
    border: 0.5px solid rgba(210, 222, 255, 0.16);
    background: linear-gradient(170deg, rgba(13, 18, 30, 0.94), rgba(9, 12, 19, 0.95));
    box-shadow: 0 20px 42px rgba(2, 4, 9, 0.5), 0 0 18px rgba(118, 156, 238, 0.18);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
  }

  .rename-session-title {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 18px;
    letter-spacing: 0.03em;
    color: rgba(236, 241, 255, 0.9);
  }

  .rename-session-subtitle {
    margin: 8px 0 12px;
    font-size: 10px;
    letter-spacing: 0.06em;
    color: rgba(180, 194, 226, 0.62);
    text-transform: lowercase;
  }

  .rename-session-input {
    width: 100%;
    box-sizing: border-box;
    border-radius: 9px;
    border: 0.5px solid rgba(167, 186, 228, 0.22);
    background: rgba(11, 14, 22, 0.86);
    color: rgba(233, 239, 255, 0.92);
    font-family: 'Departure Mono', 'Courier New', monospace;
    font-size: 12px;
    padding: 9px 12px;
    outline: none;
    transition: border-color 0.16s ease, box-shadow 0.16s ease;
  }

  .rename-session-input:focus {
    border-color: rgba(186, 210, 255, 0.58);
    box-shadow: 0 0 0 1px rgba(157, 193, 255, 0.22);
  }

  .rename-session-error {
    margin: 8px 0 0;
    color: rgba(255, 152, 152, 0.82);
    font-size: 10px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
  }

  .rename-session-actions {
    margin-top: 13px;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .rename-session-btn {
    border-radius: 999px;
    border: 0.5px solid rgba(173, 197, 255, 0.36);
    background: rgba(53, 71, 108, 0.42);
    color: rgba(223, 233, 255, 0.92);
    font-family: 'Departure Mono', 'Courier New', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 6px 14px;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }

  .rename-session-btn:hover {
    border-color: rgba(210, 224, 255, 0.68);
    background: rgba(78, 101, 147, 0.5);
  }

  .rename-session-btn.ghost {
    border-color: rgba(132, 147, 187, 0.26);
    background: rgba(43, 52, 75, 0.3);
    color: rgba(192, 204, 231, 0.78);
  }

  .compose-continue-overlay {
    position: absolute;
    inset: 0;
    z-index: 28;
    background: radial-gradient(circle at 48% 36%, rgba(26, 36, 52, 0.28), rgba(4, 6, 11, 0.76));
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 18px;
  }

  .compose-continue-card {
    width: min(460px, calc(100vw - 34px));
    border-radius: 14px;
    padding: 18px;
    border: 0.5px solid rgba(204, 226, 255, 0.18);
    background: linear-gradient(170deg, rgba(10, 16, 28, 0.95), rgba(8, 12, 21, 0.96));
    box-shadow: 0 20px 44px rgba(2, 4, 10, 0.54), 0 0 20px rgba(103, 154, 226, 0.2);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    outline: none;
  }

  .compose-continue-title {
    margin: 0;
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 18px;
    letter-spacing: 0.03em;
    color: rgba(236, 242, 255, 0.92);
  }

  .compose-continue-subtitle {
    margin: 8px 0 10px;
    font-size: 10px;
    letter-spacing: 0.06em;
    color: rgba(181, 198, 228, 0.64);
    text-transform: lowercase;
  }

  .compose-continue-context {
    margin: 0;
    padding: 8px 10px;
    border-radius: 9px;
    border: 0.5px solid rgba(170, 198, 240, 0.22);
    background: rgba(11, 16, 27, 0.72);
    font-size: 10px;
    line-height: 1.6;
    letter-spacing: 0.03em;
    color: rgba(218, 230, 255, 0.8);
  }

  .compose-continue-options {
    margin-top: 12px;
    display: grid;
    gap: 8px;
  }

  .compose-continue-option {
    width: 100%;
    border-radius: 10px;
    border: 0.5px solid rgba(164, 188, 238, 0.3);
    background: rgba(20, 31, 54, 0.56);
    color: rgba(228, 237, 255, 0.9);
    padding: 9px 10px;
    text-align: left;
    display: grid;
    gap: 4px;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }

  .compose-continue-option:hover {
    border-color: rgba(205, 223, 255, 0.66);
    background: rgba(44, 62, 100, 0.6);
  }

  .compose-continue-option-title {
    font-size: 11px;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .compose-continue-option-meta {
    font-size: 10px;
    letter-spacing: 0.03em;
    color: rgba(195, 212, 240, 0.8);
    text-transform: lowercase;
  }

  .compose-continue-remember {
    margin-top: 10px;
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 10px;
    letter-spacing: 0.03em;
    color: rgba(198, 214, 242, 0.82);
    text-transform: lowercase;
    cursor: pointer;
  }

  .compose-continue-remember input {
    margin: 0;
    width: 12px;
    height: 12px;
    accent-color: rgba(126, 172, 242, 0.9);
  }

  .compose-continue-actions {
    margin-top: 12px;
    display: flex;
    justify-content: flex-end;
  }

  .compose-continue-cancel {
    border-radius: 999px;
    border: 0.5px solid rgba(130, 153, 196, 0.3);
    background: rgba(36, 44, 66, 0.4);
    color: rgba(201, 213, 235, 0.84);
    font-family: 'Departure Mono', 'Courier New', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 6px 14px;
    cursor: pointer;
    transition: border-color 0.14s ease, background 0.14s ease;
  }

  .compose-continue-cancel:hover {
    border-color: rgba(178, 202, 246, 0.56);
    background: rgba(62, 77, 112, 0.48);
  }

  @media (hover: none) and (pointer: coarse) {
    .rename-session-input {
      font-size: 16px;
      line-height: 1.35;
    }

    .compose-continue-option {
      padding: 11px 12px;
    }
  }
</style>
