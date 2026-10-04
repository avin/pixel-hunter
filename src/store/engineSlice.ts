import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import type { RootState } from "./index";
import type { EngineStatus } from "./types";

type EngineState = Partial<EngineStatus>;

const initialState: EngineState = {};

export const engineSlice = createSlice({
  name: "engine",
  initialState,
  reducers: {
    setEngineStatus(_state, action: PayloadAction<EngineStatus>) {
      return action.payload;
    },
  },
  extraReducers: (builder) => {
    builder
      .addMatcher(tauriApi.endpoints.engineActivate.matchPending, () => initialState)
      .addMatcher(tauriApi.endpoints.engineDeactivate.matchPending, () => initialState);
  },
});

export const { setEngineStatus } = engineSlice.actions;

/** Whether the game window of the profile is in the foreground; `false` until the engine reports. */
export function gameActiveSelector(state: RootState, profileId: string) {
  return state.engine.profileId === profileId && state.engine.gameActive === true;
}
