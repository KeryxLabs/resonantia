import type { GraphResponse, NodeDto } from "$lib/types";
import { resonantiaClient } from "$lib/resonantiaClient";

export async function listNodes(limit: number, sessionId?: string): Promise<{ nodes: NodeDto[] }> {
  const response = await resonantiaClient.listNodes(limit, sessionId);
  return { nodes: response.nodes };
}

export async function getGraph(limit: number, sessionId?: string): Promise<GraphResponse> {
  return resonantiaClient.getGraph(limit, sessionId);
}

export { resonantiaClient };
