import type { ChatMessage, ModelProvider } from '@resonantia/core';
import type { ComposeTokenUsage } from './components/compose/types';

const FALLBACK_CHARS_PER_TOKEN = 4;
const CHAT_MESSAGE_OVERHEAD_TOKENS = 4;
const CHAT_REPLY_PRIMER_TOKENS = 2;

const MANAGED_GATEWAY_CONTEXT_LIMIT = 128_000;
const OLLAMA_DEFAULT_CONTEXT_LIMIT = 8_192;
const OPENAI_DEFAULT_CONTEXT_LIMIT = 128_000;

type ModelLimitRule = {
  pattern: RegExp;
  limit: number;
};

const OPENAI_MODEL_LIMIT_RULES: ModelLimitRule[] = [
  { pattern: /gpt-3\.5|gpt-35/i, limit: 16_384 },
  { pattern: /gpt-4-32k/i, limit: 32_768 },
  { pattern: /gpt-4\.1|gpt-4o|o1|o3|gpt-5|gpt-4-turbo|gpt-4/i, limit: 128_000 },
];

export type { ComposeTokenUsage };

type EncoderLike = {
  encode(text: string): number[];
};

let encoder: EncoderLike | null = null;
let encoderLoadPromise: Promise<void> | null = null;

export async function warmupComposeTokenizer(): Promise<void> {
  if (encoder !== null) {
    return;
  }

  if (encoderLoadPromise !== null) {
    return encoderLoadPromise;
  }

  encoderLoadPromise = (async () => {
    const [{ Tiktoken }, ranksModule] = await Promise.all([
      import('js-tiktoken/lite'),
      import('js-tiktoken/ranks/cl100k_base'),
    ]);

    const ranks = (ranksModule as { default: unknown }).default;
    encoder = new Tiktoken(ranks as ConstructorParameters<typeof Tiktoken>[0]);
  })().finally(() => {
    encoderLoadPromise = null;
  });

  return encoderLoadPromise;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function fallbackApproximation(text: string): number {
  if (!text.trim()) {
    return 0;
  }

  return Math.max(1, Math.ceil(text.length / FALLBACK_CHARS_PER_TOKEN));
}

function parseContextFromModelName(modelName: string): number | null {
  const trimmed = modelName.trim();
  if (!trimmed) {
    return null;
  }

  const kMatch = trimmed.match(/(?:^|[-_ ])(\d{1,3})k(?:$|[-_ ])/i);
  if (kMatch) {
    const parsed = Number.parseInt(kMatch[1], 10);
    if (Number.isFinite(parsed)) {
      return clamp(parsed * 1_000, 2_048, 262_144);
    }
  }

  const plainMatch = trimmed.match(/(?:ctx|context)[-_ ]?(\d{3,6})/i);
  if (plainMatch) {
    const parsed = Number.parseInt(plainMatch[1], 10);
    if (Number.isFinite(parsed)) {
      return clamp(parsed, 2_048, 262_144);
    }
  }

  return null;
}

function inferOpenAiContextWindow(modelName: string): number {
  const parsed = parseContextFromModelName(modelName);
  if (parsed !== null) {
    return parsed;
  }

  for (const rule of OPENAI_MODEL_LIMIT_RULES) {
    if (rule.pattern.test(modelName)) {
      return rule.limit;
    }
  }

  return OPENAI_DEFAULT_CONTEXT_LIMIT;
}

function inferOllamaContextWindow(modelName: string): number {
  const parsed = parseContextFromModelName(modelName);
  if (parsed !== null) {
    return parsed;
  }

  return OLLAMA_DEFAULT_CONTEXT_LIMIT;
}

function ratioToPercent(value: number, max: number): number {
  if (max <= 0) {
    return 0;
  }

  return clamp((value / max) * 100, 0, 100);
}

export function countTiktoken(text: string): number {
  const source = text ?? '';
  if (!source.trim()) {
    return 0;
  }

  if (encoder === null) {
    void warmupComposeTokenizer();
    return fallbackApproximation(source);
  }

  try {
    return encoder.encode(source).length;
  } catch {
    return fallbackApproximation(source);
  }
}

export function countChatMessageTokens(messages: ChatMessage[]): number {
  let total = CHAT_REPLY_PRIMER_TOKENS;

  for (const message of messages) {
    total += CHAT_MESSAGE_OVERHEAD_TOKENS;
    total += countTiktoken(message.role);
    total += countTiktoken(message.content);
  }

  return total;
}

export function inferComposeContextWindowTokens(
  modelProvider: ModelProvider,
  openaiModel: string,
  ollamaModel: string,
): number {
  if (modelProvider === 'managed-gateway') {
    return MANAGED_GATEWAY_CONTEXT_LIMIT;
  }

  if (modelProvider === 'openai-byo') {
    return inferOpenAiContextWindow(openaiModel);
  }

  return inferOllamaContextWindow(ollamaModel);
}

export function computeComposeTokenUsage(args: {
  messages: ChatMessage[];
  draft: string;
  modelProvider: ModelProvider;
  openaiModel: string;
  ollamaModel: string;
  thresholdPercent: number;
}): ComposeTokenUsage {
  const contextWindowTokens = inferComposeContextWindowTokens(
    args.modelProvider,
    args.openaiModel,
    args.ollamaModel,
  );
  const contextTokens = countChatMessageTokens(args.messages);
  const draftTokens = countTiktoken(args.draft);
  const projectedTurnTokens = contextTokens + draftTokens;
  const usagePercent = ratioToPercent(projectedTurnTokens, contextWindowTokens);
  const thresholdPercent = clamp(args.thresholdPercent, 0, 100);
  const thresholdTokens = Math.round((contextWindowTokens * thresholdPercent) / 100);
  const remainingTokens = Math.max(0, contextWindowTokens - projectedTurnTokens);

  return {
    contextTokens,
    draftTokens,
    projectedTurnTokens,
    contextWindowTokens,
    usagePercent,
    thresholdPercent,
    thresholdTokens,
    remainingTokens,
  };
}
