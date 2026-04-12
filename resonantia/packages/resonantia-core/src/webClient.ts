import { createWasmEngines } from "@surrealdb/wasm";
import { StringRecordId, Surreal, createRemoteEngines } from "surrealdb";
import type {
  AppConfig,
  CalibrateSessionInput,
  HealthResponse,
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

const DB_ENDPOINT = "indxdb://resonantia-local";
const MEM_FALLBACK_ENDPOINT = "mem://";
const DB_NAMESPACE = "resonantia";
const DB_NAME = "local";
const APP_CONFIG_STORAGE_KEY = "resonantia:app-config:v1";
const NODE_CACHE_STORAGE_KEY = "resonantia:nodes-cache:v1";
const NODE_CACHE_LIMIT = 1200;

const DEFAULT_GATEWAY_BASE_URL = "";
const DEFAULT_OLLAMA_BASE_URL = "http://localhost:11434";
const DEFAULT_OLLAMA_MODEL = "gemma3";
const GATEWAY_STORE_PATHS = ["/api/v1/store", "/api/store", "/store"];
const GATEWAY_NODES_PATHS = ["/api/v1/nodes", "/api/nodes", "/nodes"];
const DEV_GATEWAY_PROXY_PATH = "/__gateway_proxy__";

const TABLE_TEMPORAL_NODE = "temporal_node";
const TABLE_APP_CONFIG = "app_config";
const TABLE_CALIBRATION = "calibration_state";

const DEFAULT_CONFIG: AppConfig = {
  gatewayBaseUrl: DEFAULT_GATEWAY_BASE_URL,
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

let dbPromise: Promise<Surreal> | null = null;
let storageMode: StorageMode = "indxdb";
let storageRecovered = false;

const INDEXED_DB_HINT = DB_ENDPOINT.replace(/^indxdb:\/\//i, "").split(/[/?#]/)[0] || "resonantia-local";
const INDEXED_DB_ERROR_MARKERS = [
  "indexeddb",
  "key-value store",
  "key value store",
  "invalidstateerror",
  "versionerror",
  "quotaexceedederror",
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

function isIndexedDbFailure(error: unknown): boolean {
  const normalized = errorToString(error).toLowerCase();
  if (normalized.includes("internalerror")) {
    return normalized.includes("indexeddb") || normalized.includes("key-value store") || normalized.includes("key value store");
  }

  return INDEXED_DB_ERROR_MARKERS.some((marker) => normalized.includes(marker));
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

async function recoverIndexedDbStore(): Promise<void> {
  if (typeof indexedDB === "undefined") {
    return;
  }

  const names = new Set<string>([INDEXED_DB_HINT]);
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
      if (lower.includes(INDEXED_DB_HINT.toLowerCase())) {
        names.add(entry.name);
      }
    }
  }

  for (const name of names) {
    await deleteIndexedDb(name).catch(() => undefined);
  }
}

function transportLabel(): string {
  if (storageMode === "mem") {
    return "surrealdb wasm (mem fallback, non-persistent)";
  }

  if (storageRecovered) {
    return "surrealdb wasm (indxdb local, recovered)";
  }

  return "surrealdb wasm (indxdb local)";
}

function withSlash(baseUrl: string): string {
  return baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`;
}

function joinUrl(baseUrl: string, path: string): string {
  const url = new URL(path.replace(/^\/+/, ""), withSlash(baseUrl));
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
    return trimTrailingSlash(trimmed);
  }
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

function defaultConfig(): AppConfig {
  return {
    gatewayBaseUrl: DEFAULT_GATEWAY_BASE_URL,
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
    if (target.origin === window.location.origin) {
      return [target.toString()];
    }

    const proxy = new URL(DEV_GATEWAY_PROXY_PATH, window.location.origin);
    proxy.searchParams.set("target", target.toString());
    return [proxy.toString(), target.toString()];
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
  const gatewayRaw = readString(record, "gatewayBaseUrl", "gateway_base_url") || DEFAULT_GATEWAY_BASE_URL;

  return {
    gatewayBaseUrl: normalizeGatewayBaseUrl(gatewayRaw),
    ollamaBaseUrl: readString(record, "ollamaBaseUrl", "ollama_base_url") || DEFAULT_OLLAMA_BASE_URL,
    ollamaModel: readString(record, "ollamaModel", "ollama_model") || DEFAULT_OLLAMA_MODEL,
    layoutOverrides: {
      sessionOverrides: readObject(layoutOverrides, "sessionOverrides", "session_overrides") as Record<string, { x: number; y: number }>,
      nodeOverrides: readObject(layoutOverrides, "nodeOverrides", "node_overrides") as Record<string, { x: number; y: number }>,
    },
  };
}

async function connectDb(): Promise<Surreal> {
  async function open(endpoint: string): Promise<Surreal> {
    const db = new Surreal({
      engines: {
        ...createRemoteEngines(),
        ...createWasmEngines(),
      },
    });

    await db.connect(endpoint);
    await db.use({ namespace: DB_NAMESPACE, database: DB_NAME });

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
  }

  try {
    const db = await open(DB_ENDPOINT);
    storageMode = "indxdb";
    storageRecovered = false;
    return db;
  } catch (firstError) {
    if (!isIndexedDbFailure(firstError)) {
      throw firstError;
    }

    await recoverIndexedDbStore();

    try {
      const recovered = await open(DB_ENDPOINT);
      storageMode = "indxdb";
      storageRecovered = true;
      return recovered;
    } catch {
      const fallback = await open(MEM_FALLBACK_ENDPOINT);
      storageMode = "mem";
      storageRecovered = false;
      return fallback;
    }
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

  return dbPromise;
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

  if (!raw.includes("\u23e3")) {
    return { error: "ParseFailure: missing STTP node marker" };
  }

  const tierMatch = raw.match(/\btier\s*:\s*([a-zA-Z0-9_-]+)/i);
  const timestampMatch = raw.match(/\btimestamp\s*:\s*([0-9T:.-]+Z?)/i);
  const sessionMatch = raw.match(/\bsession_id\s*:\s*([a-zA-Z0-9._:-]+)/i);
  const compressionDepthMatch = raw.match(/\bcompression_depth\s*:\s*(-?\d+)/i);

  const tier = tierMatch?.[1]?.trim() ?? "raw";
  const timestamp = timestampMatch?.[1]?.trim() ?? nowIso();
  const sessionId = sessionMatch?.[1]?.trim() || input.sessionId.trim() || "resonantia-local";
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

  const validField = result.valid;
  const valid = typeof validField === "boolean" ? validField : true;

  const validationError = readString(result, "validationError", "validation_error", "error") || null;

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

async function storeNodeToGateway(baseUrl: string, node: NodeDto): Promise<GatewayStoreOutcome> {
  const urls = gatewayPathsFor(baseUrl, GATEWAY_STORE_PATHS);
  const response = await fetchGatewayWithFallback(urls, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
    },
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

async function fetchGatewayNodes(baseUrl: string, sessionId?: string): Promise<NodeDto[]> {
  const urls = gatewayPathsFor(baseUrl, GATEWAY_NODES_PATHS).map((candidate) => {
    const url = new URL(candidate);
    url.searchParams.set("limit", "5000");
    if (sessionId && sessionId.trim()) {
      url.searchParams.set("sessionId", sessionId.trim());
    }

    return url.toString();
  });

  const response = await fetchGatewayWithFallback(urls);
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

    async listNodes(limit: number, sessionId?: string): Promise<{ nodes: NodeDto[]; retrieved: number }> {
      const cappedLimit = clamp(Math.trunc(limit), 1, 400);
      const sessionFilter = sessionId?.trim();

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
        };
      }

      const cachedConfig = readConfigFromLocalStorage();
      const fallbackGateway = normalizeGatewayBaseUrl(cachedConfig?.gatewayBaseUrl ?? "");

      if (fallbackGateway) {
        const remoteNodes = (await fetchGatewayNodes(fallbackGateway, sessionFilter))
          .sort(byTimestampDesc)
          .slice(0, cappedLimit);

        writeNodesCacheToLocalStorage(remoteNodes);
        await hydrateLocalCacheFromRemote(db, remoteNodes);

        return {
          nodes: remoteNodes,
          retrieved: remoteNodes.length,
        };
      }

      const cachedNodes = readNodesCacheFromLocalStorage()
        .filter((node) => !sessionFilter || node.sessionId === sessionFilter)
        .slice(0, cappedLimit);

      if (cachedNodes.length > 0) {
        return {
          nodes: cachedNodes,
          retrieved: cachedNodes.length,
        };
      }

      if (localError) {
        throw localError;
      }

      return {
        nodes: [],
        retrieved: 0,
      };
    },

    async getGraph(limit: number, sessionId?: string): Promise<GraphResponse> {
      const listed = await this.listNodes(limit, sessionId);
      return buildGraph(listed.nodes);
    },

    async storeContext(input: StoreContextInput): Promise<StoreContextResponse> {
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

      if (!gatewayBaseUrl) {
        throw new Error("cloud sync path not set. open settings -> advanced sync once, then sync is one-click.");
      }

      const localNodes = await readAllNodes(db);
      const scopedLocalNodes = sessionFilter
        ? localNodes.filter((node) => node.sessionId === sessionFilter)
        : localNodes;

      const upload = {
        uploaded: 0,
        duplicate: 0,
        rejected: 0,
        batches: scopedLocalNodes.length > 0 ? 1 : 0,
        hasMore: false,
      };

      for (const node of scopedLocalNodes) {
        const outcome = await storeNodeToGateway(gatewayBaseUrl, node);
        if (!outcome.valid) {
          upload.rejected += 1;
          continue;
        }

        upload.uploaded += 1;
        if (outcome.duplicate) {
          upload.duplicate += 1;
        }
      }

      const remoteNodes = await fetchGatewayNodes(gatewayBaseUrl, sessionFilter);
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

      return {
        sessionId: sessionFilter || "all",
        remoteBaseUrl: gatewayBaseUrl,
        upload,
        download,
      };
    },

    async calibrateSession(input: CalibrateSessionInput): Promise<CalibrateSessionResponse> {
      const db = await getDb();
      const sessionId = input.sessionId.trim() || "resonantia-local";
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

    async summarizeNode(rawNode: string): Promise<AiSummary | null> {
      const db = await getDb();
      const config = await readConfig(db);

      const response = await fetch(joinUrl(config.ollamaBaseUrl, "/api/chat"), {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          model: config.ollamaModel,
          messages: [{ role: "user", content: rawNode }],
          stream: false,
        }),
      });

      if (!response.ok) {
        const body = await response.text().catch(() => "");
        throw new Error(`ollama response status failed: ${response.status} ${body}`.trim());
      }

      const payload = (await response.json().catch(() => ({}))) as OllamaChatResponse;
      const text = payload.message?.content?.trim();
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
  };
}
