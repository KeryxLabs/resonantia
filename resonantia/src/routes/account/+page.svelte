<script lang="ts">
  import { onMount } from 'svelte';
  import { AVEC_COLORS } from '@resonantia/core';
  import { getGatewayBaseUrl } from '$lib/config';
  import {
    getCloudAuthStatus,
    getGatewayAuthToken,
    redirectToCloudSignIn,
    signOutCloud,
    getCloudAccount,
    createCheckoutSession,
    createCustomerPortal,
  } from '$lib/cloudAuth';

  // ── State ───────────────────────────────────────────────────────────────────
  type Phase = 'loading' | 'signed-out' | 'signed-in';

  let phase: Phase = 'loading';
  let userId: string | null = null;
  let accountTier: string | null = null;
  let memberSince: string | null = null;
  let authError: string | null = null;
  let busy = false;
  let checkoutBusy = false;
  let checkoutingTier: 'resonant' | 'soulful' | null = null;
  let checkoutError: string | null = null;
  let portalBusy = false;
  let portalError: string | null = null;

  const gatewayBaseUrl = getGatewayBaseUrl();

  // ── Canvas stars ────────────────────────────────────────────────────────────
  let canvas: HTMLCanvasElement;

  const STARS = Array.from({ length: 60 }, () => ({
    x: Math.random(),
    y: Math.random(),
    s: 0.3 + Math.random() * 0.8,
    phase: Math.random() * Math.PI * 2,
  }));

  // Four AVEC hue blobs — subtle, slow-moving
  const BLOBS = [
    { dim: 'stability',  cx: 0.18, cy: 0.25, r: 0.32, speed: 0.00018 },
    { dim: 'logic',      cx: 0.82, cy: 0.20, r: 0.28, speed: 0.00022 },
    { dim: 'autonomy',   cx: 0.75, cy: 0.78, r: 0.30, speed: 0.00015 },
    { dim: 'friction',   cx: 0.22, cy: 0.72, r: 0.26, speed: 0.00019 },
  ] as const;

  type BlobDim = typeof BLOBS[number]['dim'];

  function drawCanvas(t: number) {
    if (!canvas) return;
    const W = canvas.width;
    const H = canvas.height;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, W, H);

    // Blobs
    for (const blob of BLOBS) {
      const col = AVEC_COLORS[blob.dim as BlobDim];
      const cx = (blob.cx + Math.sin(t * blob.speed) * 0.06) * W;
      const cy = (blob.cy + Math.cos(t * blob.speed * 0.7) * 0.05) * H;
      const r = blob.r * Math.min(W, H);
      const grad = ctx.createRadialGradient(cx, cy, 0, cx, cy, r);
      grad.addColorStop(0, `rgba(${col.r},${col.g},${col.b},0.055)`);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      ctx.fillStyle = grad;
      ctx.fillRect(0, 0, W, H);
    }

    // Stars
    for (const star of STARS) {
      const alpha = 0.18 + Math.sin(t * 0.0009 + star.phase) * 0.12;
      ctx.fillStyle = `rgba(255,255,255,${alpha.toFixed(3)})`;
      ctx.beginPath();
      ctx.arc(star.x * W, star.y * H, star.s, 0, Math.PI * 2);
      ctx.fill();
    }
  }

  let raf = 0;
  function startLoop() {
    const step = (t: number) => {
      drawCanvas(t);
      raf = requestAnimationFrame(step);
    };
    raf = requestAnimationFrame(step);
  }
  function stopLoop() {
    if (raf) { cancelAnimationFrame(raf); raf = 0; }
  }

  function resizeCanvas() {
    if (!canvas) return;
    canvas.width = window.innerWidth * devicePixelRatio;
    canvas.height = window.innerHeight * devicePixelRatio;
    canvas.style.width = window.innerWidth + 'px';
    canvas.style.height = window.innerHeight + 'px';
  }

  // ── Icosahedron orb ──────────────────────────────────────────────────────────
  let orbCanvas: HTMLCanvasElement;
  let orbRaf = 0;
  let orbT = 0;

  const ORBC = {
    stability: { r: 232, g: 232, b: 235 },
    friction:  { r: 233, g: 148, b: 58  },
    logic:     { r:  91, g: 155, b: 213 },
    autonomy:  { r: 122, g: 170, b: 122 },
  } as const;

  type OrbColorKey = keyof typeof ORBC;
  type OrbColor = { r: number; g: number; b: number };
  type OrbAVEC = { s: number; f: number; l: number; a: number };

  const ICO_VERTS: [number, number, number][] = (() => {
    const phi = (1 + Math.sqrt(5)) / 2;
    return ([
      [-1,phi,0],[1,phi,0],[-1,-phi,0],[1,-phi,0],
      [0,-1,phi],[0,1,phi],[0,-1,-phi],[0,1,-phi],
      [phi,0,-1],[phi,0,1],[-phi,0,-1],[-phi,0,1],
    ] as [number,number,number][]).map(([x,y,z]) => {
      const n = Math.sqrt(x*x + y*y + z*z);
      return [x/n, y/n, z/n] as [number,number,number];
    });
  })();

  const ICO_FACES: [number, number, number][] = [
    [0,11,5],[0,5,1],[0,1,7],[0,7,10],[0,10,11],
    [1,5,9],[5,11,4],[11,10,2],[10,7,6],[7,1,8],
    [3,9,4],[3,4,2],[3,2,6],[3,6,8],[3,8,9],
    [4,9,5],[2,4,11],[6,2,10],[8,6,7],[9,8,1],
  ];

  type OrbStream = {
    side: 1 | -1; offset: number;
    col: OrbColor; destCol: OrbColor;
    thickness: number; phase: number; speed: number;
  };
  let orbStreams: OrbStream[] = [];

  const DIM_KEY: Record<OrbColorKey, keyof OrbAVEC> = {
    stability: 's', friction: 'f', logic: 'l', autonomy: 'a',
  };

  function initOrbStreams(avec: OrbAVEC) {
    orbStreams = [];
    const ranked: [OrbColorKey, number][] = [
      ['stability', avec.s], ['friction', avec.f], ['logic', avec.l], ['autonomy', avec.a],
    ];
    ranked.sort((a, b) => b[1] - a[1]);
    for (let i = 0; i < 4; i++) {
      const side = (i % 2 === 0 ? -1 : 1) as 1 | -1;
      const dim = ranked[Math.min(i, ranked.length - 1)][0];
      const destDim = ranked[Math.min(i + 1, ranked.length - 1)][0];
      const dimVal = avec[DIM_KEY[dim]];
      const thickness = dimVal * 2.5 + 0.5;
      const count = Math.round(dimVal * 3) + 1;
      for (let j = 0; j < count; j++) {
        orbStreams.push({
          side, offset: (j - (count - 1) / 2) * 7,
          col: { ...ORBC[dim] }, destCol: { ...ORBC[destDim] },
          thickness: thickness * (0.6 + Math.random() * 0.4),
          phase: Math.random() * Math.PI * 2,
          speed: 0.4 + Math.random() * 0.3,
        });
      }
    }
  }

  function orbMixColor(a: OrbAVEC): OrbColor {
    const ws = [a.s, a.f, a.l, a.a];
    const cs: OrbColor[] = [ORBC.stability, ORBC.friction, ORBC.logic, ORBC.autonomy];
    let r = 0, g = 0, b = 0, tot = 0;
    ws.forEach((w, i) => { r += cs[i].r * w; g += cs[i].g * w; b += cs[i].b * w; tot += w; });
    return { r: r / tot, g: g / tot, b: b / tot };
  }

  function orbProject(
    v: [number,number,number], rx: number, ry: number, rz: number,
    scale: number, CX: number, CY: number,
  ) {
    const [vx, vy, vz] = v;
    const ax = vx;
    const ay = vy * Math.cos(rx) - vz * Math.sin(rx);
    const az = vy * Math.sin(rx) + vz * Math.cos(rx);
    const bx = ax * Math.cos(ry) + az * Math.sin(ry);
    const by = ay;
    const bz = -ax * Math.sin(ry) + az * Math.cos(ry);
    const cx2 = bx * Math.cos(rz) - by * Math.sin(rz);
    const cy2 = bx * Math.sin(rz) + by * Math.cos(rz);
    const fov = 3 / (3 + bz);
    return { px: CX + cx2 * scale * fov, py: CY + cy2 * scale * fov, z: bz };
  }

  function drawOrb(t: number, avec: OrbAVEC) {
    if (!orbCanvas) return;
    const ctx = orbCanvas.getContext('2d');
    if (!ctx) return;
    const W = orbCanvas.width, H = orbCanvas.height;
    const CX = W / 2, CY = H / 2;
    ctx.clearRect(0, 0, W, H);

    const psiVal = avec.s + avec.f + avec.l + avec.a;
    const mixed = orbMixColor(avec);
    const rotX = t * 0.17, rotY = t * 0.23, rotZ = t * 0.07;
    const radius = Math.min(W, H) * 0.275;
    const pulseRate = 0.3 + (1 - psiVal / 4) * 1.2;
    const pulseAmt  = 0.04 + (1 - psiVal / 4) * 0.08;
    const scale = radius * (1 + Math.sin(t * pulseRate) * pulseAmt);

    // Energy streams
    orbStreams.forEach(s => {
      const startX = s.side < 0 ? -W * 0.1 : W * 1.1;
      const endX   = s.side < 0 ? CX - scale * 0.85 : CX + scale * 0.85;
      const midX   = (startX + endX) / 2;
      const baseY  = CY + s.offset;
      const wave   = Math.sin(t * s.speed + s.phase) * (12 + Math.abs(s.offset) * 0.5);
      const grad = ctx.createLinearGradient(startX, baseY, endX, baseY);
      const sc = s.col, dc = s.destCol;
      if (s.side < 0) {
        grad.addColorStop(0,    `rgba(${sc.r},${sc.g},${sc.b},0)`);
        grad.addColorStop(0.4,  `rgba(${sc.r},${sc.g},${sc.b},0.5)`);
        grad.addColorStop(0.85, `rgba(${dc.r},${dc.g},${dc.b},0.3)`);
        grad.addColorStop(1,    `rgba(${dc.r},${dc.g},${dc.b},0)`);
      } else {
        grad.addColorStop(0,    `rgba(${sc.r},${sc.g},${sc.b},0)`);
        grad.addColorStop(0.15, `rgba(${sc.r},${sc.g},${sc.b},0.3)`);
        grad.addColorStop(0.6,  `rgba(${dc.r},${dc.g},${dc.b},0.5)`);
        grad.addColorStop(1,    `rgba(${dc.r},${dc.g},${dc.b},0)`);
      }
      ctx.beginPath();
      ctx.moveTo(startX, baseY);
      ctx.bezierCurveTo(midX * 0.5, baseY + wave, endX * 0.7, baseY - wave * 0.5, endX, baseY + s.offset * 0.1);
      ctx.strokeStyle = grad;
      ctx.lineWidth = s.thickness;
      ctx.lineCap = 'round';
      ctx.stroke();
    });

    // Inner glow
    const glowAlpha = 0.12 + Math.sin(t * pulseRate) * 0.04;
    const glowGrad = ctx.createRadialGradient(CX, CY, 0, CX, CY, scale * 0.98);
    glowGrad.addColorStop(0,   `rgba(${mixed.r},${mixed.g},${mixed.b},${glowAlpha * 2})`);
    glowGrad.addColorStop(0.5, `rgba(${mixed.r},${mixed.g},${mixed.b},${glowAlpha})`);
    glowGrad.addColorStop(1,   `rgba(${mixed.r},${mixed.g},${mixed.b},0)`);
    ctx.fillStyle = glowGrad;
    ctx.beginPath();
    ctx.arc(CX, CY, scale * 0.98, 0, Math.PI * 2);
    ctx.fill();

    // Faces
    const pv = ICO_VERTS.map(v => orbProject(v, rotX, rotY, rotZ, scale, CX, CY));
    const colPalette: OrbColor[] = [ORBC.stability, ORBC.friction, ORBC.logic, ORBC.autonomy];
    const dimWeights = [avec.s, avec.f, avec.l, avec.a];
    ICO_FACES
      .map((f, fi) => ({ f, fi, z: (pv[f[0]].z + pv[f[1]].z + pv[f[2]].z) / 3 }))
      .sort((a, b) => a.z - b.z)
      .forEach(({ f, fi, z }) => {
        const [pa, pb, pc] = f.map(i => pv[i]);
        const normZ = (z + 1) / 2;
        const brightness = 0.15 + normZ * 0.4;
        const di = fi % 4;
        const alpha = Math.min(brightness * dimWeights[di] * 0.6 + brightness * 0.3, 0.85);
        const col = colPalette[di];
        ctx.beginPath();
        ctx.moveTo(pa.px, pa.py); ctx.lineTo(pb.px, pb.py); ctx.lineTo(pc.px, pc.py);
        ctx.closePath();
        ctx.fillStyle = `rgba(${col.r},${col.g},${col.b},${alpha})`;
        ctx.fill();
        ctx.strokeStyle = `rgba(255,255,255,${(0.08 + normZ * 0.12).toFixed(3)})`;
        ctx.lineWidth = 0.5;
        ctx.stroke();
      });

    // Core
    const coreR = scale * 0.22;
    const coreAlpha = 0.7 + Math.sin(t * pulseRate * 2) * 0.15;
    const coreGrad = ctx.createRadialGradient(CX - coreR * 0.3, CY - coreR * 0.3, 0, CX, CY, coreR);
    coreGrad.addColorStop(0,   `rgba(255,255,255,${coreAlpha})`);
    coreGrad.addColorStop(0.4, `rgba(${mixed.r},${mixed.g},${mixed.b},${coreAlpha * 0.8})`);
    coreGrad.addColorStop(1,   `rgba(${mixed.r},${mixed.g},${mixed.b},0)`);
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(CX, CY, coreR, 0, Math.PI * 2);
    ctx.fill();
  }

  let currentOrbDims: OrbAVEC = { s: 0.65, f: 0.22, l: 0.72, a: 0.55 };

  function getOrbDims(p: Phase, tier: string | null): OrbAVEC {
    if (p === 'loading')    return { s: 0.40, f: 0.40, l: 0.40, a: 0.40 };
    if (p === 'signed-out') return { s: 0.65, f: 0.22, l: 0.72, a: 0.55 };
    if (tier === 'soulful') return { s: 0.95, f: 0.15, l: 0.90, a: 0.95 };
    if (tier === 'resonant') return { s: 0.88, f: 0.18, l: 0.85, a: 0.82 };
    return { s: 0.80, f: 0.28, l: 0.82, a: 0.75 }; // free
  }

  $: {
    currentOrbDims = getOrbDims(phase, accountTier);
    if (orbCanvas) initOrbStreams(currentOrbDims);
  }

  function startOrbLoop() {
    function step() {
      orbT += 0.008;
      drawOrb(orbT, currentOrbDims);
      orbRaf = requestAnimationFrame(step);
    }
    orbRaf = requestAnimationFrame(step);
  }

  function stopOrbLoop() {
    if (orbRaf) { cancelAnimationFrame(orbRaf); orbRaf = 0; }
  }

  function setupOrbCanvas() {
    if (!orbCanvas) return;
    const dpr = window.devicePixelRatio || 1;
    orbCanvas.width  = 200 * dpr;
    orbCanvas.height = 200 * dpr;
    orbCanvas.style.width  = '200px';
    orbCanvas.style.height = '200px';
    initOrbStreams(currentOrbDims);
  }

  // ── Auth helpers ───────────────────────────────────────────────────────────
  async function loadAccountState() {
    phase = 'loading';
    authError = null;
    try {
      const status = await getCloudAuthStatus();
      if (!status.signedIn) {
        phase = 'signed-out';
        return;
      }
      userId = status.username ?? status.userId;
      const token = await getGatewayAuthToken().catch(() => '');
      const account = gatewayBaseUrl
        ? await getCloudAccount(gatewayBaseUrl, token).catch(() => null)
        : null;
      accountTier = account?.tier ?? null;
      memberSince = account?.memberSince ?? null;
      phase = 'signed-in';
    } catch (err) {
      authError = String(err);
      phase = 'signed-out';
    }
  }

  async function signIn() {
    if (busy) return;
    busy = true;
    authError = null;
    try {
      await redirectToCloudSignIn(window.location.href);
      // page will redirect — no further execution
    } catch (err) {
      authError = String(err);
    } finally {
      busy = false;
    }
  }

  async function signOut() {
    if (busy) return;
    busy = true;
    authError = null;
    try {
      await signOutCloud();
      userId = null;
      accountTier = null;
      memberSince = null;
      phase = 'signed-out';
    } catch (err) {
      authError = String(err);
    } finally {
      busy = false;
    }
  }

  async function startUpgrade(tier: 'resonant' | 'soulful') {
    if (checkoutBusy) return;
    checkoutBusy = true;
    checkoutingTier = tier;
    checkoutError = null;
    try {
      const token = await getGatewayAuthToken();
      const url = await createCheckoutSession(gatewayBaseUrl, token, tier);
      window.location.href = url;
    } catch (err) {
      checkoutError = String(err);
    } finally {
      checkoutBusy = false;
      checkoutingTier = null;
    }
  }

  async function openBillingPortal() {
    if (portalBusy) return;
    portalBusy = true;
    portalError = null;
    try {
      const token = await getGatewayAuthToken();
      const url = await createCustomerPortal(gatewayBaseUrl, token);
      window.location.href = url;
    } catch (err) {
      portalError = String(err);
    } finally {
      portalBusy = false;
    }
  }

  function formatMemberSince(iso: string): string {
    try {
      return new Date(iso).toLocaleDateString([], { month: 'long', year: 'numeric' });
    } catch { return ''; }
  }

  function shortUserId(id: string | null): string {
    if (!id) return '';
    return id.length > 14 ? `${id.slice(0, 7)}…${id.slice(-5)}` : id;
  }

  // ── Payment success/cancelled query param ──────────────────────────────────
  let paymentStatus: 'success' | 'cancelled' | null = null;

  onMount(() => {
    const params = new URLSearchParams(window.location.search);
    const ps = params.get('payment');
    if (ps === 'success') paymentStatus = 'success';
    else if (ps === 'cancelled') paymentStatus = 'cancelled';

    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);
    startLoop();
    loadAccountState();
    setupOrbCanvas();
    startOrbLoop();

    return () => {
      stopLoop();
      stopOrbLoop();
      window.removeEventListener('resize', resizeCanvas);
    };
  });
</script>

<div class="root">
  <canvas bind:this={canvas} class="bg-canvas" aria-hidden="true"></canvas>

  <header class="topbar">
    <a href="/" class="wordmark">resonantia</a>
    <span class="topbar-label">account</span>
  </header>

  <main class="stage">

    {#if paymentStatus === 'success'}
      <div class="banner banner-success">
        <span class="banner-icon">✦</span>
        <span>subscription activated — welcome to resonantia.</span>
        <button class="banner-dismiss" on:click={() => (paymentStatus = null)}>✕</button>
      </div>
    {:else if paymentStatus === 'cancelled'}
      <div class="banner banner-info">
        <span>checkout cancelled. your plan is unchanged.</span>
        <button class="banner-dismiss" on:click={() => (paymentStatus = null)}>✕</button>
      </div>
    {/if}

    <!-- Single persistent card — orb canvas always rendered -->
    <div class="card">
      <canvas bind:this={orbCanvas} class="orb-canvas" aria-hidden="true"></canvas>

    {#if phase === 'loading'}
      <!-- Loading state -->
        <p class="hint">checking session…</p>

    {:else if phase === 'signed-out'}
      <!-- Sign-in state -->
        <h1 class="card-title">your resonantia account</h1>
        <p class="card-body">
          sign in to view your sync history, manage your subscription, and keep your context
          alive across every session.
        </p>
        {#if authError}
          <p class="inline-error">{authError}</p>
        {/if}
        <button class="action-btn primary" on:click={signIn} disabled={busy}>
          {busy ? 'connecting…' : 'sign in'}
        </button>
        <p class="hint">new to resonantia? an account is created automatically on first sign-in.</p>

    {:else}
      <!-- Signed-in state -->

        <!-- Identity row -->
        <div class="identity-row">
          <div class="identity-meta">
            <span class="identity-handle">{shortUserId(userId)}</span>
            {#if memberSince}
              <span class="identity-since">member since {formatMemberSince(memberSince)}</span>
            {/if}
          </div>
          {#if accountTier}
          <span class="tier-badge" class:resonant={accountTier === 'resonant'} class:soulful={accountTier === 'soulful'}>
              {accountTier}
            </span>
          {/if}
        </div>

      <hr class="divider" />

        {#if accountTier === 'free' || !accountTier}
          <!-- Upgrade section — two tiers -->
          <div class="upgrade-section">
            <h2 class="upgrade-title">choose your plan</h2>
            <div class="tier-cards">

              <div class="tier-card">
                <div class="tier-card-header">
                  <span class="tier-card-name">resonant</span>
                  <span class="tier-card-price">$4.99<span class="tier-card-period">/mo</span></span>
                </div>
                <ul class="upgrade-features">
                  <li><span class="feat-dot feat-stability"></span>cloud-backed surrealkv storage</li>
                  <li><span class="feat-dot feat-logic"></span>sync across devices and sessions</li>                              
                </ul>
                <button class="action-btn upgrade" on:click={() => startUpgrade('resonant')} disabled={checkoutBusy}>
                  {checkoutingTier === 'resonant' ? 'opening checkout…' : 'subscribe'}
                </button>
              </div>

              <div class="tier-card">
                <div class="tier-card-header">
                  <span class="tier-card-name">soulful</span>
                  <span class="tier-card-price">$8.99<span class="tier-card-period">/mo</span></span>
                </div>
                <ul class="upgrade-features">
                  <li><span class="feat-dot feat-stability"></span>everything in resonant</li>
                  <li><span class="feat-dot feat-logic"></span>managed AI — no key needed</li>
                  <li><span class="feat-dot feat-stability"></span>early access to new AI features</li>
                </ul>
                <button class="action-btn upgrade-soulful" on:click={() => startUpgrade('soulful')} disabled={checkoutBusy}>
                  {checkoutingTier === 'soulful' ? 'opening checkout…' : 'subscribe'}
                </button>
              </div>

            </div>
            {#if checkoutError}
              <p class="inline-error">{checkoutError}</p>
            {/if}
            <p class="hint">billed via stripe. cancel any time. tier updates within seconds of payment.</p>
          </div>

          <hr class="divider" />
        {:else if accountTier === 'resonant'}
          <!-- Resonant perks + upgrade to Soulful -->
          <div class="perks-section">
            <h2 class="perks-title resonant">you're on resonant</h2>
            <p class="perks-body">
              your sessions are backed by surrealkv disk storage with cloud sync across all devices.
            </p>
            {#if portalError}<p class="inline-error">{portalError}</p>{/if}
            <button class="action-btn ghost-small" on:click={openBillingPortal} disabled={portalBusy}>
              {portalBusy ? 'opening portal…' : 'manage billing →'}
            </button>
          </div>

          <div class="upgrade-section upgrade-section-compact">
            <div class="tier-card tier-card-single">
              <div class="tier-card-header">
                <span class="tier-card-name">upgrade to soulful</span>
                <span class="tier-card-price">$8.99<span class="tier-card-period">/mo</span></span>
              </div>
              <p class="upgrade-body">add managed AI to your plan — no API key needed.</p>
              <button class="action-btn upgrade-soulful" on:click={() => startUpgrade('soulful')} disabled={checkoutBusy}>
                {checkoutingTier === 'soulful' ? 'opening checkout…' : 'upgrade'}
              </button>
            </div>
            {#if checkoutError}
              <p class="inline-error">{checkoutError}</p>
            {/if}
          </div>

          <hr class="divider" />
        {:else if accountTier === 'soulful'}
          <!-- Soulful perks summary -->
          <div class="perks-section">
            <h2 class="perks-title soulful">you're on soulful</h2>
            <p class="perks-body">
              full cloud storage, session sync, and managed AI — the complete resonantia experience.
            </p>
            {#if portalError}<p class="inline-error">{portalError}</p>{/if}
            <button class="action-btn ghost-small" on:click={openBillingPortal} disabled={portalBusy}>
              {portalBusy ? 'opening portal…' : 'manage billing →'}
            </button>
          </div>

          <hr class="divider" />
        {/if}

        <!-- Back to app + sign out -->
        <div class="footer-actions">
          <a href="/" class="action-btn secondary">open app</a>
          {#if authError}
            <p class="inline-error">{authError}</p>
          {/if}
          <button class="action-btn ghost" on:click={signOut} disabled={busy}>
            {busy ? '…' : 'sign out'}
          </button>
        </div>

    {/if}
    </div>

  </main>

  <footer class="page-footer">
    <span>resonantia · <a href="/" class="footer-link">open app</a></span>
  </footer>
</div>

<style>
  /* ── Root ───────────────────────────────────────────────────────────────── */
  :global(body) {
    margin: 0;
    background: #070a12;
    color: rgba(255, 255, 255, 0.72);
    -webkit-font-smoothing: antialiased;
  }

  .root {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    font-family: 'Departure Mono', 'Courier New', monospace;
    overflow-y: auto;
  }

  /* ── Canvas ─────────────────────────────────────────────────────────────── */
  .bg-canvas {
    position: fixed;
    inset: 0;
    pointer-events: none;
    z-index: 0;
  }

  /* ── Topbar ─────────────────────────────────────────────────────────────── */
  .topbar {
    position: fixed;
    top: 0; left: 0; right: 0;
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 28px;
    background: linear-gradient(to bottom, rgba(7,10,18,0.92) 0%, transparent 100%);
    z-index: 10;
  }

  .wordmark {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 16px;
    color: rgba(255, 255, 255, 0.6);
    text-decoration: none;
    letter-spacing: 0.02em;
    transition: color 0.2s;
  }
  .wordmark:hover { color: rgba(255, 255, 255, 0.88); }

  .topbar-label {
    font-size: 9px;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    color: rgba(255, 255, 255, 0.25);
  }

  /* ── Stage ──────────────────────────────────────────────────────────────── */
  .stage {
    position: relative;
    z-index: 1;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 72px 20px 80px;
    width: 100%;
    box-sizing: border-box;
    gap: 14px;
  }

  /* ── Banners ────────────────────────────────────────────────────────────── */
  .banner {
    width: min(480px, calc(100vw - 40px));
    padding: 11px 14px;
    border-radius: 9px;
    font-size: 10px;
    letter-spacing: 0.04em;
    display: flex;
    align-items: center;
    gap: 10px;
    backdrop-filter: blur(12px);
  }
  .banner-success {
    background: rgba(82, 171, 125, 0.1);
    border: 0.5px solid rgba(147, 230, 187, 0.3);
    color: rgba(191, 245, 216, 0.88);
  }
  .banner-info {
    background: rgba(255, 255, 255, 0.04);
    border: 0.5px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.5);
  }
  .banner-icon { font-size: 11px; }
  .banner span { flex: 1; }
  .banner-dismiss {
    background: none;
    border: none;
    color: inherit;
    opacity: 0.5;
    cursor: pointer;
    font-size: 11px;
    padding: 0;
    font-family: inherit;
  }
  .banner-dismiss:hover { opacity: 1; }

  /* ── Card ───────────────────────────────────────────────────────────────── */
  .card {
    width: min(480px, calc(100vw - 40px));
    background: rgba(10, 11, 14, 0.88);
    border: 0.5px solid rgba(255, 255, 255, 0.09);
    border-radius: 18px;
    padding: 32px 28px;
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* ── Icosahedron orb canvas ─────────────────────────────────────────────── */
  .orb-canvas {
    display: block;
    width: 200px;
    height: 200px;
    margin: 0 auto 8px;
    /* no border, no background — canvas draws everything */
  }

  /* ── Card text ──────────────────────────────────────────────────────────── */
  .card-title {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 22px;
    color: rgba(255, 255, 255, 0.82);
    margin: 0;
    text-align: center;
    line-height: 1.3;
  }

  .card-body {
    font-size: 10px;
    line-height: 1.65;
    color: rgba(255, 255, 255, 0.45);
    margin: 0;
    text-align: center;
  }

  .hint {
    font-size: 9px;
    line-height: 1.5;
    color: rgba(255, 255, 255, 0.28);
    text-align: center;
    margin: 0;
  }

  .inline-error {
    font-size: 9px;
    color: rgba(233, 148, 58, 0.85);
    margin: 0;
    text-align: center;
    line-height: 1.5;
  }

  /* ── Divider ────────────────────────────────────────────────────────────── */
  .divider {
    border: none;
    border-top: 0.5px solid rgba(255, 255, 255, 0.07);
    margin: 2px 0;
  }

  /* ── Identity row ───────────────────────────────────────────────────────── */
  .identity-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .identity-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .identity-handle {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.65);
    letter-spacing: 0.04em;
  }

  .identity-since {
    font-size: 9px;
    color: rgba(255, 255, 255, 0.28);
    letter-spacing: 0.04em;
  }

  .tier-badge {
    font-size: 8px;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    padding: 3px 7px;
    border-radius: 5px;
    border: 0.5px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.45);
    background: rgba(255, 255, 255, 0.04);
    white-space: nowrap;
  }

  .tier-badge.resonant {
    border-color: rgba(91, 155, 213, 0.4);
    color: rgba(167, 209, 245, 0.9);
    background: rgba(91, 155, 213, 0.08);
  }

  .tier-badge.soulful {
    border-color: rgba(190, 130, 230, 0.4);
    color: rgba(220, 180, 250, 0.9);
    background: rgba(160, 90, 220, 0.08);
  }

  /* ── Tier plan cards ────────────────────────────────────────────────────── */
  .tier-cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .tier-card {
    background: rgba(255, 255, 255, 0.025);
    border: 0.5px solid rgba(255, 255, 255, 0.07);
    border-radius: 12px;
    padding: 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tier-card-single {
    border: 0.5px solid rgba(255, 255, 255, 0.07);
    border-radius: 12px;
    padding: 14px 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .tier-card-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
  }

  .tier-card-name {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 15px;
    color: rgba(255, 255, 255, 0.72);
  }

  .tier-card-price {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.55);
    letter-spacing: 0.02em;
  }

  .tier-card-period {
    font-size: 9px;
    color: rgba(255, 255, 255, 0.32);
  }

  .upgrade-section-compact { gap: 6px; }

  /* ── Upgrade section ────────────────────────────────────────────────────── */
  .upgrade-section {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .upgrade-title {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 17px;
    color: rgba(255, 255, 255, 0.75);
    margin: 0;
  }

  .upgrade-body {
    font-size: 10px;
    line-height: 1.65;
    color: rgba(255, 255, 255, 0.42);
    margin: 0;
  }

  .upgrade-features {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .upgrade-features li {
    font-size: 10px;
    color: rgba(255, 255, 255, 0.52);
    display: flex;
    align-items: center;
    gap: 9px;
    letter-spacing: 0.02em;
  }

  .feat-dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .feat-stability { background: rgba(82, 130, 230, 0.7); }
  .feat-logic     { background: rgba(147, 230, 187, 0.7); }
  .feat-autonomy  { background: rgba(230, 200, 82, 0.7); }

  /* ── Subscriber perks ───────────────────────────────────────────────────── */
  .perks-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .perks-title {
    font-family: 'Fraunces', Georgia, serif;
    font-weight: 300;
    font-style: italic;
    font-size: 16px;
    color: rgba(255, 255, 255, 0.72);
    margin: 0;
  }

  .perks-title.resonant { color: rgba(167, 209, 245, 0.82); }
  .perks-title.soulful  { color: rgba(220, 180, 250, 0.82); }

  .perks-body {
    font-size: 10px;
    line-height: 1.65;
    color: rgba(255, 255, 255, 0.4);
    margin: 0;
  }

  .email-hint {
    color: rgba(255, 255, 255, 0.55);
  }

  /* ── Footer actions ─────────────────────────────────────────────────────── */
  .footer-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  /* ── Buttons ────────────────────────────────────────────────────────────── */
  .action-btn {
    font-family: 'Departure Mono', 'Courier New', monospace;
    font-size: 10px;
    letter-spacing: 0.1em;
    padding: 9px 18px;
    border-radius: 7px;
    cursor: pointer;
    text-decoration: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: all 0.18s;
    border: 0.5px solid transparent;
  }

  .action-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .action-btn.primary {
    width: 100%;
    background: rgba(255, 255, 255, 0.07);
    border-color: rgba(255, 255, 255, 0.18);
    color: rgba(255, 255, 255, 0.82);
  }
  .action-btn.primary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.32);
  }

  .action-btn.upgrade {
    width: 100%;
    background: rgba(82, 171, 125, 0.1);
    border-color: rgba(147, 230, 187, 0.3);
    color: rgba(191, 245, 216, 0.88);
  }
  .action-btn.upgrade:hover:not(:disabled) {
    background: rgba(82, 171, 125, 0.18);
    border-color: rgba(147, 230, 187, 0.5);
  }

  .action-btn.upgrade-soulful {
    width: 100%;
    background: rgba(160, 90, 220, 0.1);
    border-color: rgba(190, 130, 230, 0.3);
    color: rgba(220, 180, 250, 0.88);
  }
  .action-btn.upgrade-soulful:hover:not(:disabled) {
    background: rgba(160, 90, 220, 0.18);
    border-color: rgba(190, 130, 230, 0.5);
  }

  .action-btn.secondary {
    flex: 1;
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.55);
  }
  .action-btn.secondary:hover {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.78);
  }

  .action-btn.ghost {
    background: transparent;
    border-color: transparent;
    color: rgba(255, 255, 255, 0.28);
    padding: 9px 10px;
    font-size: 9px;
  }
  .action-btn.ghost:hover:not(:disabled) {
    color: rgba(255, 255, 255, 0.55);
  }

  .action-btn.ghost-small {
    background: transparent;
    border-color: transparent;
    color: rgba(255, 255, 255, 0.32);
    padding: 4px 0;
    font-size: 9px;
    letter-spacing: 0.08em;
  }
  .action-btn.ghost-small:hover:not(:disabled) {
    color: rgba(255, 255, 255, 0.6);
  }

  /* ── Page footer ────────────────────────────────────────────────────────── */
  .page-footer {
    position: relative;
    z-index: 1;
    padding: 16px 28px;
    font-size: 9px;
    letter-spacing: 0.1em;
    color: rgba(255, 255, 255, 0.18);
    text-align: center;
  }

  .footer-link {
    color: rgba(255, 255, 255, 0.3);
    text-decoration: none;
    transition: color 0.2s;
  }
  .footer-link:hover { color: rgba(255, 255, 255, 0.6); }

  /* ── Mobile ─────────────────────────────────────────────────────────────── */
  @media (max-width: 520px) {
    .topbar { padding: 0 18px; }
    .card { padding: 22px 18px; }
    .stage { padding: 68px 16px 70px; }
    .tier-cards { grid-template-columns: 1fr; }
  }
</style>
