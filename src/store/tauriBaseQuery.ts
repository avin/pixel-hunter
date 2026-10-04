import type { BaseQueryFn } from "@reduxjs/toolkit/query";
import { invoke } from "@tauri-apps/api/core";

export type TauriQueryArgs = {
  command: string;
  args?: Record<string, unknown>;
};

export type TauriBaseQueryError = {
  status: string;
  message: string;
};

export const tauriBaseQuery: BaseQueryFn<
  TauriQueryArgs,
  unknown,
  TauriBaseQueryError
> = async ({ command, args }) => {
  try {
    const data = await invoke(command, args);
    return { data };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { error: { status: "TAURI_ERROR", message } };
  }
};
