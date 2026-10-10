import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import type { RootState } from "./index";
import type { EngineStatus, IconStatus } from "./types";

interface EngineState extends Partial<EngineStatus> {
  iconErrors: Record<string, IconStatus>;
}

const initialState: EngineState = { iconErrors: {} };

export const engineSlice = createSlice({
  name: "engine",
  initialState,
  reducers: {
    setEngineStatus(state, action: PayloadAction<EngineStatus>) {
      Object.assign(state, action.payload);
    },
    setIconStatus(state, action: PayloadAction<IconStatus>) {
      const status = action.payload;
      if (status.error) state.iconErrors[status.iconId] = status;
      else delete state.iconErrors[status.iconId];
    },
  },
  extraReducers: (builder) => {
    builder
      .addMatcher(tauriApi.endpoints.engineActivate.matchPending, () => initialState)
      .addMatcher(tauriApi.endpoints.engineDeactivate.matchPending, () => initialState);
  },
});

export const { setEngineStatus, setIconStatus } = engineSlice.actions;

/** Whether the game window of the profile is in the foreground; `false` until the engine reports. */
export function gameActiveSelector(state: RootState, profileId: string) {
  return state.engine.profileId === profileId && state.engine.gameActive === true;
}
