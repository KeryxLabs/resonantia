<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { avecColor, avecToRgb, shortLabel, AVEC_HEX, AVEC_COLORS, formatTimestamp } from '@resonantia/core';
  import CollapseCard from '@resonantia/ui/components/CollapseCard.svelte';
  import { resonantiaClient } from './resonantiaClient';
  import type {
    AiSummary,
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
  let raf: number;
  let container: HTMLDivElement;

  let viewportWidth = 800;
  let viewportHeight = 600;
  let deviceScale = 1;

  function W() { return viewportWidth; }
  function H() { return viewportHeight; }

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

  // ── Coordinate helpers ──────────────────────────────────────────
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
      const sourceSession = graphData.sessions.find(session => session.id === e.source);
      const targetSession = graphData.sessions.find(session => session.id === e.target);
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
    const orbit = sessionOrbitRadius(selectedSession);
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

    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(session.id));
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
    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(selectedSession!.id));

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

    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(session.id));
    const labeledMoments = [...sessionNodes]
      .sort((a, b) => b.psi - a.psi)
      .slice(0, Math.min(4, sessionNodes.length));

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

  // ── Main render loop ────────────────────────────────────────────
  function draw() {
    if (!ctx) { raf = requestAnimationFrame(draw); return; }

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

    t += 0.01;
    raf = requestAnimationFrame(draw);
  }

  // ── Resize ───────────────────────────────────────────────────
  function resize() {
    if (!canvas || !container) return;
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
    if (telescopeCameraEngaged) return;
    if (didDrag) return;

    const { x: sx, y: sy } = canvasXY(e);

    if (level === 2) { surfaceToWave(); return; }

    if (level === 1 && selectedSession) {
      const sessionNodes = graph?.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(selectedSession!.id)) ?? [];
      for (const n of sessionNodes) {
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
  let syncAdvancedOpen = false;

  async function openSettings() {
    menuOpen = false;
    settingsOpen = true;
    settingsLoading = true;
    settingsError = null;
    settingsSaved = false;

    try {
      const config = await resonantiaClient.getConfig();
      gatewayBaseUrl = config.gatewayBaseUrl ?? '';
      ollamaBaseUrl = config.ollamaBaseUrl;
      ollamaModel = config.ollamaModel;
      syncAdvancedOpen = false;
    } catch (err) {
      settingsError = String(err);
    } finally {
      settingsLoading = false;
    }
  }

  async function saveSettings() {
    settingsSaving = true;
    settingsError = null;
    settingsSaved = false;

    try {
      await resonantiaClient.setOllamaConfig(ollamaBaseUrl.trim(), ollamaModel.trim());
      await resonantiaClient.setGatewayBaseUrl(gatewayBaseUrl.trim());

      settingsSaved = true;
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

    negativeLayerActive = !negativeLayerActive;
    targetCamScale = constellationLayerScale();
  }

  function openTelescope() {
    if (!telescopeCanAccess || telescopeCameraEngaged) {
      return;
    }

    closeTransientUi();
    syncDetailAutoOpen = false;
    syncDetailHover = false;
    beginTelescopeEnterTransition();
  }

  function closeTelescope() {
    if (!telescopeCameraEngaged) {
      return;
    }

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
    telescopeDragY = clientY;
    telescopeDragBasePct = telescopeDialPct;
  }

  function updateTelescopeDial(clientY: number) {
    if (telescopeDragY === null) {
      return;
    }

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
    composeModeMenuOpen = false;
    openCompose('live');
  }

  function openComposeImportare() {
    composeModeMenuOpen = false;
    openCompose('importare');
  }

  function switchComposeToLive() {
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
        sessionId: composeSessionId.trim() || 'resonantia-local',
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

  function composeOutcomeLabel(status: 'created' | 'updated' | 'duplicate' | 'skipped', duplicateSkipped: boolean) {
    if (duplicateSkipped || status === 'duplicate' || status === 'skipped') {
      return 'already present · duplicate skipped';
    }
    if (status === 'updated') {
      return 'updated';
    }
    return 'stored';
  }

  async function saveComposePastedNode() {
    const rawNode = composePasteNodeDraft.trim();
    if (!rawNode || composePasteNodeLoading || composeLoading || composeReplyLoading) {
      return;
    }

    composePasteNodeLoading = true;
    composeError = null;
    composeResult = null;

    try {
      const sessionId = composeSessionId.trim() || 'resonantia-local';
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
    syncPullLoading = true;
    syncPullError = null;
    syncPullResult = null;
    syncDetailAutoOpen = true;
    clearSyncDetailTimer();

    try {
      syncPullResult = await resonantiaClient.syncNow({ pageSize: 200 });
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
    if (composeMessages.length === 0 || composeReplyLoading) return;
    composeLoading = true; composeError = null; composeResult = null; composeEncodePromptSent = false;
    try {
      const sessionId = composeSessionId.trim() || 'resonantia-local';
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
    menuOpen = false;
    calibSessionId = selectedSession?.label ?? '';
    calibError = null;
    guideOpen = false;
    resetCalibrationGuide();
    calibrateOpen = true;
  }

  async function submitCalibrate() {
    calibLoading = true; calibError = null;
    try {
      await resonantiaClient.calibrateSession({
        sessionId: calibSessionId,
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

  // ── Lifecycle ─────────────────────────────────────────────────
  onMount(() => {
    ctx = canvas.getContext('2d')!;
    ctx.imageSmoothingEnabled = true;
    ctx.imageSmoothingQuality = 'high';
    draw();
    requestAnimationFrame(() => { resize(); loadGraph(); });
    checkHealth();

    window.addEventListener('mousemove', handleTelescopeDialMouseMove);
    window.addEventListener('mouseup', endTelescopeDial);
    window.addEventListener('touchmove', handleTelescopeDialTouchMove, { passive: false });
    window.addEventListener('touchend', endTelescopeDial);

    const ro = new ResizeObserver(resize);
    ro.observe(container);
    return () => {
      ro.disconnect();
      window.removeEventListener('mousemove', handleTelescopeDialMouseMove);
      window.removeEventListener('mouseup', endTelescopeDial);
      window.removeEventListener('touchmove', handleTelescopeDialTouchMove);
      window.removeEventListener('touchend', endTelescopeDial);
    };
  });

  onDestroy(() => {
    cancelAnimationFrame(raf);
    clearSyncDetailTimer();
    clearComposePromptCopiedTimer();
  });
</script>

<div class="weaver-root" bind:this={container}>
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
      <div
        class="sync-cloud-wrap"
        role="group"
        aria-label="sync status"
        on:mouseenter={openSyncDetailHover}
        on:mouseleave={closeSyncDetailHover}
      >
        <button
          class="sync-cloud-btn"
          class:syncing={syncVisualState === 'syncing'}
          class:success={syncVisualState === 'success'}
          class:error={syncVisualState === 'error'}
          on:click={runSyncPull}
          on:focus={openSyncDetailHover}
          on:blur={closeSyncDetailHover}
          disabled={syncPullLoading}
          aria-label={syncButtonAria}
        >
          <svg class="sync-cloud-icon" viewBox="0 0 24 24" aria-hidden="true">
            <path class="sync-cloud-shape" d="M7 18h9a4 4 0 0 0 .5-7.97A6 6 0 0 0 6 9.5 4.5 4.5 0 0 0 7 18Z" />
            <g class="sync-cloud-cycle">
              <path d="M9.2 12.8a2.9 2.9 0 0 1 4.3-.7" />
              <path d="M13.6 12h-2v2" />
              <path d="M14.8 15.2a2.9 2.9 0 0 1-4.3.7" />
              <path d="M10.4 16h2v-2" />
            </g>
          </svg>
        </button>
        {#if syncDetailVisible}
          <div
            class="sync-detail-popover"
            class:syncing={syncVisualState === 'syncing'}
            class:success={syncVisualState === 'success'}
            class:error={syncVisualState === 'error'}
            role="status"
            aria-live="polite"
          >
            <div class="sync-detail-head">
              <i class="sync-detail-dot" aria-hidden="true"></i>
              <span class="sync-detail-title">{syncDetailTitle}</span>
              <span class="sync-detail-time">{syncDetailTimeLabel}</span>
            </div>

            {#if syncPullResult}
              <div class="sync-detail-row">
                <span>up</span>
                <span>↑{syncPullResult.upload.uploaded} · ={syncPullResult.upload.duplicate} · ×{syncPullResult.upload.rejected}</span>
              </div>
              <div class="sync-detail-row">
                <span>down</span>
                <span>↓{syncPullResult.download.fetched} · +{syncPullResult.download.created} · ~{syncPullResult.download.updated} · ={syncPullResult.download.duplicate}</span>
              </div>
              {#if syncDetailSubtitle}
                <p class="sync-detail-note">{syncDetailSubtitle}</p>
              {/if}
            {:else if syncPullError}
              <p class="sync-detail-error">{syncDetailSubtitle}</p>
            {:else if syncVisualState === 'syncing'}
              <p class="sync-detail-note">{syncDetailSubtitle}</p>
            {/if}
          </div>
        {/if}
      </div>
      <div class="menu-wrap">
        <button
          class="nav-btn menu-btn"
          class:open={menuOpen}
          on:click={() => (menuOpen = !menuOpen)}
          aria-label="Open menu"
          aria-expanded={menuOpen}
        >
          ☰
        </button>
        {#if menuOpen}
          <div class="menu-popover" role="menu" aria-label="Weaver actions">
            <button class="menu-item" on:click={() => { menuOpen = false; loadGraph(); }}>refresh view</button>
            <button class="menu-item" on:click={openCalibrate}>check in</button>
            <button class="menu-item" on:click={openSettings}>settings</button>
          </div>
        {/if}
      </div>
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

  <div class="compose-launch" class:faded={telescopeCameraEngaged} class:hidden={composeOpen}>
    {#if composeModeMenuOpen}
      <div class="compose-launch-popover" role="menu" aria-label="compose options">
        <button class="compose-launch-item" on:click={openComposeLive}>create live</button>
        <button class="compose-launch-item" on:click={openComposeImportare}>importare</button>
      </div>
    {/if}
    <button class="compose-btn" class:open={composeModeMenuOpen} on:click={toggleComposeModeMenu}>+ compose</button>
  </div>

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
              <button
                class="telescope-item"
                on:click={() => selectTelescopeSession(session)}
                title={session.label}
              >
                <i
                  class="telescope-dot"
                  style={`background:${telescopeSessionColor(session)}; box-shadow: 0 0 7px ${telescopeSessionColor(session, 0.4)};`}
                ></i>
                <span class="telescope-copy">
                  <span class="telescope-title">{shortLabel(session.label, 4)}</span>
                  <span class="telescope-meta">{telescopeSessionMeta(session)}</span>
                </span>
                <span class="telescope-date">{telescopeSessionDateLabel(session.lastModified)}</span>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  </div>

  {#if composeOpen}
    <div class="drawer drawer-compose" class:importare={composeMode === 'importare'} role="dialog" aria-label={composeMode === 'importare' ? 'Import node' : 'Compose context'}>
      <div class="drawer-header">
        <span class="drawer-title">{composeMode === 'importare' ? 'importare' : 'compose'}</span>
        <button class="close-btn" on:click={() => (composeOpen = false)}>✕</button>
      </div>
      <input class="drawer-input" type="text" placeholder="session id" bind:value={composeSessionId} />
      {#if composeMode === 'live'}
        <div class="compose-thread" aria-live="polite">
          {#if composeMessages.length === 0}
            <p class="compose-empty">chat first, then encode the thread into one protocol node.</p>
          {:else}
            {#each composeMessages as message}
              <article class={`compose-bubble ${message.role === 'assistant' ? 'assistant' : 'user'}`}>
                <header class="compose-bubble-meta">
                  <span>{message.role === 'assistant' ? 'resonare' : 'you'}</span>
                  <small>{formatTimestamp(message.at)}</small>
                </header>
                <p>{message.content}</p>
              </article>
            {/each}
          {/if}

          {#if composeReplyLoading}
            <article class="compose-bubble assistant compose-pending">
              <header class="compose-bubble-meta">
                <span>resonare</span>
                <small>thinking…</small>
              </header>
            </article>
          {/if}
        </div>

        <div class="compose-entry">
          <textarea
            class="drawer-textarea compose-input"
            placeholder="message…"
            bind:value={composeDraft}
            rows="3"
            on:keydown={(event) => {
              if (event.key === 'Enter' && !event.shiftKey) {
                event.preventDefault();
                void sendComposeMessage();
              }
            }}
          ></textarea>
          <button
            class="drawer-btn submit compose-send"
            on:click={sendComposeMessage}
            disabled={composeLoading || composeReplyLoading || !composeDraft.trim()}
          >
            {composeReplyLoading ? 'thinking…' : 'send'}
          </button>
        </div>
      {:else}
        <p class="compose-importare-note">paste one complete node and store it directly.</p>
      {/if}

      <div class="compose-utility-actions">
        <button class="compose-link-btn" on:click={copyComposeEncodePrompt} disabled={composePromptCopyLoading || composeLoading || composeReplyLoading}>
          {composePromptCopyLoading ? 'copying distill prompt…' : composePromptCopied ? 'distill prompt copied' : 'copy distill prompt'}
        </button>
        {#if composeMode === 'live'}
          <span class="compose-utility-divider">•</span>
          <button class="compose-link-btn" on:click={toggleComposePasteNode} disabled={composePasteNodeLoading || composeLoading || composeReplyLoading}>
            {composePasteNodeOpen ? 'hide paste save' : 'paste node to save'}
          </button>
          {#if composeMessages.length > 0}
            <span class="compose-utility-divider">•</span>
            <button class="compose-link-btn" on:click={clearComposeConversation} disabled={composeLoading || composeReplyLoading}>clear thread</button>
          {/if}
        {:else}
          <span class="compose-utility-divider">•</span>
          <button class="compose-link-btn" on:click={switchComposeToLive} disabled={composePasteNodeLoading}>switch to create live</button>
        {/if}
      </div>
      {#if composePromptCopyError}
        <p class="drawer-error">copy failed: {composePromptCopyError}</p>
      {/if}
      {#if composeMode === 'importare' || composePasteNodeOpen}
        <div class="compose-paste-panel">
          <p class="compose-paste-intro">paste a complete STTP node and save it directly.</p>
          <textarea
            class="drawer-textarea compose-paste-input"
            placeholder="paste one full STTP node"
            bind:value={composePasteNodeDraft}
            rows="9"
          ></textarea>
          <div class="compose-paste-actions">
            {#if composeMode === 'live'}
              <button class="drawer-btn cancel" on:click={toggleComposePasteNode} disabled={composePasteNodeLoading}>cancel paste</button>
            {/if}
            <button class="drawer-btn submit" on:click={saveComposePastedNode} disabled={composePasteNodeLoading || !composePasteNodeDraft.trim()}>
              {composePasteNodeLoading ? 'saving…' : 'save pasted node'}
            </button>
          </div>
        </div>
      {/if}
      {#if composeMode === 'live' && composeLoading && composeEncodePromptSent}
        <p class="drawer-success compose-encode-note">encoding prompt sent</p>
      {/if}
      {#if composeError}<p class="drawer-error">{composeError}</p>{/if}
      {#if composeResult}
        <p class="drawer-success">
          {composeOutcomeLabel(composeResult.status, composeResult.duplicateSkipped)} · Ψ {composeResult.psi.toFixed(4)}
        </p>
      {/if}
      <div class="drawer-actions compose-actions">
        <button class="drawer-btn cancel" on:click={() => (composeOpen = false)}>{composeMode === 'importare' ? 'close' : 'cancel'}</button>
        {#if composeMode === 'live'}
          <button class="drawer-btn submit" on:click={submitCompose} disabled={composeLoading || composeReplyLoading || composeMessages.length === 0}>
            {composeLoading ? 'encoding…' : 'encode + store'}
          </button>
        {/if}
      </div>
    </div>
  {/if}

  {#if calibrateOpen}
    <div class="drawer" role="dialog" aria-label="Find your current mode">
      <div class="drawer-header">
        <span class="drawer-title">find your current mode</span>
        <button class="close-btn" on:click={() => (calibrateOpen = false)}>✕</button>
      </div>
      <input class="drawer-input" type="text" placeholder="session name or id" bind:value={calibSessionId} />
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
        {#each CALIBRATION_PROFILES as profile}
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
          {#each CALIBRATION_QUESTIONS as question, questionIndex}
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
        <input type="range" min="0" max="1" step="0.01" bind:value={calibStability} class="avec-slider" style="accent-color: {AVEC_HEX.stability};" />
        <span class="slider-val" style="color:{AVEC_HEX.stability}">{calibStability.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.friction}">wear</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibFriction} class="avec-slider" style="accent-color: {AVEC_HEX.friction};" />
        <span class="slider-val" style="color:{AVEC_HEX.friction}">{calibFriction.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.logic}">clarity</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibLogic} class="avec-slider" style="accent-color: {AVEC_HEX.logic};" />
        <span class="slider-val" style="color:{AVEC_HEX.logic}">{calibLogic.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.autonomy}">self-trust</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibAutonomy} class="avec-slider" style="accent-color: {AVEC_HEX.autonomy};" />
        <span class="slider-val" style="color:{AVEC_HEX.autonomy}">{calibAutonomy.toFixed(2)}</span>
      </div>
      <p class="calibration-source">saved from {calibTrigger.replaceAll('_', ' ')}</p>
      {#if calibError}<p class="drawer-error">{calibError}</p>{/if}
      <div class="drawer-actions">
        <button class="drawer-btn cancel" on:click={() => (calibrateOpen = false)}>cancel</button>
        <button class="drawer-btn submit" on:click={submitCalibrate} disabled={calibLoading}>
          {calibLoading ? 'saving…' : 'save mode'}
        </button>
      </div>
    </div>
  {/if}

  {#if settingsOpen}
    <div class="drawer" role="dialog" aria-label="Settings">
      <div class="drawer-header">
        <span class="drawer-title">settings</span>
        <button class="close-btn" on:click={() => (settingsOpen = false)}>✕</button>
      </div>
      <p class="settings-intro">Resonantia runs local-first. Model settings live here, and cloud sync can be linked once in advanced settings.</p>

      <label class="settings-field">
        <span class="settings-label">ollama base url</span>
        <span class="settings-note">Local model endpoint for transmutation and summaries</span>
        <input class="drawer-input" type="text" placeholder="http://127.0.0.1:11434" bind:value={ollamaBaseUrl} disabled={settingsLoading || settingsSaving} />
      </label>

      <label class="settings-field">
        <span class="settings-label">ollama model</span>
        <span class="settings-note">Model name Resonantia should call by default</span>
        <input class="drawer-input" type="text" placeholder="llama3.2" bind:value={ollamaModel} disabled={settingsLoading || settingsSaving} />
      </label>

      <button
        class="settings-advanced-toggle"
        on:click={() => (syncAdvancedOpen = !syncAdvancedOpen)}
        disabled={settingsLoading || settingsSaving}
      >
        {syncAdvancedOpen ? 'hide advanced sync' : 'advanced sync'}
      </button>

      {#if syncAdvancedOpen}
        <div class="settings-advanced-panel">
          <label class="settings-field">
            <span class="settings-label">cloud sync path</span>
            <span class="settings-note">Set this once, then just use Sync from the menu.</span>
            <input
              class="drawer-input"
              type="text"
              placeholder="https://your-sync-endpoint"
              bind:value={gatewayBaseUrl}
              disabled={settingsLoading || settingsSaving}
            />
          </label>
        </div>
      {/if}

      {#if settingsLoading}<p class="drawer-success">loading config…</p>{/if}
      {#if settingsError}<p class="drawer-error">{settingsError}</p>{/if}
      {#if settingsSaved}<p class="drawer-success">settings saved</p>{/if}
      <div class="drawer-actions">
        <button class="drawer-btn cancel" on:click={() => (settingsOpen = false)}>cancel</button>
        <button class="drawer-btn submit" on:click={saveSettings} disabled={settingsLoading || settingsSaving}>
          {settingsSaving ? 'saving…' : 'save'}
        </button>
      </div>
    </div>
  {/if}
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

  .sync-cloud-wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  .sync-cloud-btn {
    width: 24px;
    height: 19px;
    border-radius: 999px;
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.02);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    cursor: pointer;
    transition: border-color 0.2s, background 0.2s, transform 0.2s, box-shadow 0.2s;
  }

  .sync-cloud-btn:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.2);
    background: rgba(255, 255, 255, 0.038);
    transform: translateY(-0.5px);
  }

  .sync-cloud-btn:disabled {
    cursor: default;
  }

  .sync-cloud-btn.syncing {
    border-color: rgba(147, 196, 255, 0.34);
    background: rgba(94, 148, 216, 0.11);
    box-shadow: 0 0 10px rgba(90, 150, 221, 0.18);
  }

  .sync-cloud-btn.success {
    border-color: rgba(147, 230, 187, 0.3);
    background: rgba(82, 171, 125, 0.11);
  }

  .sync-cloud-btn.error {
    border-color: rgba(255, 128, 125, 0.34);
    background: rgba(190, 72, 69, 0.11);
  }

  .sync-cloud-icon {
    width: 12.5px;
    height: 12.5px;
  }

  .sync-cloud-icon path {
    fill: none;
    stroke: rgba(255, 255, 255, 0.62);
    stroke-width: 1.42;
    stroke-linecap: round;
    stroke-linejoin: round;
    transition: stroke 0.2s;
  }

  .sync-cloud-btn.syncing .sync-cloud-icon path {
    stroke: rgba(191, 222, 255, 0.96);
  }

  .sync-cloud-btn.success .sync-cloud-icon path {
    stroke: rgba(194, 245, 217, 0.98);
  }

  .sync-cloud-btn.error .sync-cloud-icon path {
    stroke: rgba(255, 210, 208, 0.97);
  }

  .sync-cloud-cycle {
    transform-origin: center;
    transform-box: fill-box;
  }

  .sync-cloud-btn.syncing .sync-cloud-cycle {
    animation: sync-cloud-spin 0.9s linear infinite;
  }

  .sync-detail-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: -2px;
    width: max-content;
    min-width: 146px;
    max-width: min(216px, 68vw);
    padding: 7px 8px;
    border-radius: 10px;
    border: 0.5px solid rgba(255, 255, 255, 0.11);
    background:
      radial-gradient(circle at 88% 4%, rgba(138, 174, 228, 0.13), transparent 40%),
      radial-gradient(circle at 8% 92%, rgba(112, 192, 153, 0.09), transparent 44%),
      rgba(10, 11, 14, 0.915);
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.3);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    z-index: 28;
  }

  .sync-detail-popover::before {
    content: '';
    position: absolute;
    top: -4px;
    right: 12px;
    width: 8px;
    height: 8px;
    border-left: 0.5px solid rgba(255, 255, 255, 0.11);
    border-top: 0.5px solid rgba(255, 255, 255, 0.11);
    background: rgba(10, 11, 14, 0.915);
    transform: rotate(45deg);
  }

  .sync-detail-popover.syncing {
    border-color: rgba(147, 196, 255, 0.4);
  }

  .sync-detail-popover.success {
    border-color: rgba(147, 230, 187, 0.34);
  }

  .sync-detail-popover.error {
    border-color: rgba(255, 130, 126, 0.4);
  }

  .sync-detail-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 6px;
    margin-bottom: 4px;
  }

  .sync-detail-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: rgba(195, 205, 223, 0.7);
    box-shadow: 0 0 10px rgba(133, 163, 214, 0.35);
  }

  .sync-detail-popover.syncing .sync-detail-dot {
    background: rgba(186, 222, 255, 0.95);
    box-shadow: 0 0 12px rgba(117, 171, 237, 0.46);
  }

  .sync-detail-popover.success .sync-detail-dot {
    background: rgba(191, 245, 216, 0.96);
    box-shadow: 0 0 10px rgba(89, 176, 127, 0.42);
  }

  .sync-detail-popover.error .sync-detail-dot {
    background: rgba(255, 201, 198, 0.94);
    box-shadow: 0 0 10px rgba(202, 90, 82, 0.42);
  }

  .sync-detail-title {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.74);
  }

  .sync-detail-time {
    font-family: 'Departure Mono', monospace;
    font-size: 7px;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.42);
  }

  .sync-detail-row {
    display: grid;
    grid-template-columns: 34px 1fr;
    gap: 6px;
    margin-bottom: 3px;
  }

  .sync-detail-row span:first-child {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.4);
  }

  .sync-detail-row span:last-child {
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.71);
    text-align: right;
    white-space: nowrap;
  }

  .sync-detail-note,
  .sync-detail-error {
    margin: 6px 0 0;
    font-family: 'Departure Mono', monospace;
    font-size: 8px;
    letter-spacing: 0.05em;
    line-height: 1.45;
  }

  .sync-detail-note {
    color: rgba(255, 255, 255, 0.54);
  }

  .sync-detail-error {
    color: rgba(255, 200, 197, 0.92);
  }

  @keyframes sync-cloud-spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  .nav-btn {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    color: rgba(255, 255, 255, 0.3);
    background: transparent;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    border-radius: 4px;
    padding: 4px 10px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s;
  }
  .nav-btn:hover {
    color: rgba(255, 255, 255, 0.75);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .menu-wrap {
    position: relative;
  }

  .menu-btn {
    min-width: 36px;
    padding: 4px 0;
    text-align: center;
    font-size: 13px;
    line-height: 1;
  }

  .menu-btn.open {
    color: rgba(255, 255, 255, 0.72);
    border-color: rgba(255, 255, 255, 0.22);
  }

  .menu-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    width: 148px;
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    background: rgba(10, 11, 14, 0.97);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 10px;
    backdrop-filter: blur(18px);
    -webkit-backdrop-filter: blur(18px);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.28);
  }

  .menu-item {
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-align: left;
    color: rgba(255, 255, 255, 0.58);
    background: transparent;
    border: 0.5px solid transparent;
    border-radius: 6px;
    padding: 8px 10px;
    cursor: pointer;
    transition: color 0.2s, border-color 0.2s, background 0.2s;
  }

  .menu-item:hover {
    color: rgba(255, 255, 255, 0.84);
    border-color: rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.03);
  }

  .menu-item:disabled {
    cursor: default;
    opacity: 0.5;
  }


  .compose-launch {
    position: absolute;
    bottom: max(24px, calc(var(--safe-bottom) + 12px));
    right: max(24px, calc(var(--safe-right) + 12px));
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

  .telescope-shell {
    position: absolute;
    left: max(20px, calc(var(--safe-left) + 12px));
    bottom: max(20px, calc(var(--safe-bottom) + 12px));
    z-index: 16;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    pointer-events: none;
  }

  .negative-layer-btn {
    width: 30px;
    height: 30px;
    border-radius: 999px;
    background: rgba(12, 15, 22, 0.9);
    border: 0.5px solid rgba(132, 172, 226, 0.32);
    display: flex;
    align-items: center;
    justify-content: center;
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
    width: 36px;
    height: 36px;
    border-radius: 999px;
    background: rgba(14, 10, 30, 0.9);
    border: 0.5px solid rgba(160, 140, 255, 0.35);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    pointer-events: all;
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
    grid-template-columns: auto minmax(0, 1fr) auto;
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

  .telescope-empty {
    margin: auto 0;
    padding-left: 17px;
    font-size: 10px;
    letter-spacing: 0.06em;
    color: rgba(170, 156, 224, 0.44);
    text-transform: lowercase;
  }

  .drawer {
    position: absolute;
    top: 64px;
    bottom: 84px;
    left: 50%;
    transform: translateX(-50%);
    width: min(456px, calc(100vw - 32px));
    max-height: calc(100dvh - 148px);
    overflow-y: auto;
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

  .drawer-compose {
    top: max(64px, calc(var(--safe-top) + 46px));
    bottom: auto;
    max-height: min(680px, calc(100dvh - 150px));
  }

  .drawer-compose.importare {
    max-height: min(620px, calc(100dvh - 160px));
  }

  .compose-thread {
    min-height: 144px;
    max-height: 290px;
    overflow-y: auto;
    border: 0.5px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.018);
    padding: 9px;
    margin-bottom: 10px;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .compose-empty {
    margin: auto 0;
    text-align: center;
    font-size: 10px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.34);
    letter-spacing: 0.03em;
  }

  .compose-bubble {
    border: 0.5px solid rgba(255, 255, 255, 0.065);
    border-radius: 9px;
    padding: 6px 8px;
    background: rgba(255, 255, 255, 0.03);
  }

  .compose-bubble.user {
    border-color: rgba(255, 255, 255, 0.18);
    background: rgba(255, 255, 255, 0.06);
  }

  .compose-bubble.assistant {
    border-color: rgba(214, 184, 109, 0.26);
    background: rgba(214, 184, 109, 0.08);
  }

  .compose-bubble-meta {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 5px;
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.5);
  }

  .compose-bubble-meta small {
    font-size: 8px;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.35);
  }

  .compose-bubble p {
    margin: 0;
    font-size: 10px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.76);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .compose-pending {
    animation: composePulse 1.4s ease-in-out infinite;
  }

  .compose-entry {
    display: flex;
    gap: 6px;
    align-items: flex-end;
  }

  .compose-input {
    margin-bottom: 0;
    min-height: 84px;
  }

  .compose-send {
    min-width: 88px;
    margin-bottom: 1px;
  }

  .compose-importare-note {
    margin: 0 0 8px;
    font-size: 10px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.44);
    text-transform: lowercase;
  }

  .compose-utility-actions {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 7px;
    margin-bottom: 2px;
  }

  .compose-utility-divider {
    color: rgba(255, 255, 255, 0.18);
    font-size: 9px;
    line-height: 1;
    user-select: none;
  }

  .compose-link-btn {
    border: none;
    background: transparent;
    padding: 0;
    margin: 0;
    font-family: 'Departure Mono', monospace;
    font-size: 9px;
    letter-spacing: 0.04em;
    text-transform: lowercase;
    color: rgba(255, 255, 255, 0.46);
    cursor: pointer;
    transition: color 0.2s;
  }

  .compose-link-btn:hover:not(:disabled) {
    color: rgba(255, 255, 255, 0.76);
  }

  .compose-link-btn:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .compose-paste-panel {
    margin-top: 7px;
    padding: 10px;
    border-radius: 10px;
    border: 0.5px dashed rgba(255, 255, 255, 0.11);
    background: rgba(255, 255, 255, 0.012);
  }

  .compose-paste-intro {
    margin: 0 0 8px;
    font-size: 9px;
    line-height: 1.45;
    letter-spacing: 0.04em;
    color: rgba(255, 255, 255, 0.48);
    text-transform: lowercase;
  }

  .compose-paste-input {
    min-height: 168px;
    margin-bottom: 0;
  }

  .drawer-compose.importare .compose-paste-input {
    min-height: 224px;
  }

  .compose-paste-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 8px;
  }

  .compose-actions {
    justify-content: flex-end;
    align-items: center;
  }

  .compose-encode-note {
    margin-top: 8px;
    opacity: 0.85;
    letter-spacing: 0.04em;
    text-transform: lowercase;
  }

  @keyframes composePulse {
    0%,
    100% { opacity: 0.64; }
    50% { opacity: 1; }
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
  .close-btn:hover { color: rgba(255, 255, 255, 0.7); }

  .drawer-input {
    width: 100%; box-sizing: border-box;
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
  .drawer-input:focus { border-color: rgba(255, 255, 255, 0.25); }

  .drawer-textarea {
    width: 100%; box-sizing: border-box;
    background: rgba(255, 255, 255, 0.04);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 8px 10px;
    color: rgba(255, 255, 255, 0.7);
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    resize: vertical;
    margin-bottom: 10px;
    outline: none;
    transition: border-color 0.2s;
  }
  .drawer-textarea:focus { border-color: rgba(255, 255, 255, 0.25); }

  .settings-intro {
    margin: 0 0 14px;
    font-size: 10px;
    line-height: 1.55;
    color: rgba(255, 255, 255, 0.48);
  }

  .settings-field {
    display: flex;
    flex-direction: column;
    gap: 5px;
    margin-bottom: 12px;
  }

  .settings-label {
    font-size: 9px;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.42);
  }

  .settings-note {
    font-size: 10px;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.3);
  }

  .settings-field .drawer-input {
    margin-bottom: 0;
  }

  .settings-advanced-toggle {
    width: 100%;
    margin: 4px 0 10px;
    background: rgba(255, 255, 255, 0.02);
    border: 0.5px dashed rgba(255, 255, 255, 0.16);
    color: rgba(255, 255, 255, 0.54);
    border-radius: 8px;
    padding: 8px 10px;
    font-family: 'Departure Mono', monospace;
    font-size: 10px;
    letter-spacing: 0.08em;
    text-transform: lowercase;
    text-align: left;
    cursor: pointer;
    transition: border-color 0.2s, color 0.2s, background 0.2s;
  }

  .settings-advanced-toggle:hover:not(:disabled) {
    border-color: rgba(255, 255, 255, 0.26);
    color: rgba(255, 255, 255, 0.78);
    background: rgba(255, 255, 255, 0.04);
  }

  .settings-advanced-toggle:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .settings-advanced-panel {
    margin-bottom: 10px;
    padding: 11px;
    border-radius: 9px;
    border: 0.5px solid rgba(255, 255, 255, 0.08);
    background: rgba(255, 255, 255, 0.02);
  }

  .slider-row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .slider-label { font-size: 9px; letter-spacing: 0.1em; text-transform: uppercase; width: 72px; flex-shrink: 0; }
  .avec-slider  { flex: 1; accent-color: rgba(255,255,255,0.4); height: 2px; }
  .slider-val   { font-size: 10px; color: rgba(255,255,255,0.4); width: 32px; text-align: right; flex-shrink: 0; }

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

  @media (max-width: 520px) {
    .telescope-shell {
      left: max(12px, calc(var(--safe-left) + 10px));
      bottom: max(12px, calc(var(--safe-bottom) + 10px));
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

    .drawer {
      top: calc(var(--safe-top) + 56px);
      bottom: max(74px, calc(var(--safe-bottom) + 58px));
      width: calc(100vw - 20px);
      max-height: calc(100dvh - 130px);
      padding: 16px;
    }

    .drawer-compose {
      top: calc(var(--safe-top) + 56px);
      bottom: auto;
      max-height: min(74dvh, 560px);
      padding: 14px;
    }

    .compose-thread {
      max-height: 236px;
      min-height: 132px;
    }

    .compose-entry {
      flex-direction: column;
      align-items: stretch;
      gap: 6px;
    }

    .compose-send {
      width: 100%;
      min-width: 0;
    }

    .compose-input {
      min-height: 72px;
    }

    .compose-utility-actions,
    .compose-paste-actions {
      flex-direction: column;
      align-items: stretch;
    }

    .compose-link-btn,
    .compose-paste-actions .drawer-btn {
      width: 100%;
      text-align: left;
      padding: 1px 0;
    }

    .compose-utility-divider {
      display: none;
    }

    .profile-grid,
    .guide-options {
      grid-template-columns: 1fr;
    }

  }

  .drawer-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }

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
  .drawer-btn:disabled { opacity: 0.4; cursor: not-allowed; }

  .drawer-error   { font-size: 10px; color: rgba(233,148,58,0.8);  margin: 6px 0 0; }
  .drawer-success { font-size: 10px; color: rgba(122,170,122,0.9); margin: 6px 0 0; }
</style>
