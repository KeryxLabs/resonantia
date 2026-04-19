export type ComposeMode = "live" | "importare";

export type CrossSessionRoutingPreference = "ask" | "match-session" | "active-tab";

export type ComposeMessageCrossSessionMeta = {
  sourceSessionId: string;
  targetSessionId: string;
};

export type ComposeMessage = {
  role: "user" | "assistant";
  content: string;
  at: string;
  crossSession?: ComposeMessageCrossSessionMeta;
};

export type ComposeTabInfo = {
  id: string;
  title: string;
  sessionId: string;
};

export type ComposeContextSession = {
  sessionId: string;
  label: string;
};

export type ComposeContextNode = {
  key: string;
  sessionId: string;
  title: string;
  timestamp: string;
  tier: string;
  psi: number;
  preview: string;
};

export type ComposeInjectedNode = {
  key: string;
  title: string;
  sessionId: string;
  timestamp: string;
};

export type ComposeTokenUsage = {
  contextTokens: number;
  draftTokens: number;
  projectedTurnTokens: number;
  contextWindowTokens: number;
  usagePercent: number;
  thresholdPercent: number;
  thresholdTokens: number;
  remainingTokens: number;
};

export type ComposeProviderUsage = {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
  responseCount: number;
  provider: string;
  model: string;
  hasUsageData: boolean;
};

export type ComposeCalibrationAvec = {
  stability: number;
  friction: number;
  logic: number;
  autonomy: number;
  psi: number;
};

export type ComposeResult = {
  psi: number;
  duplicateSkipped: boolean;
  status: "created" | "updated" | "duplicate" | "skipped";
} | null;

export type ComposeLiveUiProps = {
  tabs: ComposeTabInfo[];
  activeTabId: string;
  maxTabs: number;
  contextSessions: ComposeContextSession[];
  contextOriginSessionId: string;
  contextBrowseSessionId: string;
  contextNodes: ComposeContextNode[];
  contextNodesLoading: boolean;
  contextNodesError: string | null;
  injectedNodes: ComposeInjectedNode[];
  onDraftInput: () => void;
  selectComposeTab: (tabId: string) => void;
  createComposeTab: () => void;
  closeComposeTab: (tabId: string) => void;
  selectContextSession: (sessionId: string) => void;
  injectContextNode: (nodeKey: string) => void;
  removeInjectedNode: (nodeKey: string) => void;
  crossSessionRoutingPreference: CrossSessionRoutingPreference;
  clearCrossSessionRoutingPreference: () => void;
  tokenUsage: ComposeTokenUsage;
  providerUsage: ComposeProviderUsage;
  calibrationAvec: ComposeCalibrationAvec;
  autoEncodeEnabled: boolean;
  autoEncodeThresholdPercent: number;
  setAutoEncodeEnabled: (enabled: boolean) => void;
  setAutoEncodeThresholdPercent: (thresholdPercent: number) => void;
};
