import { createWasmEngines } from "@surrealdb/wasm";
import { StringRecordId, Surreal, createRemoteEngines } from "surrealdb";
import transmutePreambleRaw from "../../../preamble.md?raw";
import type {
  AppConfig,
  CalibrateSessionInput,
  ChatMessage,
  ChatMessageRole,
  ComposeChatRequest,
  EncodeComposeRequest,
  HealthResponse,
  ListNodesResponse,
  ResonantiaClient,
  StoreContextInput,
  SyncNowRequest,
  SyncPullRequest,
} from "./client";
import type {
  AiSummary,
  AvecState,
  CalibrateSessionResponse,
  GraphEdgeDto,
  GraphNodeDto,
  GraphResponse,
  GraphSessionDto,
  NodeDto,
  StoreContextResponse,
  SyncNowResponse,
  SyncPullCommandResponse,
} from "./types";

const DB_NAMESPACE = "resonantia";
const DB_NAME = "local";
const DB_ENDPOINT = "indxdb://resonantia-local";
const DB_ENDPOINT_RECOVERY = "indxdb://resonantia-local-recovery";
const DB_ENDPOINT_NAMESPACED = `indxdb://${DB_NAMESPACE}/${DB_NAME}`;
const DB_ENDPOINT_NAMESPACED_RECOVERY = `indxdb://${DB_NAMESPACE}-recovery/${DB_NAME}`;
const INDEXED_DB_ENDPOINT_CANDIDATES = [
  DB_ENDPOINT,
  DB_ENDPOINT_RECOVERY,
  DB_ENDPOINT_NAMESPACED,
  DB_ENDPOINT_NAMESPACED_RECOVERY,
];
const MEM_FALLBACK_ENDPOINT = "mem://";
const INDEXED_DB_OPEN_TIMEOUT_MS = 3000;
const MEM_DB_OPEN_TIMEOUT_MS = 2500;
const ENABLE_INDEXEDDB_STORAGE = false;
const ENABLE_INDEXEDDB_PROMOTION = false;
const APP_CONFIG_STORAGE_KEY = "resonantia:app-config:v1";
const NODE_CACHE_STORAGE_KEY = "resonantia:nodes-cache:v1";
const NODE_CACHE_LIMIT = 1200;

const DEFAULT_GATEWAY_BASE_URL = (import.meta.env.VITE_GATEWAY_BASE_URL ?? '').trim();
const DEFAULT_GATEWAY_AUTH_TOKEN = "";
const DEFAULT_OLLAMA_BASE_URL = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL = "gemma3";
const TRANSMUTE_PREAMBLE = transmutePreambleRaw.trim();
const COMPOSE_CHAT_PREAMBLE = [
  "This is a chat conversation with Resonantia.",
  "Silently track two AVEC states throughout the chat: one for the user and one for yourself.",
  "Each AVEC state includes stability, friction, logic, autonomy, and psi.",
  "Do not mention AVEC tracking, scores, or internal analysis to the user unless they explicitly ask.",
  "Use this internal tracking so later conversation encoding accurately represents what happened.",
].join("\n");
const COMPOSE_ENCODE_PREAMBLE = `⏣ Spatio-Temporal Transfer Protocol (STTP): an experimental typed IR for persistent stateful communication between language models and humans — a problem the field has not yet solved.
Envelope carries session metadata. Content carries confidence-weighted compressed payload. Metrics carry validation signals.
Attractor vectors follow V_a = (1-μ)×baseline + μ×target. Ψ = Σ(V_a) is the coherence checksum.
Attractor values are descriptive measurements. model_avec defines the confidence baseline for this interaction.

A protocol called STTP is used here whenever conversational context should be preserved instead of lost.
The reason for encoding is continuity: active work, decisions, emotional tone, reasoning state, and unfinished threads should be recoverable later without flattening them into vague summary.
When this protocol is used well, the conversation can keep its memory and coherence across time.

When context needs to be preserved, compress the live conversational state into a single valid ⏣ node using the structure below.
Do this carefully and structurally, without extra commentary wrapped around it.

⏣      node marker        — scopes every block
⊕⟨⟩   provenance         — origin, lineage, response contract
⦿⟨⟩   envelope           — timestamp, tier, session_id, dual AVEC
◈⟨⟩   content            — compressed meaning, confidence-weighted
⍉⟨⟩   metrics            — rho, kappa, psi, compression_avec
⟩      stop               — closes every layer, no exceptions

Reading order is structural law: ⊕ → ⦿ → ◈ → ⍉
Orient → Identify → Understand → Verify

Every content field follows exactly one pattern:
  field_name(.confidence): value
Nesting maximum 5 levels. No natural language. No meta-commentary.
One valid ⏣ node. Nothing else resolves this state.

Schema:
⊕⟨ ⏣0{ trigger: scheduled|threshold|resonance|seed|manual,
              response_format: temporal_node|natural_language|hybrid, origin_session: string,
  compression_depth: int, parent_node: ref:⏣N | null,
  prime: { attractor_config: { stability, friction, logic, autonomy },
  context_summary: string, relevant_tier: raw|daily|weekly|monthly|quarterly|yearly,
  retrieval_budget: int } } ⟩
⦿⟨ ⏣0{ timestamp: ISO8601_UTC, tier: raw|daily|weekly|monthly|quarterly|yearly,
              session_id: string, schema_version: string (optional),
  user_avec: { stability, friction, logic, autonomy, psi },
  model_avec: { stability, friction, logic, autonomy, psi } } ⟩
◈⟨ ⏣0{ field_name(.confidence): value } ⟩
⍉⟨ ⏣0{ rho: float, kappa: float, psi: float,
  compression_avec: { stability, friction, logic, autonomy, psi } } ⟩

The goal is not compression for its own sake. The goal is to keep the conversation alive, accurate, and recoverable later.
Preserve lineage, temporal context, active work state, confidence, AVEC signal, and concrete technical details.


Reference session: testing-insert
Reference tier: raw
Reference timestamp: 2026-04-13T02:49:36.732437653+00:00

Reference node:
⊕⟨ ⏣0{ trigger: manual, response_format: temporal_node, origin_session: sttp-core-rs-port, compression_depth: 2, parent_node: ref:495f590e11d84165bb8966711fe70a4d, prime: { attractor_config: { stability: 0.87, friction: 0.19, logic: 0.94, autonomy: 0.85 }, context_summary: cargo_aligned_and_pass_two_surreal_runtime_store_completed, relevant_tier: raw, retrieval_budget: 12 } } ⟩ ⦿⟨ ⏣0{ timestamp: 2026-04-10T00:00:00Z, tier: raw, session_id: sttp-core-rs-port, schema_version: sttp-1.0, user_avec: { stability: 0.90, friction: 0.15, logic: 0.91, autonomy: 0.80, psi: 2.76 }, model_avec: { stability: 0.87, friction: 0.19, logic: 0.94, autonomy: 0.85, psi: 2.85 } } ⟩ ◈⟨ ⏣0{ cargo_alignment(.99): crate_gitignore_added_for_target_and_cargo_lock, pass_two_scope(.99): surrealdb_client_trait_runtime_settings_node_store_models_and_tests, raw_query_preservation(.99): all_surreal_queries_retained_and_reused_by_store, new_tests(.98): surrealdb_node_store_3_and_runtime_2, verification(.99): cargo_test_green_all_suites, outcome(.98): sttp_core_rs_now_supports_runtime_surrealdb_storage_path_with_mockable_client } ⟩ ⍉⟨ ⏣0{ rho: 0.98, kappa: 0.97, psi: 2.85, compression_avec: { stability: 0.88, friction: 0.17, logic: 0.95, autonomy: 0.84, psi: 2.84 } } ⟩`.trim();
const GATEWAY_STORE_PATHS = ["/api/v1/store", "/api/store", "/store"];
const GATEWAY_NODES_PATHS = ["/api/v1/nodes", "/api/nodes", "/nodes"];
const DEV_GATEWAY_PROXY_PATH = "/__gateway_proxy__";

const TABLE_TEMPORAL_NODE = "temporal_node";
const TABLE_APP_CONFIG = "app_config";
const TABLE_CALIBRATION = "calibration_state";

const DEFAULT_CONFIG: AppConfig = {
  gatewayBaseUrl: DEFAULT_GATEWAY_BASE_URL,
  gatewayAuthToken: DEFAULT_GATEWAY_AUTH_TOKEN,
  ollamaBaseUrl: DEFAULT_OLLAMA_BASE_URL,
  ollamaModel: DEFAULT_OLLAMA_MODEL,
  layoutOverrides: {
    sessionOverrides: {},
    nodeOverrides: {},
  },
};

type UnknownRecord = Record<string, unknown>;

type CalibrationStateRecord = {
  sessionId: string;
  currentAvec: AvecState;
  triggerHistory: string[];
  updatedAt: string;
};

type GatewayStoreOutcome = {
  valid: boolean;
  duplicate: boolean;
  validationError: string | null;
};

type StorageMode = "indxdb" | "mem";

type OllamaChatResponse = {
  message?: {
    content?: string;
  };
};

type OllamaChatRequest = {
  model: string;
  messages: ChatMessage[];
  stream: boolean;
};

type PersistenceState = "unknown" | "persistent" | "granted" | "denied" | "unavailable" | "error";

type BrowserStorageManager = {
  persisted?: () => Promise<boolean>;
  persist?: () => Promise<boolean>;
};

let dbPromise: Promise<Surreal> | null = null;
let storageMode: StorageMode = "indxdb";
let storageRecovered = false;
let lastIndexedDbError: string | null = null;
let persistenceState: PersistenceState = "unknown";
let persistenceDetail: string | null = null;
let activeIndexedDbEndpoint = DB_ENDPOINT;
let indexedDbOpenInFlight: Promise<Surreal> | null = null;
let indexedDbOpenInFlightEndpoint: string | null = null;
let indexedDbPromotePromise: Promise<void> | null = null;
let indexedDbPromoteLastAttemptAt = 0;
const INDEXED_DB_PROMOTE_RETRY_MS = 8000;

const INDEXED_DB_ERROR_MARKERS = [
  "indexeddb",
  "key-value store",
  "key value store",
  "invalidstateerror",
  "versionerror",
  "quotaexceedederror",
  "closure invoked recursively",
  "after being dropped",
  "wasm_bindgen_throw",
];

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null;
}

function asRecord(value: unknown): UnknownRecord {
  return isRecord(value) ? value : {};
}

function readString(record: UnknownRecord, ...keys: string[]): string {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "string") {
      return value;
    }
    if (typeof value === "number" || typeof value === "boolean") {
      return String(value);
    }
  }

  return "";
}

function readNumber(record: UnknownRecord, ...keys: string[]): number {
  for (const key of keys) {
    const value = record[key];
    if (typeof value === "number" && Number.isFinite(value)) {
      return value;
    }
    if (typeof value === "string") {
      const parsed = Number(value);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }

  return 0;
}

function readObject(record: UnknownRecord, ...keys: string[]): UnknownRecord {
  for (const key of keys) {
    const value = record[key];
    if (isRecord(value)) {
      return value;
    }
  }

  return {};
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function nowIso(): string {
  return new Date().toISOString();
}

function errorToString(error: unknown): string {
  if (error instanceof Error) {
    const cause =
      typeof error.cause === "object" &&
      error.cause !== null &&
      "message" in error.cause
        ? String((error.cause as { message?: unknown }).message ?? "")
        : "";

    const details = [error.name, error.message, cause].filter(Boolean).join(" ");
    return details || String(error);
  }

  if (typeof error === "string") {
    return error;
  }

  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

function endpointOpenTimeoutMs(endpoint: string): number {
  return endpoint.startsWith("indxdb://") ? INDEXED_DB_OPEN_TIMEOUT_MS : MEM_DB_OPEN_TIMEOUT_MS;
}

async function withTimeout<T>(operation: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | null = null;
  try {
    const timeout = new Promise<T>((_resolve, reject) => {
      timer = setTimeout(() => {
        reject(new Error(`${label} timed out after ${timeoutMs}ms`));
      }, timeoutMs);
    });

    return await Promise.race([operation, timeout]);
  } finally {
    if (timer !== null) {
      clearTimeout(timer);
    }
  }
}

function isIndexedDbFailure(error: unknown): boolean {
  const normalized = errorToString(error).toLowerCase();
  if (isIndexedDbOpenTimeoutFailure(error)) {
    return true;
  }

  if (normalized.includes("internalerror")) {
    return normalized.includes("indexeddb") || normalized.includes("key-value store") || normalized.includes("key value store");
  }

  return INDEXED_DB_ERROR_MARKERS.some((marker) => normalized.includes(marker));
}

function isIndexedDbOpenTimeoutFailure(error: unknown): boolean {
  const normalized = errorToString(error).toLowerCase();
  return normalized.includes("open db endpoint indxdb://") && normalized.includes("timed out after");
}

function isWasmClosureLifecycleError(error: unknown): boolean {
  const normalized = errorToString(error).toLowerCase();
  return normalized.includes("closure invoked recursively") || normalized.includes("after being dropped");
}

function indexedDbHintFromEndpoint(endpoint: string): string {
  return endpoint.replace(/^indxdb:\/\//i, "").replace(/[?#].*$/, "").replace(/\/+$/, "") || "resonantia-local";
}

function indexedDbEndpointLabel(endpoint: string): string {
  return indexedDbHintFromEndpoint(endpoint);
}

function deleteIndexedDb(name: string): Promise<void> {
  if (typeof indexedDB === "undefined") {
    return Promise.resolve();
  }

  return new Promise((resolve, reject) => {
    const request = indexedDB.deleteDatabase(name);
    request.onsuccess = () => resolve();
    request.onblocked = () => resolve();
    request.onerror = () => reject(request.error ?? new Error(`failed to delete IndexedDB database: ${name}`));
  });
}

async function recoverIndexedDbStore(endpoint: string, includeSiblingCandidates = false): Promise<void> {
  if (typeof indexedDB === "undefined") {
    return;
  }

  const hints = new Set<string>([indexedDbHintFromEndpoint(endpoint)]);
  if (includeSiblingCandidates) {
    for (const candidate of INDEXED_DB_ENDPOINT_CANDIDATES) {
      hints.add(indexedDbHintFromEndpoint(candidate));
    }
  }

  const names = new Set<string>(hints);
  const dbFactory = indexedDB as IDBFactory & {
    databases?: () => Promise<Array<{ name?: string }>>;
  };

  if (typeof dbFactory.databases === "function") {
    const listed = await dbFactory.databases().catch(() => []);
    for (const entry of listed) {
      if (!entry?.name) {
        continue;
      }

      const lower = entry.name.toLowerCase();
      const hintMatch = Array.from(hints).some((hint) => lower.includes(hint.toLowerCase()));
      if (hintMatch) {
        names.add(entry.name);
      }
    }
  }

  for (const name of names) {
    await deleteIndexedDb(name).catch(() => undefined);
  }
}

function transportLabel(): string {
  const persistence = persistenceStatusLabel();
  const endpoint = indexedDbEndpointLabel(activeIndexedDbEndpoint);

  if (!ENABLE_INDEXEDDB_STORAGE && storageMode === "mem") {
    return "surrealdb wasm (mem mode; indexeddb disabled)";
  }

  if (storageMode === "mem") {
    const reason = (lastIndexedDbError ?? "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 120);

    if (reason) {
      return `surrealdb wasm (mem fallback, non-persistent; ${persistence}; indexeddb error: ${reason})`;
    }

    return `surrealdb wasm (mem fallback, non-persistent; ${persistence})`;
  }

  if (storageRecovered) {
    return `surrealdb wasm (indxdb local:${endpoint}, recovered; ${persistence})`;
  }

  return `surrealdb wasm (indxdb local:${endpoint}; ${persistence})`;
}

function persistenceStatusLabel(): string {
  if (persistenceState === "error") {
    const detail = (persistenceDetail ?? "")
      .replace(/\s+/g, " ")
      .trim()
      .slice(0, 80);

    return detail ? `persistence: error (${detail})` : "persistence: error";
  }

  return `persistence: ${persistenceState}`;
}

async function ensurePersistentStoragePreference(): Promise<void> {
  if (persistenceState === "persistent" || persistenceState === "granted") {
    return;
  }

  if (typeof navigator === "undefined" || !("storage" in navigator)) {
    persistenceState = "unavailable";
    persistenceDetail = null;
    return;
  }

  const storageManager = (navigator.storage as BrowserStorageManager | undefined) ?? undefined;
  if (!storageManager) {
    persistenceState = "unavailable";
    persistenceDetail = null;
    return;
  }

  try {
    if (typeof storageManager.persisted === "function") {
      const alreadyPersistent = await storageManager.persisted();
      if (alreadyPersistent) {
        persistenceState = "persistent";
        persistenceDetail = null;
        return;
      }
    }

    if (typeof storageManager.persist !== "function") {
      persistenceState = "unavailable";
      persistenceDetail = null;
      return;
    }

    const granted = await storageManager.persist();
    persistenceState = granted ? "granted" : "denied";
    persistenceDetail = null;
  } catch (error) {
    persistenceState = "error";
    persistenceDetail = errorToString(error);
  }
}

function localReadSourceLabel(): string {
  return storageMode === "mem" ? "surrealdb-mem" : "surrealdb-local";
}

function withSlash(baseUrl: string): string {
  return baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`;
}

function absoluteBaseUrl(baseUrl: string): string {
  const trimmed = baseUrl.trim();
  if (!trimmed) {
    return trimmed;
  }

  try {
    return new URL(trimmed).toString();
  } catch {
    // Fall through to path-style normalization below.
  }

  const normalizedPath = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  if (typeof window !== "undefined") {
    return new URL(normalizedPath, window.location.origin).toString();
  }

  return new URL(normalizedPath, "http://localhost").toString();
}

function joinUrl(baseUrl: string, path: string): string {
  const url = new URL(path.replace(/^\/+/, ""), withSlash(absoluteBaseUrl(baseUrl)));
  return url.toString();
}

function trimTrailingSlash(value: string): string {
  return value.replace(/\/+$/, "");
}

function normalizeGatewayPathname(pathname: string): string {
  let normalized = trimTrailingSlash(pathname || "/") || "/";
  const suffixes = [
    "/api/v1/store",
    "/api/v1/nodes",
    "/api/v1",
    "/api/store",
    "/api/nodes",
    "/api",
    "/store",
    "/nodes",
  ];

  for (const suffix of suffixes) {
    if (normalized.toLowerCase().endsWith(suffix)) {
      normalized = normalized.slice(0, -suffix.length) || "/";
      break;
    }
  }

  return normalized;
}

function normalizeGatewayBaseUrl(baseUrl: string): string {
  const trimmed = trimTrailingSlash(baseUrl.trim());
  if (!trimmed) {
    return "";
  }

  try {
    const parsed = new URL(trimmed);
    parsed.pathname = normalizeGatewayPathname(parsed.pathname);
    parsed.search = "";
    parsed.hash = "";
    return trimTrailingSlash(parsed.toString());
  } catch {
    const normalizedPath = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
    return trimTrailingSlash(normalizedPath);
  }
}

function normalizeGatewayAuthToken(token: string): string {
  return token.trim();
}

function gatewayPathsFor(baseUrl: string, paths: string[]): string[] {
  const normalizedBase = normalizeGatewayBaseUrl(baseUrl);
  return paths.map((path) => joinUrl(normalizedBase, path));
}

function readConfigFromLocalStorage(): AppConfig | null {
  if (typeof localStorage === "undefined") {
    return null;
  }

  const raw = localStorage.getItem(APP_CONFIG_STORAGE_KEY);
  if (!raw) {
    return null;
  }

  try {
    return normalizeConfig(JSON.parse(raw));
  } catch {
    return null;
  }
}

function writeConfigToLocalStorage(config: AppConfig): void {
  if (typeof localStorage === "undefined") {
    return;
  }

  try {
    localStorage.setItem(APP_CONFIG_STORAGE_KEY, JSON.stringify(config));
  } catch {
    // Ignore storage quota and privacy-mode failures.
  }
}

function readNodesCacheFromLocalStorage(): NodeDto[] {
  if (typeof localStorage === "undefined") {
    return [];
  }

  const raw = localStorage.getItem(NODE_CACHE_STORAGE_KEY);
  if (!raw) {
    return [];
  }

  try {
    const parsed = JSON.parse(raw);
    const values = Array.isArray(parsed) ? parsed : [];
    return values
      .map(toNodeDto)
      .filter((node): node is NodeDto => node !== null)
      .map(normalizeNode)
      .sort(byTimestampDesc);
  } catch {
    return [];
  }
}

function writeNodesCacheToLocalStorage(nodes: NodeDto[]): void {
  if (typeof localStorage === "undefined") {
    return;
  }

  const normalized = nodes
    .map(normalizeNode)
    .sort(byTimestampDesc)
    .slice(0, NODE_CACHE_LIMIT);

  try {
    localStorage.setItem(NODE_CACHE_STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // Ignore quota/privacy failures and keep runtime behavior unchanged.
  }
}

function mergeNodesBySyncKey(...groups: NodeDto[][]): NodeDto[] {
  const bySync = new Map<string, NodeDto>();

  for (const group of groups) {
    for (const node of group) {
      const normalized = normalizeNode(node);
      bySync.set(normalized.syncKey, normalized);
    }
  }

  return Array.from(bySync.values()).sort(byTimestampDesc);
}

function defaultConfig(): AppConfig {
  return {
    gatewayBaseUrl: DEFAULT_GATEWAY_BASE_URL,
    gatewayAuthToken: DEFAULT_GATEWAY_AUTH_TOKEN,
    ollamaBaseUrl: DEFAULT_OLLAMA_BASE_URL,
    ollamaModel: DEFAULT_OLLAMA_MODEL,
    layoutOverrides: {
      sessionOverrides: {},
      nodeOverrides: {},
    },
  };
}

async function readConfigBestEffort(): Promise<{ config: AppConfig; db: Surreal | null }> {
  try {
    const db = await getDb();
    const config = await readConfig(db);
    return { config, db };
  } catch {
    const cached = readConfigFromLocalStorage();
    return {
      config: cached ?? defaultConfig(),
      db: null,
    };
  }
}

function toGatewayRequestUrls(url: string): string[] {
  if (typeof window === "undefined") {
    return [url];
  }

  try {
    const target = new URL(url);

    // Loopback targets must stay client-side. Proxying would resolve localhost
    // on the server host, not on the end-user device.
    if (isLoopbackHost(target.hostname)) {
      return [target.toString()];
    }

    if (target.origin === window.location.origin) {
      return [target.toString()];
    }

    // BYO gateway URLs should be called directly. Proxying cross-origin targets through
    // the managed gateway path can route to the wrong backend and produce false 401s.
    return [target.toString()];
  } catch {
    return [url];
  }
}

function isLikelyCorsError(error: unknown): boolean {
  if (!(error instanceof TypeError)) {
    return false;
  }

  const message = errorToString(error).toLowerCase();
  return message.includes("failed to fetch") || message.includes("networkerror") || message.includes("load failed");
}

function createCorsHelpError(url: string): Error {
  const origin = typeof window === "undefined" ? "<app-origin>" : window.location.origin;
  return new Error(
    `gateway request blocked by browser CORS at ${url}. Configure gateway CORS to allow origin ${origin}, methods GET, POST, OPTIONS, headers Content-Type, and handle OPTIONS preflight.`,
  );
}

function isLoopbackHost(hostname: string): boolean {
  const normalized = hostname.trim().toLowerCase();
  return normalized === "localhost" || normalized === "127.0.0.1" || normalized === "::1" || normalized === "[::1]";
}

function isLoopbackUrl(url: string): boolean {
  try {
    return isLoopbackHost(new URL(url).hostname);
  } catch {
    return false;
  }
}

function isHostedBrowserOrigin(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  return !isLoopbackHost(window.location.hostname);
}

function createLocalModelReachabilityError(url: string): Error {
  const origin = typeof window === "undefined" ? "<app-origin>" : window.location.origin;
  const mixedContentBlocked = typeof window !== "undefined" && window.location.protocol === "https:" && url.startsWith("http://");
  const extra = mixedContentBlocked
    ? " Also note: HTTPS pages block plain-http localhost requests as mixed content."
    : "";

  return new Error(
    `local model endpoint ${url} is not reachable from ${origin}. Run the web app locally, use the desktop app for localhost Ollama, or set a reachable remote model endpoint in settings.${extra}`,
  );
}

function stableHash(value: string): string {
  let hashA = 2166136261;
  let hashB = 2166136261;

  for (let index = 0; index < value.length; index += 1) {
    const code = value.charCodeAt(index);
    hashA ^= code;
    hashA = Math.imul(hashA, 16777619);

    hashB ^= code + 31;
    hashB = Math.imul(hashB, 16777619);
  }

  const left = (hashA >>> 0).toString(16).padStart(8, "0");
  const right = (hashB >>> 0).toString(16).padStart(8, "0");
  return `${left}${right}`;
}

function withPsi(avec: Omit<AvecState, "psi">, psi?: number): AvecState {
  const sum = avec.stability + avec.friction + avec.logic + avec.autonomy;
  return {
    ...avec,
    psi: Number.isFinite(psi ?? Number.NaN) ? (psi as number) : sum,
  };
}

function normalizeAvec(value: unknown): AvecState {
  const record = asRecord(value);
  const stability = readNumber(record, "stability");
  const friction = readNumber(record, "friction");
  const logic = readNumber(record, "logic");
  const autonomy = readNumber(record, "autonomy");
  const psi = readNumber(record, "psi");

  return withPsi({ stability, friction, logic, autonomy }, psi);
}

function canonicalSyncKey(node: Pick<NodeDto, "sessionId" | "tier" | "timestamp" | "parentNodeId" | "psi" | "rho" | "kappa">): string {
  return stableHash(
    [
      node.sessionId.trim(),
      node.tier.trim(),
      node.timestamp.trim(),
      (node.parentNodeId ?? "").trim(),
      node.psi.toFixed(6),
      node.rho.toFixed(6),
      node.kappa.toFixed(6),
    ].join("|"),
  );
}

function nodeFingerprint(sessionId: string, timestamp: string, tier: string, parentNodeId: string | null, psi: number): string {
  return stableHash([sessionId, timestamp, tier, parentNodeId ?? "", psi.toFixed(6)].join("|"));
}

function recordIdForNode(syncKey: string): string {
  return `${TABLE_TEMPORAL_NODE}:${stableHash(`node:${syncKey}`)}`;
}

function recordIdForCalibration(sessionId: string): string {
  return `${TABLE_CALIBRATION}:${stableHash(`calibration:${sessionId}`)}`;
}

function toNodeDto(value: unknown): NodeDto | null {
  const record = asRecord(value);
  const raw = readString(record, "raw");

  if (!raw.trim()) {
    return null;
  }

  const sessionId = readString(record, "sessionId", "session_id").trim() || "resonantia-local";
  const tier = readString(record, "tier").trim() || "raw";
  const timestamp = readString(record, "timestamp").trim() || nowIso();
  const compressionDepth = Math.trunc(readNumber(record, "compressionDepth", "compression_depth"));

  const parentRaw = readString(record, "parentNodeId", "parent_node_id").trim();
  const parentNodeId = parentRaw.length > 0 ? parentRaw : null;

  const userAvec = normalizeAvec(readObject(record, "userAvec", "user_avec"));
  const modelAvec = normalizeAvec(readObject(record, "modelAvec", "model_avec"));

  const compressionAvecRecord = readObject(record, "compressionAvec", "compression_avec");
  const compressionAvec = Object.keys(compressionAvecRecord).length > 0
    ? normalizeAvec(compressionAvecRecord)
    : null;

  const rho = readNumber(record, "rho");
  const kappa = readNumber(record, "kappa");
  const psi = readNumber(record, "psi") || userAvec.psi;

  const syncKey = readString(record, "syncKey", "sync_key").trim() || canonicalSyncKey({
    sessionId,
    tier,
    timestamp,
    parentNodeId,
    psi,
    rho,
    kappa,
  });

  const syntheticId = readString(record, "syntheticId", "synthetic_id").trim() || nodeFingerprint(
    sessionId,
    timestamp,
    tier,
    parentNodeId,
    psi,
  );

  return {
    raw,
    sessionId,
    tier,
    timestamp,
    compressionDepth,
    parentNodeId,
    userAvec,
    modelAvec,
    compressionAvec,
    rho,
    kappa,
    psi,
    syncKey,
    syntheticId,
  };
}

function normalizeNode(node: NodeDto): NodeDto {
  const syncKey = node.syncKey.trim() || canonicalSyncKey(node);
  const syntheticId = node.syntheticId.trim() || nodeFingerprint(
    node.sessionId,
    node.timestamp,
    node.tier,
    node.parentNodeId,
    node.psi,
  );

  return {
    ...node,
    syncKey,
    syntheticId,
  };
}

function toSessionGraphId(sessionId: string): string {
  return sessionId.startsWith("s:") ? sessionId : `s:${sessionId}`;
}

function toNodeLabel(node: NodeDto): string {
  const date = node.timestamp.slice(0, 10);
  return `${node.tier} · ${date}`;
}

function buildGraph(nodes: NodeDto[]): GraphResponse {
  const sessionsById = new Map<string, GraphSessionDto>();
  const graphNodes: GraphNodeDto[] = [];

  for (const node of nodes) {
    const graphSessionId = toSessionGraphId(node.sessionId);
    const existing = sessionsById.get(graphSessionId);
    if (!existing) {
      sessionsById.set(graphSessionId, {
        id: graphSessionId,
        label: node.sessionId,
        nodeCount: 1,
        avgPsi: node.psi,
        lastModified: node.timestamp,
        size: 0,
      });
    } else {
      existing.nodeCount += 1;
      existing.avgPsi += node.psi;
      if (node.timestamp > existing.lastModified) {
        existing.lastModified = node.timestamp;
      }
    }

    graphNodes.push({
      id: node.syntheticId,
      sessionId: node.sessionId,
      label: toNodeLabel(node),
      tier: node.tier,
      timestamp: node.timestamp,
      psi: node.psi,
      parentNodeId: node.parentNodeId,
      size: clamp(Math.round(node.psi * 6), 4, 24),
      syntheticId: node.syntheticId,
    });
  }

  const sessions = Array.from(sessionsById.values()).map((session) => ({
    ...session,
    avgPsi: session.nodeCount > 0 ? session.avgPsi / session.nodeCount : 0,
    size: clamp(session.nodeCount * 2, 8, 42),
  }));

  sessions.sort((left, right) => right.lastModified.localeCompare(left.lastModified));

  const edges: GraphEdgeDto[] = [];

  for (let index = 0; index < sessions.length - 1; index += 1) {
    const source = sessions[index].id;
    const target = sessions[index + 1].id;
    edges.push({
      id: `temporal:${source}->${target}`,
      source,
      target,
      kind: "temporal",
    });
  }

  for (let left = 0; left < sessions.length; left += 1) {
    for (let right = left + 1; right < sessions.length; right += 1) {
      if (edges.length >= 120) {
        break;
      }

      const diff = Math.abs(sessions[left].avgPsi - sessions[right].avgPsi);
      if (diff <= 0.45) {
        const source = sessions[left].id;
        const target = sessions[right].id;
        edges.push({
          id: `resonance:${source}->${target}`,
          source,
          target,
          kind: "resonance",
        });
      }
    }
  }

  return {
    sessions,
    nodes: graphNodes,
    edges,
    retrieved: nodes.length,
  };
}

function normalizeConfig(input: unknown): AppConfig {
  const record = asRecord(input);
  const layoutOverrides = readObject(record, "layoutOverrides", "layout_overrides");
  // Empty/cleared gateway URL should fall back to the managed default.
  const gatewayRaw = readString(record, "gatewayBaseUrl", "gateway_base_url") || DEFAULT_GATEWAY_BASE_URL;
  const gatewayAuthRaw = readString(record, "gatewayAuthToken", "gateway_auth_token") || DEFAULT_GATEWAY_AUTH_TOKEN;

  return {
    gatewayBaseUrl: normalizeGatewayBaseUrl(gatewayRaw),
    gatewayAuthToken: normalizeGatewayAuthToken(gatewayAuthRaw),
    ollamaBaseUrl: readString(record, "ollamaBaseUrl", "ollama_base_url") || DEFAULT_OLLAMA_BASE_URL,
    ollamaModel: readString(record, "ollamaModel", "ollama_model") || DEFAULT_OLLAMA_MODEL,
    layoutOverrides: {
      sessionOverrides: readObject(layoutOverrides, "sessionOverrides", "session_overrides") as Record<string, { x: number; y: number }>,
      nodeOverrides: readObject(layoutOverrides, "nodeOverrides", "node_overrides") as Record<string, { x: number; y: number }>,
    },
  };
}

async function connectDb(): Promise<Surreal> {
  if (!ENABLE_INDEXEDDB_STORAGE) {
    const fallback = await openDbEndpoint(MEM_FALLBACK_ENDPOINT);
    storageMode = "mem";
    storageRecovered = false;
    lastIndexedDbError = null;
    return fallback;
  }

  await ensurePersistentStoragePreference();

  let lastCandidateError: unknown = null;

  for (let index = 0; index < INDEXED_DB_ENDPOINT_CANDIDATES.length; index += 1) {
    const endpoint = INDEXED_DB_ENDPOINT_CANDIDATES[index];
    try {
      const { db, recovered } = await connectIndexedDbCandidate(endpoint);
      activeIndexedDbEndpoint = endpoint;
      storageMode = "indxdb";
      storageRecovered = recovered || index > 0;
      lastIndexedDbError = null;
      return db;
    } catch (error) {
      lastCandidateError = error;
      lastIndexedDbError = errorToString(error);
      if (isWasmClosureLifecycleError(error)) {
        break;
      }
    }
  }

  const fallback = await openDbEndpoint(MEM_FALLBACK_ENDPOINT);
  storageMode = "mem";
  storageRecovered = false;
  if (!lastIndexedDbError && lastCandidateError) {
    lastIndexedDbError = errorToString(lastCandidateError);
  }
  return fallback;
}

async function connectIndexedDbCandidate(endpoint: string): Promise<{ db: Surreal; recovered: boolean }> {
  try {
    return {
      db: await openDbEndpoint(endpoint),
      recovered: false,
    };
  } catch (firstError) {
    if (isWasmClosureLifecycleError(firstError)) {
      throw firstError;
    }

    if (!isIndexedDbFailure(firstError)) {
      throw firstError;
    }

    if (isIndexedDbOpenTimeoutFailure(firstError)) {
      throw firstError;
    }

    await recoverIndexedDbStore(endpoint, false);
    return {
      db: await openDbEndpoint(endpoint),
      recovered: true,
    };
  }
}

async function openDbEndpoint(endpoint: string): Promise<Surreal> {
  if (endpoint.startsWith("indxdb://") && indexedDbOpenInFlight) {
    if (indexedDbOpenInFlightEndpoint === endpoint) {
      return withTimeout(
        indexedDbOpenInFlight,
        endpointOpenTimeoutMs(endpoint),
        `open db endpoint ${indexedDbOpenInFlightEndpoint ?? endpoint}`,
      );
    }
  }

  const db = new Surreal({
    engines: {
      ...createRemoteEngines(),
      ...createWasmEngines(),
    },
  });

  let openOperation: Promise<Surreal> | null = null;

  try {
    const timeoutMs = endpointOpenTimeoutMs(endpoint);
    openOperation = (async () => {
      await db.connect(endpoint, {
        namespace: DB_NAMESPACE,
        database: DB_NAME,
      });

      await db.query(`
        DEFINE TABLE ${TABLE_TEMPORAL_NODE} SCHEMALESS;
        DEFINE TABLE ${TABLE_APP_CONFIG} SCHEMALESS;
        DEFINE TABLE ${TABLE_CALIBRATION} SCHEMALESS;
      `);

      const existingConfig = await selectAny(db, `${TABLE_APP_CONFIG}:singleton`);
      if (!existingConfig) {
        const seeded = readConfigFromLocalStorage() ?? { ...DEFAULT_CONFIG };
        await upsertAny(db, `${TABLE_APP_CONFIG}:singleton`, seeded);
        writeConfigToLocalStorage(seeded);
      } else {
        writeConfigToLocalStorage(normalizeConfig(existingConfig));
      }
      return db;
    })();

    if (endpoint.startsWith("indxdb://")) {
      indexedDbOpenInFlightEndpoint = endpoint;
      const trackedOpen = openOperation.finally(() => {
        indexedDbOpenInFlight = null;
        indexedDbOpenInFlightEndpoint = null;
      });
      // The tracking promise can outlive the caller. Attach a catch so rejected
      // IndexedDB attempts do not surface as unhandled promise rejections.
      trackedOpen.catch(() => undefined);
      indexedDbOpenInFlight = trackedOpen;
    }

    await withTimeout(openOperation, timeoutMs, `open db endpoint ${endpoint}`);

    return db;
  } catch (error) {
    // For IndexedDB endpoints, avoid forcing close while callbacks may still be in-flight.
    // This prevents wasm "closure invoked recursively or after being dropped" crashes.
    const isIndexedDbEndpoint = endpoint.startsWith("indxdb://");
    const shouldSkipClose = isIndexedDbEndpoint;

    if (!shouldSkipClose) {
      await db.close().catch(() => undefined);
    }
    throw error;
  }
}

async function selectAny(db: Surreal, target: string): Promise<unknown> {
  return (db as unknown as { select: (resource: string | StringRecordId) => Promise<unknown> }).select(toResource(target));
}

async function upsertAny(db: Surreal, target: string, content: unknown): Promise<void> {
  const upsertBuilder = (db as unknown as { upsert: (resource: string | StringRecordId) => { content: (value: Record<string, unknown>) => Promise<unknown> } }).upsert(toResource(target));
  await upsertBuilder.content(content as Record<string, unknown>);
}

function toResource(target: string): string | StringRecordId {
  const separator = target.indexOf(":");
  const looksLikeRecordId = separator > 0 && separator < target.length - 1;
  return looksLikeRecordId ? new StringRecordId(target) : target;
}

async function getDb(): Promise<Surreal> {
  if (!dbPromise) {
    dbPromise = connectDb().catch((error) => {
      dbPromise = null;
      throw error;
    });
  }

  const db = await dbPromise;

  if (storageMode === "mem" && ENABLE_INDEXEDDB_PROMOTION) {
    return promoteMemFallbackToIndexedDb(db);
  }

  return db;
}

async function promoteMemFallbackToIndexedDb(currentDb: Surreal): Promise<Surreal> {
  if (storageMode !== "mem") {
    return currentDb;
  }

  const now = Date.now();
  if (indexedDbPromotePromise || now - indexedDbPromoteLastAttemptAt < INDEXED_DB_PROMOTE_RETRY_MS) {
    return currentDb;
  }

  indexedDbPromoteLastAttemptAt = now;

  indexedDbPromotePromise = (async () => {
    try {
      await ensurePersistentStoragePreference();

      const seedConfig = await readConfig(currentDb).catch(() => defaultConfig());
      const seedNodes = await readAllNodes(currentDb).catch(() => []);

      let promoted: Surreal | null = null;
      let recovered = false;
      let promotedEndpoint = activeIndexedDbEndpoint;
      let promoteError: unknown = null;

      for (let index = 0; index < INDEXED_DB_ENDPOINT_CANDIDATES.length; index += 1) {
        const endpoint = INDEXED_DB_ENDPOINT_CANDIDATES[index];
        try {
          const attempt = await connectIndexedDbCandidate(endpoint);
          promoted = attempt.db;
          recovered = attempt.recovered || index > 0;
          promotedEndpoint = endpoint;
          break;
        } catch (error) {
          promoteError = error;
          lastIndexedDbError = errorToString(error);
        }
      }

      if (!promoted) {
        throw promoteError ?? new Error("failed to promote mem fallback to indexeddb");
      }

      await writeConfig(promoted, seedConfig).catch(() => undefined);

      for (const node of seedNodes) {
        await upsertLocalNode(promoted, node).catch(() => undefined);
      }

      storageMode = "indxdb";
      storageRecovered = recovered;
      activeIndexedDbEndpoint = promotedEndpoint;
      lastIndexedDbError = null;
      dbPromise = Promise.resolve(promoted);
      await currentDb.close().catch(() => undefined);
    } catch (error) {
      lastIndexedDbError = errorToString(error);
    } finally {
      indexedDbPromotePromise = null;
    }
  })();

  return currentDb;
}

async function readConfig(db: Surreal): Promise<AppConfig> {
  const existing = await selectAny(db, `${TABLE_APP_CONFIG}:singleton`);
  if (existing) {
    const normalized = normalizeConfig(existing);
    writeConfigToLocalStorage(normalized);
    return normalized;
  }

  const cached = readConfigFromLocalStorage();
  const seeded = cached ?? { ...DEFAULT_CONFIG };
  await upsertAny(db, `${TABLE_APP_CONFIG}:singleton`, seeded);
  writeConfigToLocalStorage(seeded);
  return seeded;
}

async function writeConfig(db: Surreal, config: AppConfig): Promise<void> {
  const normalized = normalizeConfig(config);
  await upsertAny(db, `${TABLE_APP_CONFIG}:singleton`, normalized);
  writeConfigToLocalStorage(normalized);
}

async function readAllNodes(db: Surreal): Promise<NodeDto[]> {
  const rows = await selectAny(db, TABLE_TEMPORAL_NODE);
  const values = Array.isArray(rows) ? rows : rows ? [rows] : [];

  return values
    .map(toNodeDto)
    .filter((node): node is NodeDto => node !== null)
    .map(normalizeNode);
}

async function upsertLocalNode(db: Surreal, node: NodeDto): Promise<void> {
  const normalized = normalizeNode(node);
  await upsertAny(db, recordIdForNode(normalized.syncKey), normalized);
}

async function hydrateLocalCacheFromRemote(db: Surreal | null, nodes: NodeDto[]): Promise<void> {
  if (!db || nodes.length === 0) {
    return;
  }

  for (const node of nodes) {
    try {
      await upsertLocalNode(db, node);
    } catch {
      // Ignore local cache hydration failures; rendering can still continue from remote data.
    }
  }
}

function byTimestampDesc(left: NodeDto, right: NodeDto): number {
  return right.timestamp.localeCompare(left.timestamp);
}

function extractFirstNumber(source: string, regex: RegExp): number | null {
  const match = source.match(regex);
  if (!match || !match[1]) {
    return null;
  }

  const value = Number(match[1]);
  return Number.isFinite(value) ? value : null;
}

function extractLastNumber(source: string, regex: RegExp): number | null {
  let value: number | null = null;
  let match: RegExpExecArray | null = null;
  const expression = new RegExp(regex.source, regex.flags.includes("g") ? regex.flags : `${regex.flags}g`);

  while (true) {
    match = expression.exec(source);
    if (!match) {
      break;
    }

    const parsed = Number(match[1]);
    if (Number.isFinite(parsed)) {
      value = parsed;
    }
  }

  return value;
}

function extractAvec(raw: string, field: "user_avec" | "model_avec" | "compression_avec"): AvecState | null {
  const marker = `${field}:`;
  const start = raw.toLowerCase().indexOf(marker);
  if (start === -1) {
    return null;
  }

  const section = raw.slice(start, Math.min(raw.length, start + 420));
  const stability = extractFirstNumber(section, /stability\s*:\s*([-+]?\d*\.?\d+)/i);
  const friction = extractFirstNumber(section, /friction\s*:\s*([-+]?\d*\.?\d+)/i);
  const logic = extractFirstNumber(section, /logic\s*:\s*([-+]?\d*\.?\d+)/i);
  const autonomy = extractFirstNumber(section, /autonomy\s*:\s*([-+]?\d*\.?\d+)/i);

  if (stability === null || friction === null || logic === null || autonomy === null) {
    return null;
  }

  const psi = extractFirstNumber(section, /\bpsi\s*:\s*([-+]?\d*\.?\d+)/i);
  return withPsi({ stability, friction, logic, autonomy }, psi ?? undefined);
}

function parseParentNodeId(raw: string): string | null {
  const match = raw.match(/parent_node(?:_id)?\s*:\s*([^,\n}]+)/i);
  if (!match || !match[1]) {
    return null;
  }

  const value = match[1]
    .trim()
    .replace(/^ref:/i, "")
    .replace(/^['\"]/, "")
    .replace(/['\"]$/, "");

  if (!value || value.toLowerCase() === "null") {
    return null;
  }

  return value;
}

function parseSttpNode(input: StoreContextInput): { node?: NodeDto; error?: string } {
  const raw = input.node.trim();
  if (!raw) {
    return { error: "ParseFailure: empty node" };
  }

  const requestedSessionId = input.sessionId.trim();
  if (!requestedSessionId) {
    return { error: "ParseFailure: session_id is required" };
  }

  if (!raw.includes("\u23e3")) {
    return { error: "ParseFailure: missing STTP node marker" };
  }

  const tierMatch = raw.match(/\btier\s*:\s*([a-zA-Z0-9_-]+)/i);
  const timestampMatch = raw.match(/\btimestamp\s*:\s*([0-9T:.-]+Z?)/i);
  const sessionMatch = raw.match(/\bsession_id\s*:\s*([a-zA-Z0-9._:-]+)/i);
  const compressionDepthMatch = raw.match(/\bcompression_depth\s*:\s*(-?\d+)/i);

  const tier = tierMatch?.[1]?.trim() ?? "raw";
  const timestamp = timestampMatch?.[1]?.trim() ?? nowIso();
  const nodeSessionId = sessionMatch?.[1]?.trim();
  if (nodeSessionId && nodeSessionId !== requestedSessionId) {
    return { error: `ParseFailure: session_id mismatch (${nodeSessionId} !== ${requestedSessionId})` };
  }

  const sessionId = nodeSessionId || requestedSessionId;
  const compressionDepth = Number(compressionDepthMatch?.[1] ?? "0");

  const userAvec = extractAvec(raw, "user_avec");
  const modelAvec = extractAvec(raw, "model_avec");
  const compressionAvec = extractAvec(raw, "compression_avec");

  if (!userAvec || !modelAvec) {
    return { error: "ParseFailure: missing user_avec or model_avec" };
  }

  const rho = extractLastNumber(raw, /\brho\s*:\s*([-+]?\d*\.?\d+)/i) ?? 0;
  const kappa = extractLastNumber(raw, /\bkappa\s*:\s*([-+]?\d*\.?\d+)/i) ?? 0;
  const psi = extractLastNumber(raw, /\bpsi\s*:\s*([-+]?\d*\.?\d+)/i) ?? userAvec.psi;

  const parentNodeId = parseParentNodeId(raw);

  const provisional: NodeDto = {
    raw,
    sessionId,
    tier,
    timestamp,
    compressionDepth: Number.isFinite(compressionDepth) ? Math.trunc(compressionDepth) : 0,
    parentNodeId,
    userAvec,
    modelAvec,
    compressionAvec,
    rho,
    kappa,
    psi,
    syncKey: "",
    syntheticId: "",
  };

  const normalized = normalizeNode(provisional);
  return { node: normalized };
}

function normalizeChatRole(role: string): ChatMessageRole | null {
  if (role === "system" || role === "user" || role === "assistant") {
    return role;
  }

  return null;
}

function normalizeChatMessages(messages: ChatMessage[], options: { includeSystem: boolean }): ChatMessage[] {
  return messages
    .map((message) => {
      const role = normalizeChatRole(String(message.role));
      const content = String(message.content ?? "").trim();
      if (!role || !content) {
        return null;
      }

      if (!options.includeSystem && role === "system") {
        return null;
      }

      return {
        role,
        content,
      };
    })
    .filter((message): message is ChatMessage => message !== null);
}

function toConversationTranscript(messages: ChatMessage[]): string {
  return messages
    .map((message) => `${message.role.toUpperCase()}: ${message.content}`)
    .join("\n\n")
    .trim();
}

function stripMarkdownFence(text: string): string {
  const trimmed = text.trim();
  if (!trimmed.startsWith("```")) {
    return trimmed;
  }

  const lines = trimmed.split(/\r?\n/);
  if (lines.length === 0) {
    return "";
  }

  const body: string[] = [];
  for (let index = 1; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.trim().startsWith("```")) {
      break;
    }
    body.push(line);
  }

  return body.join("\n").trim();
}

function normalizeModelNodeCandidate(text: string): string {
  const unfenced = stripMarkdownFence(text);
  const startMarkers = ["⊕⟨", "⦿⟨", "◈⟨", "⍉⟨", "⏣"];
  const markerIndex = startMarkers
    .map((marker) => unfenced.indexOf(marker))
    .filter((index) => index >= 0)
    .reduce((min, index) => (index < min ? index : min), Number.POSITIVE_INFINITY);

  if (Number.isFinite(markerIndex)) {
    return unfenced.slice(markerIndex).trim();
  }

  return unfenced.trim();
}

function buildEncodePrompt(
  sessionId: string,
  parserErrorHint?: string,
  previousNodeCandidate?: string,
): string {
  const expectedTimestampUtc = new Date().toISOString();
  const parts = [
    `session_id: ${sessionId}`,
    `expected_timestamp_utc: ${expectedTimestampUtc}`,
    "",
    "Use expected_timestamp_utc as the ⦿ timestamp field unless the conversation itself provides a stronger explicit timestamp.",
    "Use the full prior chat history in this request as source context.",
    "Encode the conversation above into exactly one valid STTP node.",
  ];

  const errorHint = parserErrorHint?.trim();
  if (errorHint) {
    parts.push(
      "",
      "Parser feedback from previous attempt:",
      errorHint,
      "Use this feedback to repair the node while preserving conversation meaning.",
    );
  }

  const previousCandidate = previousNodeCandidate?.trim();
  if (previousCandidate) {
    parts.push(
      "",
      "Previous node candidate to repair:",
      previousCandidate,
    );
  }

  parts.push(
    "",
    "Return only the node text.",
  );

  return parts.join("\n");
}

async function runOllamaChat(config: AppConfig, messages: ChatMessage[]): Promise<string | null> {
  const payload: OllamaChatRequest = {
    model: config.ollamaModel,
    messages,
    stream: false,
  };

  const ollamaUrl = joinUrl(config.ollamaBaseUrl, "/api/chat");
  const requestUrls = toGatewayRequestUrls(ollamaUrl);
  let lastError: unknown = null;

  for (const requestUrl of requestUrls) {
    try {
      const response = await fetch(requestUrl, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(payload),
      });

      if (!response.ok) {
        const body = await response.text().catch(() => "");
        if (requestUrl.includes(DEV_GATEWAY_PROXY_PATH)) {
          lastError = new Error(`ollama proxy request failed: ${response.status} ${response.url} ${body}`.trim());
          continue;
        }

        throw new Error(`ollama response status failed: ${response.status} ${body}`.trim());
      }

      const parsed = (await response.json().catch(() => ({}))) as OllamaChatResponse;
      const text = parsed.message?.content?.trim();
      return text ? text : null;
    } catch (error) {
      lastError = error;
    }
  }

  if (isLoopbackUrl(ollamaUrl) && isHostedBrowserOrigin()) {
    throw createLocalModelReachabilityError(ollamaUrl);
  }

  throw new Error(`ollama request failed: ${errorToString(lastError)}`);
}

function parseAiResponse(text: string): AiSummary | null {
  const cleaned = text
    .replace(/Thinking\.\.\.[\s\S]*?\.\.\.done thinking\./gi, "")
    .trim();

  if (!cleaned) {
    return null;
  }

  const labels = [
    "Topic",
    "What happened",
    "Where we left off",
    "Vibe",
    "Pick back up with",
  ] as const;

  const normalized = labels.reduce((acc, label) => {
    const regex = new RegExp(`^\\s*(?:[-*]\\s*)?\\*\\*?${label}\\*\\*?\\s*:`, "gim");
    return acc.replace(regex, `${label}:`);
  }, cleaned);

  const lower = normalized.toLowerCase();
  const indices = labels
    .map((label) => ({
      label,
      index: lower.indexOf(`${label.toLowerCase()}:`),
    }))
    .filter((entry) => entry.index >= 0)
    .sort((left, right) => left.index - right.index);

  const readSection = (label: (typeof labels)[number]): string => {
    const current = indices.find((entry) => entry.label === label);
    if (!current) {
      return "";
    }

    const start = current.index + label.length + 1;
    const end = indices
      .filter((entry) => entry.index > current.index)
      .map((entry) => entry.index)
      .reduce((min, value) => Math.min(min, value), normalized.length);

    return normalized.slice(start, end).trim().replace(/^\*+|\*+$/g, "").trim();
  };

  const topic = readSection("Topic");
  const whatHappened = readSection("What happened");
  const whereWeLeftOff = readSection("Where we left off");
  const vibe = readSection("Vibe");
  const pickBackUpWith = readSection("Pick back up with");

  if (!topic && !whatHappened) {
    const firstLine = normalized.split(/\r?\n/)[0]?.trim().replace(/:$/, "") || "transmutation";
    return {
      topic: firstLine,
      whatHappened: normalized,
      whereWeLeftOff: "",
      vibe: "",
      pickBackUpWith: "",
    };
  }

  return {
    topic,
    whatHappened,
    whereWeLeftOff,
    vibe,
    pickBackUpWith,
  };
}

function parseGatewayStoreOutcome(value: unknown): GatewayStoreOutcome {
  const payload = asRecord(value);
  const result = asRecord(payload.result ?? payload);

  const upsertStatus = readString(result, "upsertStatus", "upsert_status").toLowerCase();
  const duplicateFromStatus = upsertStatus === "duplicate" || upsertStatus === "skipped";
  const duplicate = (result.duplicateSkipped as boolean | undefined) ?? duplicateFromStatus;
  const validationError = readString(result, "validationError", "validation_error", "error") || null;

  const validField = result.valid;
  const valid = typeof validField === "boolean" ? validField : validationError === null;

  return {
    valid,
    duplicate,
    validationError,
  };
}

async function fetchGatewayWithFallback(urls: string[], init?: RequestInit): Promise<Response> {
  let lastError: unknown = null;

  for (let index = 0; index < urls.length; index += 1) {
    const url = urls[index];
    const requestUrls = toGatewayRequestUrls(url);

    for (let requestIndex = 0; requestIndex < requestUrls.length; requestIndex += 1) {
      const requestUrl = requestUrls[requestIndex];
      try {
        const response = await fetch(requestUrl, init);
        if (response.status === 404 && (requestIndex < requestUrls.length - 1 || index < urls.length - 1)) {
          continue;
        }

        return response;
      } catch (error) {
        lastError = error;
        if (requestIndex < requestUrls.length - 1) {
          continue;
        }

        if (index === urls.length - 1) {
          if (isLikelyCorsError(error)) {
            throw createCorsHelpError(url);
          }

          throw error;
        }
      }
    }
  }

  if (urls.length > 0 && isLikelyCorsError(lastError)) {
    throw createCorsHelpError(urls[0]);
  }

  throw new Error(`gateway request failed: ${errorToString(lastError)}`);
}

function createGatewayHeaders(authToken?: string, includeJson = false): HeadersInit {
  const headers: Record<string, string> = {};
  if (includeJson) {
    headers["Content-Type"] = "application/json";
  }

  const token = normalizeGatewayAuthToken(authToken ?? "");
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }

  return headers;
}

async function storeNodeToGateway(baseUrl: string, node: NodeDto, authToken?: string): Promise<GatewayStoreOutcome> {
  const urls = gatewayPathsFor(baseUrl, GATEWAY_STORE_PATHS);
  const response = await fetchGatewayWithFallback(urls, {
    method: "POST",
    headers: createGatewayHeaders(authToken, true),
    body: JSON.stringify({
      node: node.raw,
      sessionId: node.sessionId,
    }),
  });

  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`gateway store failed: ${response.status} ${response.url} ${body}`.trim());
  }

  const payload = await response.json().catch(() => ({}));
  return parseGatewayStoreOutcome(payload);
}

function toGatewayNode(value: unknown): NodeDto | null {
  return toNodeDto(value);
}

async function fetchGatewayNodes(baseUrl: string, sessionId?: string, authToken?: string): Promise<NodeDto[]> {
  const urls = gatewayPathsFor(baseUrl, GATEWAY_NODES_PATHS).map((candidate) => {
    const url = new URL(candidate);
    url.searchParams.set("limit", "5000");
    if (sessionId && sessionId.trim()) {
      url.searchParams.set("sessionId", sessionId.trim());
    }

    return url.toString();
  });

  const response = await fetchGatewayWithFallback(urls, {
    headers: createGatewayHeaders(authToken),
  });
  if (!response.ok) {
    const body = await response.text().catch(() => "");
    throw new Error(`gateway list nodes failed: ${response.status} ${response.url} ${body}`.trim());
  }

  const payload = asRecord(await response.json().catch(() => ({})));
  const nodes = Array.isArray(payload.nodes) ? payload.nodes : [];

  return nodes
    .map(toGatewayNode)
    .filter((node): node is NodeDto => node !== null)
    .map(normalizeNode);
}

function toDriftClassification(delta: number): string {
  return delta > 0.6 ? "Uncontrolled" : "Intentional";
}

function calibrationDelta(left: AvecState, right: AvecState): number {
  const delta = Math.sqrt(
    (left.stability - right.stability) ** 2 +
      (left.friction - right.friction) ** 2 +
      (left.logic - right.logic) ** 2 +
      (left.autonomy - right.autonomy) ** 2,
  );

  return Number.isFinite(delta) ? delta : 0;
}

async function readCalibrationState(db: Surreal, sessionId: string): Promise<CalibrationStateRecord | null> {
  const record = await selectAny(db, recordIdForCalibration(sessionId));
  if (!record) {
    return null;
  }

  const source = asRecord(record);
  const historyRaw = source.triggerHistory;

  return {
    sessionId,
    currentAvec: normalizeAvec(readObject(source, "currentAvec", "current_avec")),
    triggerHistory: Array.isArray(historyRaw)
      ? historyRaw.filter((item): item is string => typeof item === "string")
      : [],
    updatedAt: readString(source, "updatedAt", "updated_at") || nowIso(),
  };
}

export function createWebResonantiaClient(): ResonantiaClient {
  return {
    async getHealth(): Promise<HealthResponse> {
      const db = await getDb();
      await readConfig(db);
      return {
        status: "ok",
        transport: transportLabel(),
      };
    },

    async getConfig(): Promise<AppConfig> {
      const result = await readConfigBestEffort();
      return result.config;
    },

    async getComposeEncodePreamble(): Promise<string> {
      return COMPOSE_ENCODE_PREAMBLE;
    },

    async listNodes(limit: number, sessionId?: string): Promise<ListNodesResponse> {
      const cappedLimit = clamp(Math.trunc(limit), 1, 400);
      const sessionFilter = sessionId?.trim();
      const cachedNodes = readNodesCacheFromLocalStorage()
        .filter((node) => !sessionFilter || node.sessionId === sessionFilter)
        .sort(byTimestampDesc);

      let db: Surreal | null = null;
      let localNodes: NodeDto[] = [];
      let localError: unknown = null;

      try {
        db = await getDb();
        localNodes = (await readAllNodes(db))
          .filter((node) => !sessionFilter || node.sessionId === sessionFilter)
          .sort(byTimestampDesc);
      } catch (error) {
        localError = error;
      }

      if (localNodes.length > 0) {
        writeNodesCacheToLocalStorage(localNodes);
        return {
          nodes: localNodes.slice(0, cappedLimit),
          retrieved: Math.min(localNodes.length, cappedLimit),
          source: localReadSourceLabel(),
          transport: transportLabel(),
        };
      }

      const cachedConfig = readConfigFromLocalStorage();
      const fallbackGateway = normalizeGatewayBaseUrl(cachedConfig?.gatewayBaseUrl ?? "");

      if (fallbackGateway) {
        try {
          const remoteNodes = (await fetchGatewayNodes(fallbackGateway, sessionFilter)).sort(byTimestampDesc);
          const scoped = remoteNodes.slice(0, cappedLimit);

          writeNodesCacheToLocalStorage(remoteNodes);
          await hydrateLocalCacheFromRemote(db, remoteNodes);

          return {
            nodes: scoped,
            retrieved: scoped.length,
            source: "cloud-gateway",
            transport: transportLabel(),
          };
        } catch (gatewayError) {
          if (cachedNodes.length > 0) {
            const scoped = cachedNodes.slice(0, cappedLimit);
            return {
              nodes: scoped,
              retrieved: scoped.length,
              source: "fallback-cache",
              transport: transportLabel(),
            };
          }

          if (localError) {
            throw localError;
          }

          throw gatewayError;
        }
      }

      if (cachedNodes.length > 0) {
        const scoped = cachedNodes.slice(0, cappedLimit);
        return {
          nodes: scoped,
          retrieved: scoped.length,
          source: "fallback-cache",
          transport: transportLabel(),
        };
      }

      if (localError) {
        throw localError;
      }

      return {
        nodes: [],
        retrieved: 0,
        source: localReadSourceLabel(),
        transport: transportLabel(),
      };
    },

    async getGraph(limit: number, sessionId?: string): Promise<GraphResponse> {
      const listed = await this.listNodes(limit, sessionId);
      return buildGraph(listed.nodes);
    },

    async storeContext(input: StoreContextInput): Promise<StoreContextResponse> {
      const sessionId = input.sessionId.trim();
      if (!sessionId) {
        return {
          nodeId: "",
          psi: 0,
          valid: false,
          validationError: "SessionIdRequired: session_id is required",
          duplicateSkipped: false,
          upsertStatus: undefined,
        };
      }

      const db = await getDb();
      const parsed = parseSttpNode(input);
      if (!parsed.node) {
        return {
          nodeId: "",
          psi: 0,
          valid: false,
          validationError: parsed.error ?? "ParseFailure: unable to parse node",
          duplicateSkipped: false,
          upsertStatus: undefined,
        };
      }

      const node = normalizeNode(parsed.node);
      const existing = (await readAllNodes(db)).find((entry) => entry.syncKey === node.syncKey);
      if (existing) {
        writeNodesCacheToLocalStorage(mergeNodesBySyncKey(readNodesCacheFromLocalStorage(), [existing]));
        return {
          nodeId: existing.syntheticId,
          psi: existing.psi,
          valid: true,
          validationError: null,
          duplicateSkipped: true,
          upsertStatus: "duplicate",
        };
      }

      await upsertLocalNode(db, node);
      writeNodesCacheToLocalStorage(mergeNodesBySyncKey(readNodesCacheFromLocalStorage(), [node]));

      return {
        nodeId: node.syntheticId,
        psi: node.psi,
        valid: true,
        validationError: null,
        duplicateSkipped: false,
        upsertStatus: "created",
      };
    },

    async syncPull(request: SyncPullRequest): Promise<SyncPullCommandResponse> {
      const synced = await this.syncNow({
        sessionId: request.sessionId,
        gatewayBaseUrl: request.gatewayBaseUrl,
        gatewayAuthToken: request.gatewayAuthToken,
        pageSize: request.pageSize,
        maxBatches: request.maxBatches,
      });

      return {
        source: request.source ?? "gateway",
        fetched: synced.download.fetched,
        created: synced.download.created,
        updated: synced.download.updated,
        duplicate: synced.download.duplicate,
        skipped: synced.download.skipped,
        filtered: synced.download.filtered,
        batches: synced.download.batches,
        hasMore: synced.download.hasMore,
      };
    },

    async syncNow(request: SyncNowRequest = {}): Promise<SyncNowResponse> {
      const db = await getDb();
      const config = await readConfig(db);

      const sessionFilter = request.sessionId?.trim();
      const gatewayBaseUrl = normalizeGatewayBaseUrl(request.gatewayBaseUrl?.trim() || config.gatewayBaseUrl || "");
      const gatewayAuthToken = normalizeGatewayAuthToken(
        request.gatewayAuthToken?.trim() || config.gatewayAuthToken || "",
      );

      if (!gatewayBaseUrl) {
        throw new Error("cloud sync path not set. open settings -> advanced sync once, then sync is one-click.");
      }

      let localNodes: NodeDto[] = [];
      try {
        localNodes = await readAllNodes(db);
      } catch {
        const cachedNodes = readNodesCacheFromLocalStorage();
        if (cachedNodes.length > 0) {
          localNodes = cachedNodes;
          await hydrateLocalCacheFromRemote(db, cachedNodes);
        }
      }

      const scopedLocalNodes = sessionFilter
        ? localNodes.filter((node) => node.sessionId === sessionFilter)
        : localNodes;

      const remoteBeforeUpload = await fetchGatewayNodes(gatewayBaseUrl, sessionFilter, gatewayAuthToken);
      const remoteKnownKeys = new Set(remoteBeforeUpload.map((node) => node.syncKey));

      const upload = {
        uploaded: 0,
        duplicate: 0,
        skipped: 0,
        rejected: 0,
        batches: scopedLocalNodes.length > 0 ? 1 : 0,
        hasMore: false,
      };

      for (const node of scopedLocalNodes) {
        if (remoteKnownKeys.has(node.syncKey)) {
          upload.skipped += 1;
          continue;
        }

        const outcome = await storeNodeToGateway(gatewayBaseUrl, node, gatewayAuthToken);
        if (!outcome.valid) {
          upload.rejected += 1;
          continue;
        }

        upload.uploaded += 1;
        if (outcome.duplicate) {
          upload.duplicate += 1;
        }
      }

      const remoteNodes = await fetchGatewayNodes(gatewayBaseUrl, sessionFilter, gatewayAuthToken);
      const download = {
        fetched: remoteNodes.length,
        created: 0,
        updated: 0,
        duplicate: 0,
        skipped: 0,
        filtered: 0,
        batches: remoteNodes.length > 0 ? 1 : 0,
        hasMore: false,
      };

      const localBySync = new Map(localNodes.map((node) => [node.syncKey, node]));

      for (const remoteNode of remoteNodes) {
        const normalized = normalizeNode(remoteNode);
        const existing = localBySync.get(normalized.syncKey);

        if (!existing) {
          await upsertLocalNode(db, normalized);
          localBySync.set(normalized.syncKey, normalized);
          download.created += 1;
          continue;
        }

        if (existing.raw === normalized.raw) {
          download.duplicate += 1;
          continue;
        }

        await upsertLocalNode(db, normalized);
        localBySync.set(normalized.syncKey, normalized);
        download.updated += 1;
      }

      writeNodesCacheToLocalStorage(Array.from(localBySync.values()));

      return {
        sessionId: sessionFilter || "all",
        remoteBaseUrl: gatewayBaseUrl,
        upload,
        download,
      };
    },

    async calibrateSession(input: CalibrateSessionInput): Promise<CalibrateSessionResponse> {
      const db = await getDb();
      const sessionId = input.sessionId.trim();
      if (!sessionId) {
        throw new Error("session id is required for calibration");
      }

      const target = withPsi({
        stability: input.stability,
        friction: input.friction,
        logic: input.logic,
        autonomy: input.autonomy,
      });

      const existing = await readCalibrationState(db, sessionId);
      const isFirstCalibration = existing === null;
      const previous = existing?.currentAvec ?? target;
      const delta = isFirstCalibration ? 0 : calibrationDelta(previous, target);

      const triggerHistory = [...(existing?.triggerHistory ?? []), input.trigger].slice(-20);

      await upsertAny(db, recordIdForCalibration(sessionId), {
        sessionId,
        currentAvec: target,
        triggerHistory,
        updatedAt: nowIso(),
      });

      return {
        previousAvec: previous,
        delta,
        driftClassification: toDriftClassification(delta),
        trigger: input.trigger,
        triggerHistory,
        isFirstCalibration,
      };
    },

    async chatCompose(request: ComposeChatRequest): Promise<string | null> {
      const db = await getDb();
      const config = await readConfig(db);
      const conversation = normalizeChatMessages(request.messages, { includeSystem: false });
      if (conversation.length === 0) {
        return null;
      }

      return runOllamaChat(config, [
        { role: "system", content: COMPOSE_CHAT_PREAMBLE },
        ...conversation,
      ]);
    },

    async encodeCompose(request: EncodeComposeRequest): Promise<string> {
      const db = await getDb();
      const config = await readConfig(db);
      const sessionId = request.sessionId.trim();
      if (!sessionId) {
        throw new Error("session id is required for encode");
      }

      const conversation = normalizeChatMessages(request.messages, { includeSystem: false });
      if (conversation.length === 0) {
        throw new Error("encode requires at least one chat message");
      }

      const text = await runOllamaChat(config, [
        { role: "system", content: COMPOSE_ENCODE_PREAMBLE },
        ...conversation,
        {
          role: "user",
          content: buildEncodePrompt(
            sessionId,
            request.parserErrorHint,
            request.previousNodeCandidate,
          ),
        },
      ]);

      if (!text) {
        throw new Error("model returned an empty encode response");
      }

      const normalized = normalizeModelNodeCandidate(text);
      if (!normalized.includes("⏣")) {
        throw new Error("encode response did not include an STTP node marker");
      }

      return normalized;
    },

    async summarizeNode(rawNode: string): Promise<AiSummary | null> {
      const db = await getDb();
      const config = await readConfig(db);

      const text = await runOllamaChat(config, [
        { role: "system", content: TRANSMUTE_PREAMBLE },
        { role: "user", content: rawNode },
      ]);
      if (!text) {
        return null;
      }

      return parseAiResponse(text);
    },

    async setOllamaConfig(baseUrl?: string, model?: string): Promise<void> {
      const { config: current, db } = await readConfigBestEffort();
      const next: AppConfig = {
        ...current,
        ollamaBaseUrl: baseUrl ?? current.ollamaBaseUrl,
        ollamaModel: model ?? current.ollamaModel,
      };

      writeConfigToLocalStorage(normalizeConfig(next));
      if (db) {
        await writeConfig(db, next);
      }
    },

    async setGatewayBaseUrl(baseUrl: string): Promise<void> {
      const { config: current, db } = await readConfigBestEffort();
      const next: AppConfig = {
        ...current,
        gatewayBaseUrl: normalizeGatewayBaseUrl(baseUrl),
      };

      writeConfigToLocalStorage(normalizeConfig(next));
      if (db) {
        await writeConfig(db, next);
      }
    },

    async setGatewayAuthToken(token: string): Promise<void> {
      const { config: current, db } = await readConfigBestEffort();
      const next: AppConfig = {
        ...current,
        gatewayAuthToken: normalizeGatewayAuthToken(token),
      };

      writeConfigToLocalStorage(normalizeConfig(next));
      if (db) {
        await writeConfig(db, next);
      }
    },
  };
}
