import {
  createResonantiaClient,
  createWebResonantiaClient,
  type ResonantiaClient,
} from "@resonantia/core";

declare global {
  interface Window {
    __TAURI__?: unknown;
    __TAURI_INTERNALS__?: unknown;
  }
}

function isTauriRuntime(): boolean {
  if (typeof window === "undefined") {
    return false;
  }

  return "__TAURI_INTERNALS__" in window || "__TAURI__" in window;
}

async function resolveClient(): Promise<ResonantiaClient> {
  if (isTauriRuntime()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return createResonantiaClient((command, args) => invoke(command, args ?? {}));
  }

  return createWebResonantiaClient();
}

const clientPromise = resolveClient();

function withClient<T>(handler: (client: ResonantiaClient) => Promise<T>): Promise<T> {
  return clientPromise.then(handler);
}

async function syncAfterStoreIfConfigured(client: ResonantiaClient, sessionId: string): Promise<void> {
  try {
    const config = await client.getConfig();
    if (!config.gatewayBaseUrl.trim()) {
      return;
    }

    const trimmedSessionId = sessionId.trim();
    await client.syncNow(trimmedSessionId ? { sessionId: trimmedSessionId } : {});
  } catch (error) {
    // Store remains local-first; network sync failures should not block saves.
    console.warn("post-store sync failed", error);
  }
}

export const resonantiaClient: ResonantiaClient = {
  getHealth: () => withClient((client) => client.getHealth()),
  getConfig: () => withClient((client) => client.getConfig()),
  listNodes: (limit, sessionId) => withClient((client) => client.listNodes(limit, sessionId)),
  getGraph: (limit, sessionId) => withClient((client) => client.getGraph(limit, sessionId)),
  storeContext: (input) => withClient(async (client) => {
    const response = await client.storeContext(input);
    if (response.valid) {
      await syncAfterStoreIfConfigured(client, input.sessionId);
    }

    return response;
  }),
  syncPull: (request) => withClient((client) => client.syncPull(request)),
  syncNow: (request) => withClient((client) => client.syncNow(request)),
  calibrateSession: (input) => withClient((client) => client.calibrateSession(input)),
  summarizeNode: (rawNode) => withClient((client) => client.summarizeNode(rawNode)),
  setOllamaConfig: (baseUrl, model) => withClient((client) => client.setOllamaConfig(baseUrl, model)),
  setGatewayBaseUrl: (baseUrl) => withClient((client) => client.setGatewayBaseUrl(baseUrl)),
};
