import { invoke } from "@tauri-apps/api/core";
import { createResonantiaClient } from "@resonantia/core";

const invokeCommand = <T>(command: string, args?: Record<string, unknown>) =>
  invoke<T>(command, args ?? {});

export const resonantiaClient = createResonantiaClient(invokeCommand);
