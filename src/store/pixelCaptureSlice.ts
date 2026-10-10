import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import type { RootState } from "./index";
import type { CaptureStatus } from "./types";

interface PixelCaptureState {
  profileId?: string;
  trackerId?: string;
  status?: CaptureStatus;
}

const initialState: PixelCaptureState = {};
const WAIT_FOR_GAME_MS = 10_000;

export const pixelCaptureSlice = createSlice({
  name: "pixelCapture",
  initialState,
  reducers: {
    setCaptureStatus(state, action: PayloadAction<CaptureStatus>) {
      if (action.payload.phase === "done" || action.payload.phase === "cancelled") {
        return initialState;
      }
      state.status = action.payload;
    },
    dismissCaptureStatus() {
      return initialState;
    },
  },
  extraReducers: (builder) => {
    builder
      .addMatcher(
        tauriApi.endpoints.pixelCaptureStart.matchPending,
        (_state, action) => ({
          ...action.meta.arg.originalArgs,
          status: { phase: "waitingForGame", remainingMs: WAIT_FOR_GAME_MS },
        }),
      )
      .addMatcher(
        tauriApi.endpoints.pixelCaptureStart.matchRejected,
        () => initialState,
      )
      .addMatcher(
        tauriApi.endpoints.iconCaptureStart.matchPending,
        (_state, action) => ({ ...action.meta.arg.originalArgs, status: { phase: "waitingForGame", remainingMs: WAIT_FOR_GAME_MS } }),
      )
      .addMatcher(tauriApi.endpoints.iconCaptureStart.matchRejected, () => initialState);
  },
});

export const { setCaptureStatus, dismissCaptureStatus } = pixelCaptureSlice.actions;

export function isCapturingSelector(state: RootState) {
  const phase = state.pixelCapture.status?.phase;
  return phase === "waitingForGame" || phase === "countdown" || phase === "picking" || phase === "selectingSearch" || phase === "selectingTemplate";
}

/** Status of the capture started from this tracker. */
export function trackerCaptureStatusSelector(state: RootState, trackerId: string) {
  return state.pixelCapture.trackerId === trackerId
    ? state.pixelCapture.status
    : undefined;
}
