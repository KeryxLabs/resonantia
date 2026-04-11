<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { avecToRgb, AVEC_COLORS } from '@resonantia/core';
  import type { AvecState } from '@resonantia/core';

  export let avec: AvecState = {
    stability: 0.95,
    friction:  0.19,
    logic:     0.81,
    autonomy:  0.99,
    psi:       2.94,
  };
  export let size = 160;
  export let showStreams = true;

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D;
  let raf: number;
  let t = 0;

  interface Stream {
    side: number;
    offset: number;
    col: { r: number; g: number; b: number };
    destCol: { r: number; g: number; b: number };
    thickness: number;
    phase: number;
    speed: number;
  }

  let streams: Stream[] = [];

  const ICO_VERTS = (() => {
    const phi = (1 + Math.sqrt(5)) / 2;
    const raw = [
      [-1,phi,0],[1,phi,0],[-1,-phi,0],[1,-phi,0],
      [0,-1,phi],[0,1,phi],[0,-1,-phi],[0,1,-phi],
      [phi,0,-1],[phi,0,1],[-phi,0,-1],[-phi,0,1],
    ];
    return raw.map(([x, y, z]) => {
      const n = Math.sqrt(x*x + y*y + z*z);
      return [x/n, y/n, z/n] as [number, number, number];
    });
  })();

  const ICO_FACES = [
    [0,11,5],[0,5,1],[0,1,7],[0,7,10],[0,10,11],
    [1,5,9],[5,11,4],[11,10,2],[10,7,6],[7,1,8],
    [3,9,4],[3,4,2],[3,2,6],[3,6,8],[3,8,9],
    [4,9,5],[2,4,11],[6,2,10],[8,6,7],[9,8,1],
  ];

  function buildStreams(av: AvecState) {
    const ordered = Object.entries({
      stability: av.stability,
      friction:  av.friction,
      logic:     av.logic,
      autonomy:  av.autonomy,
    }).sort((a, b) => b[1] - a[1]) as [keyof typeof AVEC_COLORS, number][];

    streams = [];
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
          thickness: weight * 2.5 * (0.6 + Math.random() * 0.4),
          phase: Math.random() * Math.PI * 2,
          speed: 0.4 + Math.random() * 0.3,
        });
      }
    }
  }

  function project(v: [number, number, number], rx: number, ry: number, rz: number, scale: number) {
    let [x, y, z] = v;
    let ry1 = y * Math.cos(rx) - z * Math.sin(rx);
    let rz1 = y * Math.sin(rx) + z * Math.cos(rx);
    let rx2 = x * Math.cos(ry) + rz1 * Math.sin(ry);
    let rz2 = -x * Math.sin(ry) + rz1 * Math.cos(ry);
    let rx3 = rx2 * Math.cos(rz) - ry1 * Math.sin(rz);
    let ry3 = rx2 * Math.sin(rz) + ry1 * Math.cos(rz);
    const dist = 3;
    const fov = dist / (dist + rz2);
    const cx = size / 2, cy = size / 2;
    return { x: cx + rx3 * scale * fov, y: cy + ry3 * scale * fov, z: rz2 };
  }

  function getFaceColor(faceIdx: number, av: AvecState, brightness: number): string {
    const dims = [av.stability, av.friction, av.logic, av.autonomy];
    const cols = [AVEC_COLORS.stability, AVEC_COLORS.friction, AVEC_COLORS.logic, AVEC_COLORS.autonomy];
    const di = faceIdx % 4;
    const col = cols[di];
    const alpha = Math.min(brightness * dims[di] * 0.6 + brightness * 0.3, 0.85);
    return `rgba(${col.r},${col.g},${col.b},${alpha})`;
  }

  function drawFrame() {
    if (!ctx) return;
    ctx.clearRect(0, 0, size, size);

    const cx = size / 2, cy = size / 2;
    const av = avec;
    const mixed = avecToRgb(av);
    const psiVal = av.stability + av.friction + av.logic + av.autonomy;
    const pulseRate = 0.3 + (1 - psiVal / 4) * 1.2;
    const pulseAmt = 0.04 + (1 - psiVal / 4) * 0.08;
    const baseRadius = size * 0.27;
    const scale = baseRadius * (1 + Math.sin(t * pulseRate) * pulseAmt);

    const rot = { x: t * 0.17, y: t * 0.23, z: t * 0.07 };

    if (showStreams) drawStreams(av, mixed, scale, cx, cy);

    const glowR = scale * 0.7;
    const glowGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, glowR * 1.6);
    const ga = 0.12 + Math.sin(t * pulseRate) * 0.04;
    glowGrad.addColorStop(0, `rgba(${mixed.r|0},${mixed.g|0},${mixed.b|0},${ga * 2})`);
    glowGrad.addColorStop(0.5, `rgba(${mixed.r|0},${mixed.g|0},${mixed.b|0},${ga})`);
    glowGrad.addColorStop(1, `rgba(${mixed.r|0},${mixed.g|0},${mixed.b|0},0)`);
    ctx.fillStyle = glowGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, glowR * 1.6, 0, Math.PI * 2);
    ctx.fill();

    const projected = ICO_VERTS.map(v =>
      project(v as [number,number,number], rot.x, rot.y, rot.z, scale)
    );

    ICO_FACES
      .map((f, i) => ({ f, i, z: (projected[f[0]].z + projected[f[1]].z + projected[f[2]].z) / 3 }))
      .sort((a, b) => a.z - b.z)
      .forEach(({ f, i, z }) => {
        const [a, b, c] = f.map(vi => projected[vi]);
        const normZ = (z + 1) / 2;
        ctx.beginPath();
        ctx.moveTo(a.x, a.y);
        ctx.lineTo(b.x, b.y);
        ctx.lineTo(c.x, c.y);
        ctx.closePath();
        ctx.fillStyle = getFaceColor(i, av, 0.15 + normZ * 0.4);
        ctx.fill();
        ctx.strokeStyle = `rgba(255,255,255,${0.06 + normZ * 0.1})`;
        ctx.lineWidth = 0.5;
        ctx.stroke();
      });

    const coreR = scale * 0.22;
    const coreGrad = ctx.createRadialGradient(cx - coreR*0.3, cy - coreR*0.3, 0, cx, cy, coreR);
    const ca = 0.7 + Math.sin(t * pulseRate * 2) * 0.15;
    coreGrad.addColorStop(0, `rgba(255,255,255,${ca})`);
    coreGrad.addColorStop(0.4, `rgba(${mixed.r|0},${mixed.g|0},${mixed.b|0},${ca * 0.8})`);
    coreGrad.addColorStop(1, `rgba(${mixed.r|0},${mixed.g|0},${mixed.b|0},0)`);
    ctx.fillStyle = coreGrad;
    ctx.beginPath();
    ctx.arc(cx, cy, coreR, 0, Math.PI * 2);
    ctx.fill();

    t += 0.008;
    raf = requestAnimationFrame(drawFrame);
  }

  function drawStreams(
    av: AvecState,
    mixed: { r: number; g: number; b: number },
    scale: number,
    cx: number,
    cy: number
  ) {
    streams.forEach(s => {
      const startX = s.side < 0 ? -size * 0.1 : size * 1.1;
      const endX   = s.side < 0 ? cx - scale * 0.85 : cx + scale * 0.85;
      const midX   = (startX + endX) / 2;
      const baseY  = cy + s.offset;
      const wave   = Math.sin(t * s.speed + s.phase) * (12 + Math.abs(s.offset) * 0.5);
      const cp1x = midX * 0.5, cp1y = baseY + wave;
      const cp2x = endX * 0.7, cp2y = baseY - wave * 0.5;

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
      ctx.bezierCurveTo(cp1x, cp1y, cp2x, cp2y, endX, baseY + s.offset * 0.1);
      ctx.strokeStyle = grad;
      ctx.lineWidth = s.thickness;
      ctx.lineCap = 'round';
      ctx.stroke();
    });
  }

  $: if (avec) buildStreams(avec);

  onMount(() => {
    ctx = canvas.getContext('2d')!;
    buildStreams(avec);
    drawFrame();
  });

  onDestroy(() => cancelAnimationFrame(raf));
</script>

<canvas
  bind:this={canvas}
  width={size}
  height={size}
  style="width:{size}px;height:{size}px;display:block;"
></canvas>
