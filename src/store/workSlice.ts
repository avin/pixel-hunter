import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";

type WorkState = {
  paused: boolean;
};

const initialState: WorkState = { paused: false };

export const workSlice = createSlice({
  name: "work",
  initialState,
  reducers: {
    setWorkPaused(state, action: PayloadAction<boolean>) {
      state.paused = action.payload;
    },
  },
  extraReducers: (builder) => {
    builder.addMatcher(
      tauriApi.endpoints.workPauseToggle.matchFulfilled,
      (state, action) => {
        state.paused = action.payload.paused;
      },
    );
  },
});

export const { setWorkPaused } = workSlice.actions;
