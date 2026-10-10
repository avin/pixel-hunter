import { configureStore, isPlain, type SerializableStateInvariantMiddlewareOptions } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import { engineSlice } from "./engineSlice";
import { errorToastMiddleware } from "./errorToastMiddleware";
import { pixelCaptureSlice } from "./pixelCaptureSlice";
import { actionEditsSlice } from "./actionEditsSlice";
import { workSlice } from "./workSlice";

export function createAppStore() {
  return configureStore({
    reducer: {
      [tauriApi.reducerPath]: tauriApi.reducer,
      [pixelCaptureSlice.name]: pixelCaptureSlice.reducer,
      [engineSlice.name]: engineSlice.reducer,
      [actionEditsSlice.name]: actionEditsSlice.reducer,
      [workSlice.name]: workSlice.reducer,
    },
    middleware: (getDefaultMiddleware) =>
      getDefaultMiddleware<{ serializableCheck: SerializableStateInvariantMiddlewareOptions }>({
        serializableCheck: {
          // The picker keeps the binary IPC response only until it draws the canvas.
          isSerializable: (value) => value instanceof ArrayBuffer || isPlain(value),
        },
      }).concat(tauriApi.middleware, errorToastMiddleware),
  });
}

export const store = createAppStore();

export type AppStore = typeof store;
export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
