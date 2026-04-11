import type { AvecState } from './types';

export const AVEC_COLORS = {
  stability: { r: 232, g: 232, b: 235 },
  friction:  { r: 233, g: 148, b:  58 },
  logic:     { r:  91, g: 155, b: 213 },
  autonomy:  { r: 122, g: 170, b: 122 },
} as const;

export const AVEC_HEX = {
  stability: '#e8e8eb',
  friction:  '#e9943a',
  logic:     '#5b9bd5',
  autonomy:  '#7aaa7a',
} as const;

export function avecToRgb(avec: AvecState): { r: number; g: number; b: number } {
  const weights = [avec.stability, avec.friction, avec.logic, avec.autonomy];
  const cols = [
    AVEC_COLORS.stability,
    AVEC_COLORS.friction,
    AVEC_COLORS.logic,
    AVEC_COLORS.autonomy,
  ];

  let r = 0, g = 0, b = 0, total = 0;
  weights.forEach((w, i) => {
    r += cols[i].r * w;
    g += cols[i].g * w;
    b += cols[i].b * w;
    total += w;
  });

  if (total === 0) return { r: 200, g: 200, b: 200 };
  return { r: r / total, g: g / total, b: b / total };
}

export function avecColor(avec: AvecState, alpha = 1): string {
  const { r, g, b } = avecToRgb(avec);
  return `rgba(${Math.round(r)},${Math.round(g)},${Math.round(b)},${alpha})`;
}

export function dominantDim(avec: AvecState): keyof typeof AVEC_COLORS {
  const dims: [keyof typeof AVEC_COLORS, number][] = [
    ['stability', avec.stability],
    ['friction',  avec.friction],
    ['logic',     avec.logic],
    ['autonomy',  avec.autonomy],
  ];
  return dims.sort((a, b) => b[1] - a[1])[0][0];
}

export function psi(avec: AvecState): number {
  return avec.stability + avec.friction + avec.logic + avec.autonomy;
}

export function formatTimestamp(iso: string): string {
  return new Date(iso).toLocaleString('en-US', {
    month: 'short',
    day:   'numeric',
    year:  'numeric',
    hour:  '2-digit',
    minute:'2-digit',
  });
}

export function shortLabel(label: string, words = 3): string {
  return label.replace(/-/g, '_').split('_').slice(0, words).join(' ');
}
