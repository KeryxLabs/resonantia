export interface AvecState {
  stability: number;
  friction: number;
  logic: number;
  autonomy: number;
  psi: number;
}

export interface GraphSessionDto {
  id: string;
  label: string;
  nodeCount: number;
  avgPsi: number;
  lastModified: string;
  size: number;
}

export interface GraphNodeDto {
  id: string;
  sessionId: string;
  label: string;
  tier: string;
  timestamp: string;
  psi: number;
  parentNodeId: string | null;
  size: number;
  syntheticId: string;
}

export interface GraphEdgeDto {
  id: string;
  source: string;
  target: string;
  kind: string;
}

export interface GraphResponse {
  sessions: GraphSessionDto[];
  nodes: GraphNodeDto[];
  edges: GraphEdgeDto[];
  retrieved: number;
}

export interface NodeDto {
  raw: string;
  sessionId: string;
  tier: string;
  timestamp: string;
  compressionDepth: number;
  parentNodeId: string | null;
  userAvec: AvecState;
  modelAvec: AvecState;
  compressionAvec: AvecState | null;
  rho: number;
  kappa: number;
  psi: number;
  syncKey: string;
  syntheticId: string;
}

export interface StoreContextResponse {
  nodeId: string;
  psi: number;
  valid: boolean;
  validationError: string | null;
  duplicateSkipped?: boolean;
  upsertStatus?: 'created' | 'updated' | 'duplicate' | 'skipped';
}

export interface SyncPullCommandResponse {
  source: string;
  fetched: number;
  created: number;
  updated: number;
  duplicate: number;
  skipped: number;
  filtered: number;
  batches: number;
  hasMore: boolean;
}

export interface SyncUploadStats {
  uploaded: number;
  duplicate: number;
  rejected: number;
  batches: number;
  hasMore: boolean;
}

export interface SyncDownloadStats {
  fetched: number;
  created: number;
  updated: number;
  duplicate: number;
  skipped: number;
  filtered: number;
  batches: number;
  hasMore: boolean;
}

export interface SyncNowResponse {
  sessionId: string;
  remoteBaseUrl: string;
  upload: SyncUploadStats;
  download: SyncDownloadStats;
}

export interface AiSummary {
  topic: string;
  whatHappened: string;
  whereWeLeftOff: string;
  vibe: string;
  pickBackUpWith: string;
}

export interface CalibrateSessionResponse {
  previousAvec: AvecState;
  delta: number;
  driftClassification: string;
  trigger: string;
  triggerHistory: string[];
  isFirstCalibration: boolean;
}

export interface CollapseCardData {
  node: GraphNodeDto;
  nodeDto: NodeDto | null;
  relatedSessions: GraphSessionDto[];
}

export interface Vec2 {
  x: number;
  y: number;
}
