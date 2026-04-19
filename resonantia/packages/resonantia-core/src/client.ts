import type {
  AiSummary,
  CalibrateSessionResponse,
  GraphResponse,
  NodeDto,
  StoreContextResponse,
  SyncNowResponse,
  SyncPullCommandResponse,
} from "./types";

export type CommandInvoker = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface HealthResponse {
  status: string;
  transport: string;
}

export interface LayoutPoint {
  x: number;
  y: number;
}

export interface LayoutOverrides {
  sessionOverrides: Record<string, LayoutPoint>;
  nodeOverrides: Record<string, LayoutPoint>;
}

export type ModelProvider = "managed-gateway" | "ollama" | "openai-byo";

export interface OpenAiByoKeyStatus {
  configured: boolean;
  source: "os-keyring" | "local-storage" | "unsupported";
}

export interface AppConfig {
  modelProvider: ModelProvider;
  gatewayBaseUrl: string;
  gatewayAuthToken: string;
  ollamaBaseUrl: string;
  ollamaModel: string;
  openaiBaseUrl: string;
  openaiModel: string;
  layoutOverrides: LayoutOverrides;
}

export interface SyncNowRequest {
  sessionId?: string;
  gatewayBaseUrl?: string;
  gatewayAuthToken?: string;
  pageSize?: number;
  maxBatches?: number;
}

export interface CalibrateSessionInput {
  sessionId: string;
  stability: number;
  friction: number;
  logic: number;
  autonomy: number;
  trigger: string;
}

export interface StoreContextInput {
  node: string;
  sessionId: string;
}

export interface RenameSessionInput {
  sourceSessionId: string;
  targetSessionId: string;
  allowMerge?: boolean;
}

export interface RenameSessionResponse {
  sourceSessionId: string;
  targetSessionId: string;
  movedNodes: number;
  movedCalibrations: number;
  scopesApplied: number;
}

export type ChatMessageRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatMessageRole;
  content: string;
}

export interface ComposeChatRequest {
  sessionId?: string;
  messages: ChatMessage[];
}

export interface ComposeUsage {
  promptTokens?: number;
  completionTokens?: number;
  totalTokens?: number;
}

export interface ComposeChatResponse {
  content: string | null;
  provider?: string;
  model?: string;
  usage?: ComposeUsage | null;
}

export interface EncodeComposeRequest {
  sessionId: string;
  messages: ChatMessage[];
  parserErrorHint?: string;
  previousNodeCandidate?: string;
}

export interface SyncPullRequest {
  sessionId: string;
  connectorId: string;
  source?: string;
  gatewayBaseUrl?: string;
  gatewayAuthToken?: string;
  pageSize?: number;
  maxBatches?: number;
  minPsi?: number;
  blockedTiers?: string[];
}

export interface ListNodesResponse {
  nodes: NodeDto[];
  retrieved: number;
  source?: string;
  transport?: string;
}

export interface ResonantiaClient {
  getHealth(): Promise<HealthResponse>;
  getConfig(): Promise<AppConfig>;
  getComposeEncodePreamble(): Promise<string>;
  listNodes(limit: number, sessionId?: string): Promise<ListNodesResponse>;
  getGraph(limit: number, sessionId?: string): Promise<GraphResponse>;
  storeContext(input: StoreContextInput): Promise<StoreContextResponse>;
  renameSession(input: RenameSessionInput): Promise<RenameSessionResponse>;
  syncPull(request: SyncPullRequest): Promise<SyncPullCommandResponse>;
  syncNow(request?: SyncNowRequest): Promise<SyncNowResponse>;
  calibrateSession(input: CalibrateSessionInput): Promise<CalibrateSessionResponse>;
  chatCompose(request: ComposeChatRequest): Promise<ComposeChatResponse>;
  encodeCompose(request: EncodeComposeRequest): Promise<string>;
  summarizeNode(rawNode: string): Promise<AiSummary | null>;
  getOpenAiByoKeyStatus(): Promise<OpenAiByoKeyStatus>;
  setOpenAiByoKey(key: string): Promise<void>;
  clearOpenAiByoKey(): Promise<void>;
  setModelProvider(provider: ModelProvider): Promise<void>;
  setOpenAiConfig(baseUrl?: string, model?: string): Promise<void>;
  setOllamaConfig(baseUrl?: string, model?: string): Promise<void>;
  setGatewayBaseUrl(baseUrl: string): Promise<void>;
  setGatewayAuthToken(token: string): Promise<void>;
}

function toObjectRecord(value: unknown): Record<string, unknown> {
  return typeof value === "object" && value !== null
    ? (value as Record<string, unknown>)
    : {};
}

function toOptionalNumber(value: unknown): number | undefined {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }

  if (typeof value === "string") {
    const parsed = Number(value.trim());
    if (Number.isFinite(parsed)) {
      return parsed;
    }
  }

  return undefined;
}

function normalizeComposeChatResponse(value: unknown): ComposeChatResponse {
  if (typeof value === "string") {
    return {
      content: value,
    };
  }

  if (value === null || value === undefined) {
    return {
      content: null,
    };
  }

  const record = toObjectRecord(value);
  const usageRecord = toObjectRecord(record.usage);
  const usage = {
    promptTokens: toOptionalNumber(usageRecord.promptTokens ?? usageRecord.prompt_tokens),
    completionTokens: toOptionalNumber(usageRecord.completionTokens ?? usageRecord.completion_tokens),
    totalTokens: toOptionalNumber(usageRecord.totalTokens ?? usageRecord.total_tokens),
  };
  const hasUsage = usage.promptTokens !== undefined
    || usage.completionTokens !== undefined
    || usage.totalTokens !== undefined;

  return {
    content: typeof record.content === "string" ? record.content : null,
    provider: typeof record.provider === "string" ? record.provider : undefined,
    model: typeof record.model === "string" ? record.model : undefined,
    usage: hasUsage ? usage : null,
  };
}

export function createResonantiaClient(invokeCommand: CommandInvoker): ResonantiaClient {
  return {
    getHealth: () => invokeCommand<HealthResponse>("get_health"),
    getConfig: () => invokeCommand<AppConfig>("get_config"),
    getComposeEncodePreamble: () => invokeCommand<string>("get_compose_encode_preamble"),
    listNodes: (limit, sessionId) =>
      invokeCommand<ListNodesResponse>("list_nodes", {
        limit,
        sessionId: sessionId ?? null,
      }),
    getGraph: (limit, sessionId) =>
      invokeCommand<GraphResponse>("get_graph", {
        limit,
        sessionId: sessionId ?? null,
      }),
    storeContext: (input) =>
      invokeCommand<StoreContextResponse>("store_context", {
        request: {
          node: input.node,
          sessionId: input.sessionId,
        },
      }),
    renameSession: (input) =>
      invokeCommand<RenameSessionResponse>("rename_session", {
        request: {
          sourceSessionId: input.sourceSessionId,
          targetSessionId: input.targetSessionId,
          allowMerge: input.allowMerge ?? false,
        },
      }),
    syncPull: (request) =>
      invokeCommand<SyncPullCommandResponse>("sync_pull", {
        request,
      }),
    syncNow: (request = {}) =>
      invokeCommand<SyncNowResponse>("sync_now", {
        request,
      }),
    calibrateSession: (input) =>
      invokeCommand<CalibrateSessionResponse>("calibrate_session", {
        request: {
          sessionId: input.sessionId,
          stability: input.stability,
          friction: input.friction,
          logic: input.logic,
          autonomy: input.autonomy,
          trigger: input.trigger,
        },
      }),
    chatCompose: async (request) => {
      const response = await invokeCommand<unknown>("chat_compose", {
        request,
      });

      return normalizeComposeChatResponse(response);
    },
    encodeCompose: (request) =>
      invokeCommand<string>("encode_compose", {
        request,
      }),
    summarizeNode: (rawNode) =>
      invokeCommand<AiSummary | null>("summarize_node", {
        rawNode,
      }),
    getOpenAiByoKeyStatus: () => invokeCommand<OpenAiByoKeyStatus>("get_openai_byo_key_status"),
    setOpenAiByoKey: (key) =>
      invokeCommand<void>("set_openai_byo_key", {
        key,
      }),
    clearOpenAiByoKey: () => invokeCommand<void>("clear_openai_byo_key"),
    setModelProvider: (provider) =>
      invokeCommand<void>("set_model_provider", {
        provider,
      }),
    setOpenAiConfig: (baseUrl, model) =>
      invokeCommand<void>("set_openai_config", {
        baseUrl,
        model,
      }),
    setOllamaConfig: (baseUrl, model) =>
      invokeCommand<void>("set_ollama_config", {
        baseUrl,
        model,
      }),
    setGatewayBaseUrl: (baseUrl) =>
      invokeCommand<void>("set_gateway_base_url", {
        baseUrl,
      }),
    setGatewayAuthToken: (token) =>
      invokeCommand<void>("set_gateway_auth_token", {
        token,
      }),
  };
}
