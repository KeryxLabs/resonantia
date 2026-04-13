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

export interface AppConfig {
  gatewayBaseUrl: string;
  ollamaBaseUrl: string;
  ollamaModel: string;
  layoutOverrides: LayoutOverrides;
}

export interface SyncNowRequest {
  sessionId?: string;
  gatewayBaseUrl?: string;
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

export type ChatMessageRole = "system" | "user" | "assistant";

export interface ChatMessage {
  role: ChatMessageRole;
  content: string;
}

export interface ComposeChatRequest {
  sessionId?: string;
  messages: ChatMessage[];
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
  syncPull(request: SyncPullRequest): Promise<SyncPullCommandResponse>;
  syncNow(request?: SyncNowRequest): Promise<SyncNowResponse>;
  calibrateSession(input: CalibrateSessionInput): Promise<CalibrateSessionResponse>;
  chatCompose(request: ComposeChatRequest): Promise<string | null>;
  encodeCompose(request: EncodeComposeRequest): Promise<string>;
  summarizeNode(rawNode: string): Promise<AiSummary | null>;
  setOllamaConfig(baseUrl?: string, model?: string): Promise<void>;
  setGatewayBaseUrl(baseUrl: string): Promise<void>;
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
    chatCompose: (request) =>
      invokeCommand<string | null>("chat_compose", {
        request,
      }),
    encodeCompose: (request) =>
      invokeCommand<string>("encode_compose", {
        request,
      }),
    summarizeNode: (rawNode) =>
      invokeCommand<AiSummary | null>("summarize_node", {
        rawNode,
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
  };
}
