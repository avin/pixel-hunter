import { configureStore } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import { engineSlice } from "./engineSlice";
import { errorToastMiddleware } from "./errorToastMiddleware";
import { pixelCaptureSlice } from "./pixelCaptureSlice";
import { actionEditsSlice } from "./actionEditsSlice";

export function createAppStore() {
  return configureStore({
    reducer: {
      [tauriApi.reducerPath]: tauriApi.reducer,
      [pixelCaptureSlice.name]: pixelCaptureSlice.reducer,
      [engineSlice.name]: engineSlice.reducer,
      [actionEditsSlice.name]: actionEditsSlice.reducer,
    },
    middleware: (getDefaultMiddleware) =>
      getDefaultMiddleware().concat(tauriApi.middleware, errorToastMiddleware),
  });
}

export const store = createAppStore();

export type AppStore = typeof store;
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
