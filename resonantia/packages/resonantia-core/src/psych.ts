import type { AvecState, NodeDto } from "./types";

export type PsychAxis = { name: string; value: number };

export type PsychLayer = {
  axes: PsychAxis[];
  archetype: string;
  reflection: string;
  nextNudge: string;
  scopeNote: string;
  orbClass: string;
  orbHue: number;
  orbEnergy: number;
  orbPulse: number;
  radarPoints: string;
  coherence: number;
  orbCaption: string;
};

function polarPoint(index: number, value: number, radius: number): { x: number; y: number } {
  const angle = (-90 + index * 72) * (Math.PI / 180);
  const r = radius * Math.min(1, Math.max(0, value));
  return { x: 110 + Math.cos(angle) * r, y: 110 + Math.sin(angle) * r };
}

function fmt(n: number): string {
  return n.toFixed(2).replace(/\.?0+$/, "");
}

function buildRadarDataPoints(axes: PsychAxis[]): string {
  return axes.map((a, i) => {
    const p = polarPoint(i, a.value, 76);
    return `${fmt(p.x)},${fmt(p.y)}`;
  }).join(" ");
}

export function buildRadarRingPoints(radius: number): string {
  return Array.from({ length: 5 }, (_, i) => {
    const p = polarPoint(i, 1, radius);
    return `${fmt(p.x)},${fmt(p.y)}`;
  }).join(" ");
}

export function getOrbStyle(psych: PsychLayer): string {
  return `--orb-hue:${Math.round(psych.orbHue)}; --orb-energy:${psych.orbEnergy.toFixed(2)}; --orb-pulse:${psych.orbPulse.toFixed(2)};`;
}

export function getLegendFillStyle(value: number): string {
  return `--axis-fill:${clamp01(value).toFixed(2)};`;
}

export type DeltaStory = {
  headline: string;
  supportingLine: string;
};

export type RecommendedAction = {
  action: string;
  why: string;
};

export type MoodTimelineItem = {
  index: number;
  isCurrent: boolean;
  hue: number;
  alpha: number;
  tooltip: string;
};

const MOOD_TIMELINE_WINDOW = 12;

export type FutureSelfPreview = {
  title: string;
  likelyPath: string;
  shiftPath: string;
};

export type InnerDialogue = {
  analyst: string;
  human: string;
  synthesis: string;
};

export type LifeContextTag = {
  label: string;
  hue: number;
};

export type LifeContextTags = {
  tags: LifeContextTag[];
  moodNote: string;
};

export type BlindSpotLens = {
  label: string;
  observation: string;
  nudge: string;
};

export type GentleCelebration = {
  show: boolean;
  glyph: string;
  title: string;
  body: string;
};

export type JournalExportEntry = {
  lines: string[];
  fullText: string;
};

const clamp01 = (value: number): number => Math.min(1, Math.max(0, value));

function resolveState(node: NodeDto): AvecState {
  if (node.compressionAvec) {
    return node.compressionAvec;
  }

  return {
    stability: (node.userAvec.stability + node.modelAvec.stability) / 2,
    friction: (node.userAvec.friction + node.modelAvec.friction) / 2,
    logic: (node.userAvec.logic + node.modelAvec.logic) / 2,
    autonomy: (node.userAvec.autonomy + node.modelAvec.autonomy) / 2,
    psi: (node.userAvec.psi + node.modelAvec.psi) / 2
  };
}

function resolveArchetype(
  curiosity: number,
  discipline: number,
  socialEnergy: number,
  flexibility: number,
  stressLoad: number
): string {
  if (stressLoad >= 0.74) return "Pressure Navigator";
  if (discipline >= 0.72 && curiosity >= 0.65) return "Precision Explorer";
  if (socialEnergy >= 0.68 && flexibility >= 0.6) return "Bridge Builder";
  if (curiosity >= 0.72 && flexibility >= 0.62) return "Pattern Diver";
  return "Steady Integrator";
}

export function buildPsychLayer(node: NodeDto): PsychLayer {
  const state = resolveState(node);
  const curiosity = clamp01(state.logic * 0.58 + state.autonomy * 0.42);
  const discipline = clamp01(state.stability * 0.64 + (1 - state.friction) * 0.36);
  const socialEnergy = clamp01(state.autonomy * 0.56 + (1 - state.friction) * 0.44);
  const flexibility = clamp01((1 - state.stability) * 0.34 + state.logic * 0.36 + state.autonomy * 0.3);
  const stressLoad = clamp01(state.friction * 0.74 + (1 - state.stability) * 0.26);

  const axes: PsychAxis[] = [
    { name: "Curiosity", value: curiosity },
    { name: "Discipline", value: discipline },
    { name: "Social Energy", value: socialEnergy },
    { name: "Flexibility", value: flexibility },
    { name: "Stress Load", value: stressLoad }
  ];

  const coherence = (curiosity + discipline + socialEnergy + flexibility + (1 - stressLoad)) / 5;
  const leadAxis = [...axes].sort((a, b) => b.value - a.value)[0].name;
  const quietAxis = [...axes].sort((a, b) => a.value - b.value)[0].name;
  const archetype = resolveArchetype(curiosity, discipline, socialEnergy, flexibility, stressLoad);

  const reflection = `Right now, ${leadAxis.toLowerCase()} is doing more of the work while ${quietAxis.toLowerCase()} is taking a back seat.`;

  const nextNudge =
    stressLoad >= 0.68
      ? "Make things lighter first, then choose one small thing you can actually finish."
      : coherence >= 0.66
        ? "Use this steadier moment to lock in one real next step before it slips away."
        : "Help the quieter part of this moment with one tiny, easy step.";

  const orbClass =
    stressLoad >= 0.68
      ? "orb-alert"
      : discipline >= 0.7 && curiosity >= 0.62
        ? "orb-focused"
        : socialEnergy >= 0.65 && flexibility >= 0.58
          ? "orb-connective"
          : "orb-balanced";

  const calmScore = clamp01(discipline * 0.38 + socialEnergy * 0.28 + (1 - stressLoad) * 0.34);
  const orbHue = Math.min(205, Math.max(12, 8 + calmScore * 150 + curiosity * 28 - stressLoad * 20));
  const orbEnergy = clamp01((curiosity + socialEnergy + flexibility) / 3);
  const orbPulse = Math.min(1.2, Math.max(0.35, 0.35 + stressLoad * 0.9));

  return {
    axes,
    archetype,
    reflection,
    nextNudge,
    scopeNote: "This is a read of this moment, not a definition of who you are.",
    orbClass,
    orbHue,
    orbEnergy,
    orbPulse,
    radarPoints: buildRadarDataPoints(axes),
    coherence,
    orbCaption: `${archetype} · ${Math.round(coherence * 100)}% aligned`
  };
}

export function buildDeltaStory(nodes: NodeDto[], currentIndex: number): DeltaStory {
  if (nodes.length < 2 || currentIndex >= nodes.length - 1) {
    return {
      headline: "This is the newest moment in this session.",
      supportingLine: "Add another moment later and this space will show what shifted."
    };
  }

  const current = resolveState(nodes[currentIndex]);
  const previous = resolveState(nodes[currentIndex + 1]);

  const deltas: Array<{ name: string; value: number }> = [
    {
      name: "curiosity",
      value: current.logic * 0.58 + current.autonomy * 0.42 - (previous.logic * 0.58 + previous.autonomy * 0.42)
    },
    {
      name: "discipline",
      value:
        current.stability * 0.64 + (1 - current.friction) * 0.36 -
        (previous.stability * 0.64 + (1 - previous.friction) * 0.36)
    },
    {
      name: "social energy",
      value:
        current.autonomy * 0.56 + (1 - current.friction) * 0.44 -
        (previous.autonomy * 0.56 + (1 - previous.friction) * 0.44)
    },
    {
      name: "flexibility",
      value:
        (1 - current.stability) * 0.34 + current.logic * 0.36 + current.autonomy * 0.3 -
        ((1 - previous.stability) * 0.34 + previous.logic * 0.36 + previous.autonomy * 0.3)
    },
    {
      name: "stress load",
      value:
        current.friction * 0.74 + (1 - current.stability) * 0.26 -
        (previous.friction * 0.74 + (1 - previous.stability) * 0.26)
    }
  ];

  const strongestUp = [...deltas].sort((a, b) => b.value - a.value)[0];
  const strongestDown = [...deltas].sort((a, b) => a.value - b.value)[0];

  const upText = strongestUp.value > 0.02 ? `${strongestUp.name} rose ${Math.round(Math.abs(strongestUp.value) * 100)}%` : null;
  const downText =
    strongestDown.value < -0.02 ? `${strongestDown.name} dropped ${Math.round(Math.abs(strongestDown.value) * 100)}%` : null;

  const headline =
    upText && downText
      ? `${upText}, while ${downText}.`
      : upText
        ? `${upText}.`
        : downText
          ? `${downText}.`
          : "Not much moved between these last two moments.";

  return {
    headline,
    supportingLine:
      upText || downText ? "Treat this as direction, not a verdict on you." : "Small changes are normal. Patterns get clearer over time."
  };
}

export function buildAdaptivePrompts(psych: PsychLayer, delta: DeltaStory): string[] {
  const prompts: string[] = [];

  if (psych.orbClass === "orb-alert") {
    prompts.push("What can I safely remove from my plate in the next 10 minutes?");
  } else if (psych.orbClass === "orb-focused") {
    prompts.push("What single commitment will convert this focus into visible progress?");
  } else if (psych.orbClass === "orb-connective") {
    prompts.push("Who could make this easier if I reached out right now?");
  } else {
    prompts.push("What one tiny step would keep this balanced state alive?");
  }

  prompts.push(
    delta.headline.includes("rose")
      ? "How can I channel this rising energy without overloading myself?"
      : "What support would help me lift the quieter part of my state?"
  );

  prompts.push("If tomorrow felt 15% better, what would I do differently first?");
  return prompts;
}

export function buildMoodTimeline(nodes: NodeDto[], currentIndex: number): MoodTimelineItem[] {
  if (nodes.length === 0) return [];

  const safeCurrentIndex = Math.max(0, Math.min(currentIndex, nodes.length - 1));
  const windowSize = Math.min(MOOD_TIMELINE_WINDOW, nodes.length);
  const halfWindow = Math.floor(windowSize / 2);
  const start = Math.max(0, Math.min(safeCurrentIndex - halfWindow, nodes.length - windowSize));

  return nodes.slice(start, start + windowSize).map((node, offset) => {
    const index = start + offset;
    const psych = buildPsychLayer(node);
    return {
      index,
      isCurrent: index === safeCurrentIndex,
      hue: 18 + psych.axes[0].value * 54 + psych.axes[2].value * 22 - psych.axes[4].value * 16,
      alpha: Math.min(0.94, Math.max(0.38, 0.42 + psych.coherence * 0.5)),
      tooltip: `Moment ${index + 1} · ${new Date(node.timestamp).toLocaleString()} - ${psych.archetype}`
    };
  });
}

export function buildFutureSelfPreview(psych: PsychLayer, delta: DeltaStory): FutureSelfPreview {
  const title =
    psych.orbClass === "orb-alert"
      ? "If today keeps feeling this heavy"
      : psych.orbClass === "orb-focused"
        ? "If this focus keeps holding"
        : psych.orbClass === "orb-connective"
          ? "If this openness keeps holding"
          : "If this steadiness keeps holding";

  const likelyPath =
    psych.orbClass === "orb-alert"
      ? "The day may start to feel crowded, and choices may get harder."
      : psych.orbClass === "orb-focused"
        ? "You are likely to get something meaningful across the line."
        : psych.orbClass === "orb-connective"
          ? "Talking it through may unlock momentum faster than pushing alone."
          : "You are likely to keep moving with less friction than usual.";

  const shiftPath =
    psych.orbClass === "orb-alert"
      ? "One small shift: remove one obligation and protect a short pocket of recovery."
      : delta.headline.includes("dropped")
        ? "One small shift: support what dipped with one tiny, concrete action."
        : "One small shift: decide the next step now while this moment is still with you.";

  return { title, likelyPath, shiftPath };
}

export function buildInnerDialogue(psych: PsychLayer, delta: DeltaStory): InnerDialogue {
  const analyst =
    psych.orbClass === "orb-alert"
      ? "This looks like a heavy moment. Make it simpler before you ask more from yourself."
      : psych.orbClass === "orb-focused"
        ? "This looks like a good window for finishing one important thing."
        : psych.orbClass === "orb-connective"
          ? "This looks like a moment where another person could help unlock progress."
          : "This looks steady. Keep your rhythm and do not overcomplicate it.";

  const human =
    psych.orbClass === "orb-alert"
      ? "I want relief and permission to make this simpler."
      : psych.orbClass === "orb-focused"
        ? "I want to finish something meaningful and feel proud of it."
        : psych.orbClass === "orb-connective"
          ? "I want support and momentum that does not feel lonely."
          : "I want progress that feels calm, not chaotic.";

  const synthesis = delta.headline.includes("stable")
    ? "Together: protect this steadiness by choosing one visible win and stopping on purpose."
    : "Together: choose one action that feels both useful and kind to yourself.";

  return { analyst, human, synthesis };
}

export function buildLifeContextTags(psych: PsychLayer): LifeContextTags {
  const curiosity = psych.axes[0].value;
  const discipline = psych.axes[1].value;
  const socialEnergy = psych.axes[2].value;
  const flexibility = psych.axes[3].value;
  const stressLoad = psych.axes[4].value;

  const tags: LifeContextTag[] = [];

  if (stressLoad >= 0.68) tags.push({ label: "Recovery Mode", hue: 0 });

  if (discipline >= 0.7 && stressLoad < 0.55) {
    tags.push({ label: "Deep Focus", hue: 220 });
  } else if (discipline >= 0.65 && stressLoad >= 0.55) {
    tags.push({ label: "Work Sprint", hue: 270 });
  }

  if (curiosity >= 0.68 && flexibility >= 0.58) {
    tags.push({ label: "Creative Flow", hue: 38 });
  } else if (curiosity >= 0.62 && discipline < 0.6) {
    tags.push({ label: "Exploration Phase", hue: 160 });
  }

  if (socialEnergy >= 0.66) tags.push({ label: "Social Push", hue: 145 });

  if (flexibility >= 0.62 && stressLoad < 0.48 && discipline >= 0.54 && !tags.some((t) => t.label === "Deep Focus")) {
    tags.push({ label: "Planning Mode", hue: 200 });
  }

  if (tags.length === 0) tags.push({ label: "Steady State", hue: 215 });

  let moodNote = "Steady momentum - the state is sustainably balanced.";
  if (tags.length === 1) {
    moodNote =
      tags[0].label === "Recovery Mode"
        ? "Load is high - protect your energy and simplify the next move."
        : tags[0].label === "Deep Focus"
          ? "A clear execution window is open right now."
          : tags[0].label === "Work Sprint"
            ? "Pushing through with pressure behind you."
            : tags[0].label === "Creative Flow"
              ? "Curiosity and flexibility are converging - follow it."
              : tags[0].label === "Exploration Phase"
                ? "Curiosity is leading. Good moment to follow without forcing structure."
                : tags[0].label === "Social Push"
                  ? "Connection energy is high. Good time to involve someone."
                  : tags[0].label === "Planning Mode"
                    ? "Adaptive structure available - map before you move."
                    : "Steady momentum - the state is sustainably balanced.";
  } else if (tags.length === 2) {
    moodNote = `Blending ${tags[0].label.toLowerCase()} with ${tags[1].label.toLowerCase()}.`;
  } else {
    moodNote = `Complex state: ${tags
      .slice(0, 3)
      .map((t) => t.label.toLowerCase())
      .join(", ")}.`;
  }

  return { tags, moodNote };
}

export function buildBlindSpotLens(psych: PsychLayer): BlindSpotLens {
  const curiosity = psych.axes[0].value;
  const discipline = psych.axes[1].value;
  const socialEnergy = psych.axes[2].value;
  const flexibility = psych.axes[3].value;
  const stressLoad = psych.axes[4].value;

  const opportunities: Array<{ name: string; value: number }> = [
    { name: "curiosity", value: 1 - curiosity },
    { name: "discipline", value: 1 - discipline },
    { name: "social energy", value: 1 - socialEnergy },
    { name: "flexibility", value: 1 - flexibility },
    { name: "stress load", value: stressLoad }
  ];

  const strongest = [...opportunities].sort((a, b) => b.value - a.value)[0];
  if (strongest.value < 0.18) {
    return {
      label: "Coherence Balance",
      observation: "Your state is unusually aligned - which makes it easy to drift without noticing.",
      nudge: "Name one priority so this steady moment turns into progress, not drift."
    };
  }

  if (strongest.name === "curiosity") {
    return {
      label: "Curiosity Gap",
      observation: "You may be solving the known problem instead of questioning whether it is the right one.",
      nudge: "Ask one question you have not asked yet about this situation."
    };
  }

  if (strongest.name === "discipline") {
    return {
      label: "Commitment Gap",
      observation: "You may be circling without committing. One locked action beats ten open loops.",
      nudge: "Name one specific thing you can finish and hold yourself to."
    };
  }

  if (strongest.name === "social energy") {
    return {
      label: "Connection Gap",
      observation: "You may be going it alone when a conversation would accelerate the work.",
      nudge: "Share one update or ask one question that could help move this forward."
    };
  }

  if (strongest.name === "flexibility") {
    return {
      label: "Flexibility Gap",
      observation: "You may be attached to a path that has already shifted beneath you.",
      nudge: "Name one assumption you may be protecting and test it quickly."
    };
  }

  return {
    label: "Stress Buildup",
    observation: "You may be carrying more than you are fully noticing.",
    nudge: "Before you add more, finish or drop one thing from your list."
  };
}

export function buildGentleCelebration(nodes: NodeDto[], psych: PsychLayer, delta: DeltaStory): GentleCelebration {
  const curiosity = psych.axes[0].value;
  const discipline = psych.axes[1].value;
  const stressLoad = psych.axes[4].value;

  if (nodes.length >= 8) {
    return {
      show: true,
      glyph: "INF",
      title: "Deep Continuity",
      body: `This session has ${nodes.length} moments - a rare depth of tracking. That's worth acknowledging.`
    };
  }

  if (psych.coherence >= 0.78) {
    return {
      show: true,
      glyph: "O",
      title: "High Coherence",
      body: "Your state is unusually aligned right now. Most people never notice moments like this. You did."
    };
  }

  if (delta.headline.includes("stress load") && delta.headline.includes("dropped")) {
    return {
      show: true,
      glyph: "V",
      title: "Pressure Released",
      body: "Your stress signal dropped between moments. That shift took something. It counts."
    };
  }

  if (discipline >= 0.74 && stressLoad < 0.4) {
    return {
      show: true,
      glyph: "HEX",
      title: "Clear Execution Window",
      body: "High discipline and low pressure rarely show up together. This is a good moment to do the hard thing."
    };
  }

  if (curiosity >= 0.76) {
    return {
      show: true,
      glyph: "STAR",
      title: "Curiosity Peak",
      body: "Your curiosity signal is at a high. Ideas are more accessible now than they usually are."
    };
  }

  return { show: false, glyph: "", title: "", body: "" };
}

export function buildJournalExport(
  node: NodeDto,
  psych: PsychLayer,
  delta: DeltaStory,
  contextTags: LifeContextTags,
  blindSpot: BlindSpotLens
): JournalExportEntry {
  const date = new Date(node.timestamp);
  const dateLine = `${date.toISOString().slice(0, 10)} - ${node.sessionId}`;
  const stateLine = `Moment: ${psych.archetype}`;
  const contextLine =
    contextTags.tags.length > 0
      ? `What stands out: ${contextTags.tags.map((t) => t.label).join(", ")}`
      : "What stands out: Steady State";
  const deltaLine = `What changed: ${delta.headline}`;
  const blindSpotLine = `What to watch: ${blindSpot.label} - ${blindSpot.nudge}`;

  return {
    lines: [dateLine, stateLine, contextLine],
    fullText: [dateLine, stateLine, contextLine, deltaLine, blindSpotLine].join("\n")
  };
}

export function getContextTagStyle(tag: LifeContextTag): string {
  return `--tag-hue:${tag.hue.toFixed(0)};`;
}

export function buildRecommendedAction(
  psych: PsychLayer,
  delta: DeltaStory,
  blindSpot?: BlindSpotLens,
  contextTags?: LifeContextTags
): RecommendedAction {
  const action =
    psych.orbClass === "orb-alert"
      ? "Make the day lighter: drop, delay, or hand off one thing."
      : psych.orbClass === "orb-focused"
        ? "Pick one meaningful thing and finish it end to end."
        : psych.orbClass === "orb-connective"
          ? "Send one clear message that helps this move forward."
          : "Choose one visible next step and give it a clear finish line.";

  let why =
    delta.headline.includes("stable")
      ? "This helps this moment turn into progress instead of drift."
      : "This helps turn this shift into something useful.";

  if (blindSpot) {
    why = `This matters most right now because ${blindSpot.label.toLowerCase()} is where the most movement is available. ${why}`;
  }

  if (contextTags?.tags.some((t) => t.label === "Recovery Mode")) {
    why = "This feels like a recovery moment. Make it lighter before you make it bigger.";
  }

  return { action, why };
}
