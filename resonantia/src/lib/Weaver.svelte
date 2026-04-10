<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { avecColor, avecToRgb, shortLabel, AVEC_HEX, formatTimestamp } from './avec';
  import CollapseCard from './CollapseCard.svelte';
  import type {
    GraphResponse,
    GraphSessionDto,
    GraphNodeDto,
    NodeDto,
    CollapseCardData,
    Vec2,
  } from './types';

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

  const sessionPos: Record<string, Vec2> = {};
  const nodePos:    Record<string, Vec2> = {};

  // ── Camera ──────────────────────────────────────────────
  // Camera: the world point at screen-center, and the current scale
  let camX = 0, camY = 0, camScale = 1;
  let targetCamX = 0, targetCamY = 0, targetCamScale = 1;

  const CONSTELLATION_SCALE = 1.2;
  const WAVE_SCALE     = 4.6;
  const COLLAPSE_SCALE = 10.2;
  const LERP           = 0.09;

  // ── Level state machine ────────────────────────────────────────
  // 0 = constellation  1 = wave (session)  2 = collapse (node/moment)
  let level = 0;
  let selectedSession: GraphSessionDto | null = null;
  let selectedNode:    GraphNodeDto    | null = null;
  let cardData:    CollapseCardData | null = null;
  let cardVisible  = false;

  function matchesSelectedNode(graphNode: GraphNodeDto, dto: NodeDto) {
    return dto.syntheticId === graphNode.syntheticId;
  }

  // ── Interaction ───────────────────────────────────────────
  let dragging    = false;
  let didDrag     = false;
  let dragStart:   Vec2 = { x: 0, y: 0 };
  let panCamStart: Vec2 = { x: 0, y: 0 };

  let t = 0;
  const STATIC_STARS = Array.from({ length: 80 }, () => ({
    x:     Math.random(),
    y:     Math.random(),
    s:     0.3 + Math.random() * 0.9,
    phase: Math.random() * Math.PI * 2,
  }));

  // ── Coordinate helpers ──────────────────────────────────────────
  function toScreen(wx: number, wy: number): Vec2 {
    return {
      x: (wx - camX) * camScale + W() / 2,
      y: (wy - camY) * camScale + H() / 2,
    };
  }

  function toWorld(sx: number, sy: number): Vec2 {
    return {
      x: (sx - W() / 2) / camScale + camX,
      y: (sy - H() / 2) / camScale + camY,
    };
  }

  function canvasXY(e: MouseEvent): Vec2 {
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
      graph   = await invoke<GraphResponse>('get_graph', { limit: 200, sessionId: null });
      layoutConstellation();
      camX = targetCamX = W() / 2;
      camY = targetCamY = H() / 2;
      camScale = targetCamScale = CONSTELLATION_SCALE;
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function layoutConstellation() {
    if (!graph) return;
    Object.keys(sessionPos).forEach(key => delete sessionPos[key]);
    Object.keys(nodePos).forEach(key => delete nodePos[key]);
    const cx = W() / 2, cy = H() / 2;
    const n  = graph.sessions.length;
    const spread = Math.min(W(), H()) * 0.3;
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));

    graph.sessions.forEach((s, i) => {
      const seed = hashUnit(s.id);
      const angle = i * goldenAngle + seed * 1.8;
      const radial = spread * (0.24 + Math.sqrt((i + 0.5) / Math.max(1, n)) * 0.74);
      const bendX = Math.sin(angle * 1.4 + seed * 8) * spread * 0.16;
      const bendY = Math.cos(angle * 1.1 + seed * 6) * spread * 0.12;
      sessionPos[s.id] = {
        x: cx + Math.cos(angle) * radial + bendX,
        y: cy + Math.sin(angle) * radial * 0.76 + bendY,
      };
    });

      graph.sessions.forEach((session, sessionIndex) => {
        const sp = sessionPos[sessionKey(session.id)];
      if (!sp) return;

        const sessionNodes = graph!.nodes.filter(node => sessionKey(node.sessionId) === sessionKey(session.id));
      const orbitRadius = Math.max(54, sessionRadius(session) + 20);
      const seed = sessionIndex * 0.73 + session.id.length * 0.11;

      sessionNodes.forEach((node, nodeIndex) => {
        const progress = (nodeIndex + 1) / (sessionNodes.length + 1);
        const radial = orbitRadius * (0.24 + Math.sqrt(progress) * 0.78);
        const angle = seed + nodeIndex * 2.399963229728653;
        nodePos[node.id] = {
          x: sp.x + Math.cos(angle) * radial,
          y: sp.y + Math.sin(angle) * radial * 0.72,
        };
      });
    });
  }

  function sessionRadius(s: GraphSessionDto) { return 10 + s.nodeCount * 2.2; }
  function nodeRadius(n: GraphNodeDto)        { return 4.2 + n.psi * 1.35;    }

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
    selectedSession = s;
    selectedNode    = null;
    closeCard();
    level = 1;
    const sp = sessionRenderPos(s);
    if (sp) { targetCamX = sp.x; targetCamY = sp.y; targetCamScale = WAVE_SCALE; }
  }

  async function descendToCollapse(n: GraphNodeDto) {
    closeTransientUi();
    selectedNode = n;
    level        = 2;
    const np = nodePos[n.id];
    if (np) { targetCamX = np.x; targetCamY = np.y; targetCamScale = COLLAPSE_SCALE; }

    cardData = {
      node:            n,
      nodeDto:         null,
      relatedSessions: graph?.sessions.filter(s => s.id !== sessionKey(n.sessionId)).slice(0, 4) ?? [],
    };
    setTimeout(() => { cardVisible = true; }, 520);

    try {
      const res = await invoke<{ nodes: NodeDto[] }>('list_nodes', {
        limit: Math.max(selectedSession?.nodeCount ?? 50, 50), sessionId: n.sessionId,
      });
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
      if (sp) { targetCamX = sp.x; targetCamY = sp.y; targetCamScale = WAVE_SCALE; }
    }
  }

  function surfaceToConstellation() {
    closeTransientUi();
    selectedSession = null;
    selectedNode    = null;
    closeCard();
    level = 0;
    targetCamX = W() / 2; targetCamY = H() / 2; targetCamScale = CONSTELLATION_SCALE;
  }

  function closeCard() {
    cardVisible = false;
    setTimeout(() => { if (!cardVisible) cardData = null; }, 500);
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
    if (!graph || level > 0) return;
    const graphData = graph;
    graphData.edges.forEach(e => {
      const sourceSession = graphData.sessions.find(session => session.id === e.source);
      const targetSession = graphData.sessions.find(session => session.id === e.target);
      if (!sourceSession || !targetSession) return;
      const sp = sessionRenderPos(sourceSession);
      const tp = sessionRenderPos(targetSession);
      if (!sp || !tp) return;
      const mx = (sp.x + tp.x) / 2 + Math.sin(e.id.length * 0.7) * 30;
      const my = (sp.y + tp.y) / 2 + Math.cos(e.id.length * 0.5) * 20;
      const grad = ctx.createLinearGradient(sp.x, sp.y, tp.x, tp.y);
      grad.addColorStop(0,    'rgba(255,255,255,0)');
      grad.addColorStop(0.35, 'rgba(255,255,255,0.06)');
      grad.addColorStop(0.5,  e.kind === 'resonance' ? 'rgba(255,255,255,0.1)' : 'rgba(255,255,255,0.04)');
      grad.addColorStop(0.65, 'rgba(255,255,255,0.06)');
      grad.addColorStop(1,    'rgba(255,255,255,0)');
      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.quadraticCurveTo(mx, my, tp.x, tp.y);
      ctx.strokeStyle = grad;
      ctx.lineWidth   = (e.kind === 'resonance' ? 0.8 : 0.4) / camScale;
      ctx.setLineDash(e.kind === 'temporal' ? [3, 6] : []);
      ctx.stroke();
      ctx.setLineDash([]);
    });
  }

  function drawSessions() {
    if (!graph) return;
    const baseAvec = { stability:0.7, friction:0.2, logic:0.8, autonomy:0.85, psi:2.55 };

    graph.sessions.forEach(s => {
      const sp      = sessionRenderPos(s);
      if (!sp) return;
      const isFocus = selectedSession?.id === s.id;

      if (level > 0 && !isFocus) {
        ctx.beginPath();
        ctx.arc(sp.x, sp.y, 2, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(255,255,255,0.07)';
        ctx.fill();
        return;
      }

      const r     = sessionRadius(s);
      const pulse = isFocus && level === 1
        ? Math.sin(t * 3) * 2.5
        : Math.sin(t * 1.1 + sp.x * 0.008) * 1.2;

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, r + pulse + 12, 0, Math.PI * 2);
      ctx.fillStyle = avecColor(baseAvec, 0.03);
      ctx.fill();

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, r + pulse, 0, Math.PI * 2);
      ctx.fillStyle   = avecColor(baseAvec, isFocus ? 0.14 : 0.07);
      ctx.fill();
      ctx.strokeStyle = avecColor(baseAvec, isFocus ? 0.48 : 0.18);
      ctx.lineWidth   = (isFocus ? 0.8 : 0.5) / camScale;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(sp.x, sp.y, 2.5, 0, Math.PI * 2);
      ctx.fillStyle = avecColor(baseAvec, 0.9);
      ctx.fill();

      if (level === 0 && r > 18) {
        ctx.fillStyle = 'rgba(255,255,255,0.22)';
        ctx.font      = `10px ${FONT_MONO}`;
        ctx.textAlign = 'center';
        ctx.fillText(shortLabel(s.label, 3), sp.x, sp.y + r + 15);
      }
    });
  }

  function drawWaveBoundary() {
    if (level !== 1 || !selectedSession) return;
    const sp = sessionPos[selectedSession.id];
    if (!sp) return;
    const orbit = Math.max(54, sessionRadius(selectedSession) + 20);
    const rx  = orbit + 18;
    const ry  = (orbit + 18) * 0.74;
    const oda = 4 + Math.sin(t * 0.5);

    ctx.beginPath();
    ctx.ellipse(sp.x, sp.y, rx + 22, ry + 16, 0, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(122,170,122,0.04)';
    ctx.lineWidth   = 12 / camScale;
    ctx.setLineDash([]);
    ctx.stroke();

    ctx.beginPath();
    ctx.ellipse(sp.x, sp.y, rx, ry, 0, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(122,170,122,0.2)';
    ctx.lineWidth   = 0.5 / camScale;
    ctx.setLineDash([oda, oda * 1.6]);
    ctx.stroke();
    ctx.setLineDash([]);
  }

  function drawWaveThreads() {
    if (level !== 1 || !selectedSession || !graph) return;
    const session = selectedSession;
    const sp = sessionPos[session.id];
    if (!sp) return;

    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(session.id));
    if (sessionNodes.length === 0) return;

    ctx.strokeStyle = 'rgba(122,170,122,0.16)';
    ctx.lineWidth = 0.6 / camScale;

    sessionNodes.forEach((node, index) => {
      const np = nodePos[node.id];
      if (!np) return;

      ctx.beginPath();
      ctx.moveTo(sp.x, sp.y);
      ctx.lineTo(np.x, np.y);
      ctx.stroke();

      if (index > 0) {
        const prev = nodePos[sessionNodes[index - 1].id];
        if (!prev) return;
        ctx.beginPath();
        ctx.moveTo(prev.x, prev.y);
        ctx.lineTo(np.x, np.y);
        ctx.strokeStyle = 'rgba(122,170,122,0.11)';
        ctx.stroke();
        ctx.strokeStyle = 'rgba(122,170,122,0.16)';
      }
    });
  }

  function drawNodes() {
    if (!graph || level < 1 || !selectedSession) return;
    const av = { stability:0.75, friction:0.18, logic:0.85, autonomy:0.9, psi:2.68 };
    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(selectedSession!.id));

    sessionNodes.forEach(n => {
      const np = nodePos[n.id];
      if (!np) return;
      const r          = nodeRadius(n);
      const isSelected = selectedNode?.id === n.id;

      if (level === 2 && !isSelected) {
        ctx.beginPath();
        ctx.arc(np.x, np.y, 2.6, 0, Math.PI * 2);
        ctx.fillStyle = 'rgba(255,255,255,0.08)';
        ctx.fill();
        return;
      }

      const pulse = Math.sin(t * 1.8 + np.x * 0.04);
      const tierAlpha = n.tier === 'daily' ? 0.32 : n.tier === 'weekly' ? 0.28 : 0.24;
      const outerAlpha = isSelected ? 0.16 : 0.1;

      ctx.beginPath();
      ctx.arc(np.x, np.y, r + pulse + 7, 0, Math.PI * 2);
      ctx.fillStyle = avecColor(av, outerAlpha);
      ctx.fill();

      ctx.beginPath();
      ctx.arc(np.x, np.y, r + pulse, 0, Math.PI * 2);
      ctx.fillStyle   = avecColor(av, isSelected ? 0.72 : tierAlpha);
      ctx.fill();
      ctx.strokeStyle = avecColor(av, isSelected ? 0.95 : 0.38);
      ctx.lineWidth   = (isSelected ? 0.9 : 0.6) / camScale;
      ctx.stroke();

      ctx.beginPath();
      ctx.arc(np.x, np.y, Math.max(2.2, r * 0.34), 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(232,245,245,0.78)';
      ctx.fill();

    });
  }

  function drawWaveLabels() {
    if (level !== 1 || !graph || !selectedSession) return;
    const session = selectedSession;
    const sp = sessionPos[session.id];
    if (!sp) return;

    const sessionNodes = graph.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(session.id));
    const labeledMoments = [...sessionNodes]
      .sort((a, b) => b.psi - a.psi)
      .slice(0, Math.min(4, sessionNodes.length));

    labeledMoments.forEach(node => {
      const np = nodePos[node.id];
      if (!np) return;
      const sc = toScreen(np.x, np.y);
      ctx.textAlign = 'center';
      ctx.font = `600 8px ${FONT_MONO}`;
      ctx.strokeStyle = 'rgba(7,10,13,0.85)';
      ctx.lineWidth = 2.4;
      ctx.strokeText(momentWhisperLabel(node), sc.x, sc.y + 22);
      ctx.fillStyle = 'rgba(196,223,196,0.42)';
      ctx.fillText(momentWhisperLabel(node), sc.x, sc.y + 22);
    });

    const center = toScreen(sp.x, sp.y);
    ctx.textAlign = 'center';
    ctx.font = `600 9px ${FONT_MONO}`;
    ctx.strokeStyle = 'rgba(7,10,13,0.9)';
    ctx.lineWidth = 2.8;
    ctx.strokeText(waveTitle(session), center.x, center.y + 28);
    ctx.fillStyle = 'rgba(210,228,210,0.46)';
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
    const descriptor = collapseDescriptors(avec);
    const orbX = sc.x;
    const orbY = sc.y - 12;
    const breathe = Math.sin(t * 1.2) * 4;
    const shimmer = Math.sin(t * 2.4) * 0.08;

    ctx.beginPath();
    ctx.ellipse(orbX, orbY + 14, 118, 92, 0, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(122,170,122,0.06)';
    ctx.lineWidth = 0.6;
    ctx.setLineDash([3, 7]);
    ctx.stroke();

    ctx.beginPath();
    ctx.ellipse(orbX, orbY + 10, 84, 66, 0, 0, Math.PI * 2);
    ctx.strokeStyle = 'rgba(255,255,255,0.035)';
    ctx.lineWidth = 18;
    ctx.setLineDash([]);
    ctx.stroke();

    ctx.beginPath();
    ctx.ellipse(orbX, orbY + 6, 50, 42, 0, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${col},0.045)`;
    ctx.fill();

    const glo = ctx.createRadialGradient(orbX, orbY, 14, orbX, orbY, 96 + breathe);
    glo.addColorStop(0, `rgba(${col},0.2)`);
    glo.addColorStop(1, `rgba(${col},0)`);
    ctx.beginPath();
    ctx.arc(orbX, orbY, 96 + breathe, 0, Math.PI * 2);
    ctx.fillStyle = glo;
    ctx.fill();

    ctx.beginPath();
    ctx.arc(orbX, orbY, 42 + breathe * 0.5, 0, Math.PI * 2);
    ctx.fillStyle   = `rgba(${col},0.075)`;
    ctx.fill();
    ctx.strokeStyle = `rgba(${col},0.22)`;
    ctx.lineWidth   = 0.5;
    ctx.stroke();

    ctx.beginPath();
    ctx.arc(orbX, orbY, 28 + breathe * 0.3, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${col},${0.24 + shimmer})`;
    ctx.fill();

    ctx.beginPath();
    ctx.arc(orbX, orbY, 17, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${col},0.82)`;
    ctx.fill();

    ctx.beginPath();
    ctx.arc(orbX - 6, orbY - 6, 5.2, 0, Math.PI * 2);
    ctx.fillStyle = `rgba(${Math.min(255,Math.round(r+45))},${Math.min(255,Math.round(g+45))},${Math.min(255,Math.round(b+45))},0.55)`;
    ctx.fill();

    [
      { x: orbX - 34, y: orbY - 18, radius: 1.8, alpha: 0.18 },
      { x: orbX + 28, y: orbY + 20, radius: 1.4, alpha: 0.14 },
      { x: orbX + 40, y: orbY - 10, radius: 1.2, alpha: 0.12 },
    ].forEach(mote => {
      ctx.beginPath();
      ctx.arc(mote.x, mote.y, mote.radius + Math.sin(t * 1.5 + mote.x * 0.03) * 0.2, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(${col},${mote.alpha})`;
      ctx.fill();
    });

    const threads: [{ x: number; y: number; c1x: number; c1y: number; c2x: number; c2y: number; alpha: number }, ...Array<{ x: number; y: number; c1x: number; c1y: number; c2x: number; c2y: number; alpha: number }>] = [
      { x: W() - 18, y: 22, c1x: orbX + 44, c1y: orbY - 26, c2x: W() - 90, c2y: 40, alpha: 0.14 },
      { x: 22, y: 34, c1x: orbX - 46, c1y: orbY - 18, c2x: 80, c2y: 46, alpha: 0.1 },
      { x: W() - 24, y: H() - 86, c1x: orbX + 36, c1y: orbY + 26, c2x: W() - 72, c2y: H() - 130, alpha: 0.08 },
    ];
    threads.forEach(thread => {
      ctx.beginPath();
      ctx.moveTo(orbX, orbY);
      ctx.bezierCurveTo(thread.c1x, thread.c1y, thread.c2x, thread.c2y, thread.x, thread.y);
      ctx.strokeStyle = `rgba(${col},${thread.alpha})`;
      ctx.lineWidth   = 0.45;
      ctx.setLineDash([2, 5]);
      ctx.stroke();
      ctx.setLineDash([]);
    });

  }

  function drawHints() {
    if (loading) return;
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

  // ── Main render loop ────────────────────────────────────────────
  function draw() {
    if (!ctx) { raf = requestAnimationFrame(draw); return; }

    camX     += (targetCamX     - camX)     * LERP;
    camY     += (targetCamY     - camY)     * LERP;
    camScale += (targetCamScale - camScale) * LERP;

    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.setTransform(deviceScale, 0, 0, deviceScale, 0, 0);
    ctx.fillStyle = '#0a0b0e';
    ctx.fillRect(0, 0, W(), H());

    drawStars();

    ctx.save();
    ctx.translate(W() / 2, H() / 2);
    ctx.scale(camScale, camScale);
    ctx.translate(-camX, -camY);
    drawEdges();
    drawSessions();
    drawWaveBoundary();
    drawWaveThreads();
    drawNodes();
    ctx.restore();

    drawWaveLabels();
    drawCollapseOrb();
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
    if (level === 0) {
      camX = targetCamX = W() / 2;
      camY = targetCamY = H() / 2;
      camScale = targetCamScale = CONSTELLATION_SCALE;
    }
  }

  // ── Pointer events ─────────────────────────────────────────────
  function onPointerDown(e: MouseEvent) {
    if (level !== 0) return;
    dragging    = true;
    didDrag     = false;
    dragStart   = { x: e.clientX, y: e.clientY };
    panCamStart = { x: camX, y: camY };
  }

  function onPointerMove(e: MouseEvent) {
    if (!dragging || level !== 0) return;
    const dx = e.clientX - dragStart.x;
    const dy = e.clientY - dragStart.y;
    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) didDrag = true;
    camX = targetCamX = panCamStart.x - dx / camScale;
    camY = targetCamY = panCamStart.y - dy / camScale;
  }

  function onPointerUp(e: MouseEvent) {
    dragging = false;
    if (didDrag) return;

    const { x: sx, y: sy } = canvasXY(e);

    if (level === 2) { surfaceToWave(); return; }

    if (level === 1 && selectedSession) {
      const sessionNodes = graph?.nodes.filter(n => sessionKey(n.sessionId) === sessionKey(selectedSession!.id)) ?? [];
      for (const n of sessionNodes) {
        const np = nodePos[n.id];
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

  // ── Health ───────────────────────────────────────────────────
  let healthy = false;
  async function checkHealth() {
    try { await invoke('get_health'); healthy = true; }
    catch { healthy = false; }
  }

  // ── Menu / Settings ───────────────────────────────────────
  let menuOpen = false;
  let settingsOpen = false;
  let settingsLoading = false;
  let settingsSaving = false;
  let settingsError: string | null = null;
  let settingsSaved = false;
  let gatewayBaseUrl = '';
  let ollamaBaseUrl = '';
  let ollamaModel = '';

  async function openSettings() {
    menuOpen = false;
    settingsOpen = true;
    settingsLoading = true;
    settingsError = null;
    settingsSaved = false;

    try {
      const config = await invoke<{
        gatewayBaseUrl: string;
        ollamaBaseUrl: string;
        ollamaModel: string;
      }>('get_config');
      gatewayBaseUrl = config.gatewayBaseUrl;
      ollamaBaseUrl = config.ollamaBaseUrl;
      ollamaModel = config.ollamaModel;
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
      await invoke('set_gateway_base_url', { baseUrl: gatewayBaseUrl.trim() });
      await invoke('set_ollama_config', {
        baseUrl: ollamaBaseUrl.trim(),
        model: ollamaModel.trim(),
      });
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
  let composeText     = '';
  let composeSessionId = '';
  let composeLoading  = false;
  let composeError: string | null = null;
  let composeResult: { psi: number } | null = null;

  function openCompose() {
    composeSessionId = selectedSession?.id ?? '';
    composeError = null; composeResult = null;
    composeOpen = true;
  }

  async function submitCompose() {
    if (!composeText.trim()) return;
    composeLoading = true; composeError = null; composeResult = null;
    try {
      const res = await invoke<{ nodeId: string; psi: number; valid: boolean; validationError: string | null }>(
        'store_context', { request: { node: composeText, sessionId: composeSessionId } },
      );
      composeResult = { psi: res.psi };
      composeText = '';
      await loadGraph();
    } catch (err) { composeError = String(err); }
    finally      { composeLoading = false; }
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

  function openCalibrate() {
    menuOpen = false;
    calibSessionId = selectedSession?.id ?? '';
    calibError = null;
    calibrateOpen = true;
  }

  async function submitCalibrate() {
    calibLoading = true; calibError = null;
    try {
      await invoke('calibrate_session', {
        request: {
          sessionId: calibSessionId,
          stability: calibStability,
          friction:  calibFriction,
          logic:     calibLogic,
          autonomy:  calibAutonomy,
          trigger:   calibTrigger,
        },
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
    const ro = new ResizeObserver(resize);
    ro.observe(container);
    return () => ro.disconnect();
  });

  onDestroy(() => cancelAnimationFrame(raf));
</script>

<div class="weaver-root" bind:this={container}>
  <canvas
    bind:this={canvas}
    on:mousedown={onPointerDown}
    on:mousemove={onPointerMove}
    on:mouseup={onPointerUp}
    on:mouseleave={() => (dragging = false)}
    class:grabbing={dragging}
  ></canvas>

  <nav class="navbar">
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
            <button class="menu-item" on:click={() => { menuOpen = false; loadGraph(); }}>refresh</button>
            <button class="menu-item" on:click={openCalibrate}>calibrate</button>
            <button class="menu-item" on:click={openSettings}>settings</button>
          </div>
        {/if}
      </div>
    </div>
  </nav>

  <CollapseCard
    data={cardData}
    visible={cardVisible}
    on:close={closeCard}
    on:navigate={handleNavigate}
  />

  <button class="compose-btn" on:click={openCompose}>+ compose</button>

  {#if composeOpen}
    <div class="drawer" role="dialog" aria-label="Compose context">
      <div class="drawer-header">
        <span class="drawer-title">compose</span>
        <button class="close-btn" on:click={() => (composeOpen = false)}>✕</button>
      </div>
      <input class="drawer-input" type="text" placeholder="session id" bind:value={composeSessionId} />
      <textarea class="drawer-textarea" placeholder="what happened…" bind:value={composeText} rows="6"></textarea>
      {#if composeError}<p class="drawer-error">{composeError}</p>{/if}
      {#if composeResult}<p class="drawer-success">stored · Ψ {composeResult.psi.toFixed(4)}</p>{/if}
      <div class="drawer-actions">
        <button class="drawer-btn cancel" on:click={() => (composeOpen = false)}>cancel</button>
        <button class="drawer-btn submit" on:click={submitCompose} disabled={composeLoading}>
          {composeLoading ? 'storing…' : 'store'}
        </button>
      </div>
    </div>
  {/if}

  {#if calibrateOpen}
    <div class="drawer" role="dialog" aria-label="Calibrate session">
      <div class="drawer-header">
        <span class="drawer-title">calibrate</span>
        <button class="close-btn" on:click={() => (calibrateOpen = false)}>✕</button>
      </div>
      <input class="drawer-input" type="text" placeholder="session id" bind:value={calibSessionId} />
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.stability}">stability</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibStability} class="avec-slider" />
        <span class="slider-val">{calibStability.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.friction}">friction</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibFriction} class="avec-slider" />
        <span class="slider-val">{calibFriction.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.logic}">logic</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibLogic} class="avec-slider" />
        <span class="slider-val">{calibLogic.toFixed(2)}</span>
      </div>
      <div class="slider-row">
        <span class="slider-label" style="color:{AVEC_HEX.autonomy}">autonomy</span>
        <input type="range" min="0" max="1" step="0.01" bind:value={calibAutonomy} class="avec-slider" />
        <span class="slider-val">{calibAutonomy.toFixed(2)}</span>
      </div>
      <input class="drawer-input" type="text" placeholder="trigger (e.g. session_start, manual)" bind:value={calibTrigger} />
      {#if calibError}<p class="drawer-error">{calibError}</p>{/if}
      <div class="drawer-actions">
        <button class="drawer-btn cancel" on:click={() => (calibrateOpen = false)}>cancel</button>
        <button class="drawer-btn submit" on:click={submitCalibrate} disabled={calibLoading}>
          {calibLoading ? 'calibrating…' : 'calibrate'}
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
      <input class="drawer-input" type="text" placeholder="gateway base url" bind:value={gatewayBaseUrl} disabled={settingsLoading || settingsSaving} />
      <input class="drawer-input" type="text" placeholder="ollama base url" bind:value={ollamaBaseUrl} disabled={settingsLoading || settingsSaving} />
      <input class="drawer-input" type="text" placeholder="ollama model" bind:value={ollamaModel} disabled={settingsLoading || settingsSaving} />
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
    position: fixed;
    inset: 0;
    width: 100vw;
    height: 100vh;
    min-height: 100dvh;
    overflow: hidden;
    background: #0a0b0e;
    font-family: 'Departure Mono', 'Courier New', monospace;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    cursor: grab;
    display: block;
  }
  canvas.grabbing { cursor: grabbing; }

  .navbar {
    position: absolute;
    top: 0; left: 0; right: 0;
    height: 46px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 22px;
    background: linear-gradient(to bottom, rgba(10,11,14,0.88) 0%, transparent 100%);
    z-index: 10;
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


  .compose-btn {
    position: absolute;
    bottom: 24px; right: 24px;
    font-family: 'Departure Mono', monospace;
    font-size: 11px;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.6);
    background: rgba(14, 16, 22, 0.88);
    border: 0.5px solid rgba(255, 255, 255, 0.14);
    border-radius: 6px;
    padding: 10px 18px;
    cursor: pointer;
    z-index: 10;
    transition: all 0.2s;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
  }
  .compose-btn:hover {
    color: #fff;
    border-color: rgba(255, 255, 255, 0.28);
    background: rgba(20, 23, 32, 0.95);
  }

  .drawer {
    position: absolute;
    bottom: 70px; left: 50%;
    transform: translateX(-50%);
    width: 360px;
    background: rgba(10, 11, 14, 0.97);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 20px;
    z-index: 20;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    font-family: 'Departure Mono', 'Courier New', monospace;
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

  .slider-row { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
  .slider-label { font-size: 9px; letter-spacing: 0.1em; text-transform: uppercase; width: 58px; flex-shrink: 0; }
  .avec-slider  { flex: 1; accent-color: rgba(255,255,255,0.4); height: 2px; }
  .slider-val   { font-size: 10px; color: rgba(255,255,255,0.4); width: 32px; text-align: right; flex-shrink: 0; }

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
