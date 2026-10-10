import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import type { RootState } from "./index";
import type { CaptureSource, CaptureStatus } from "./types";

interface PixelCaptureState {
  active?: CaptureSource;
  status?: CaptureStatus;
}

const initialState: PixelCaptureState = {};

export function captureIsWaiting(status?: CaptureStatus) {
  return status?.phase === "starting" || status?.phase === "waitingForPrintScreen";
}

export const pixelCaptureSlice = createSlice({
  name: "pixelCapture",
  initialState,
  reducers: {
    setCaptureStatus(state, { payload }: PayloadAction<CaptureStatus>) {
      if (!state.active || payload.sessionId !== state.active.sessionId || payload.mode !== state.active.mode || payload.profileId !== state.active.profileId || payload.trackerId !== state.active.trackerId) return;
      if (!captureIsWaiting(state.status) && (payload.phase === "starting" || payload.phase === "waitingForPrintScreen")) return;
      if (state.status?.phase === "saving" && payload.phase !== "done" && payload.phase !== "cancelled" && payload.phase !== "failed") return;
      if (payload.phase === "done" || payload.phase === "cancelled") return initialState;
      state.status = payload;
      if (payload.phase === "failed") state.active = undefined;
    },
    dismissCaptureStatus(state) {
      if (!state.active) return initialState;
    },
  },
  extraReducers: (builder) => {
    builder
      .addMatcher(tauriApi.endpoints.pixelCaptureStart.matchPending, (_state, action) => {
        const active: CaptureSource = { ...action.meta.arg.originalArgs, mode: "pixel" };
        return { active, status: { ...active, phase: "starting" } };
      })
      .addMatcher(tauriApi.endpoints.iconCaptureStart.matchPending, (_state, action) => {
        const active: CaptureSource = { ...action.meta.arg.originalArgs, mode: "icon" };
        return { active, status: { ...active, phase: "starting" } };
      })
      .addMatcher(
        (action) => tauriApi.endpoints.pixelCaptureStart.matchRejected(action) || tauriApi.endpoints.iconCaptureStart.matchRejected(action),
        (state, action) => {
          if (state.active?.sessionId !== action.meta.arg.originalArgs.sessionId) return;
          state.status = { ...state.active, phase: "failed", reason: action.payload?.message ?? action.error.message ?? "Не удалось начать захват" };
          state.active = undefined;
        },
      );
  },
});

export const { setCaptureStatus, dismissCaptureStatus } = pixelCaptureSlice.actions;

export function isCapturingSelector(state: RootState) {
  return Boolean(state.pixelCapture.active);
}

export function trackerCaptureStatusSelector(state: RootState, profileId: string, trackerId: string) {
  const status = state.pixelCapture.status;
  return status?.profileId === profileId && status.trackerId === trackerId ? status : undefined;
}
