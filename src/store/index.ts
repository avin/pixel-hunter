import { configureStore } from "@reduxjs/toolkit";
import { tauriApi } from "./api";

export const store = configureStore({
  reducer: {
    [tauriApi.reducerPath]: tauriApi.reducer,
  },
  middleware: (getDefaultMiddleware) =>
    getDefaultMiddleware().concat(tauriApi.middleware),
});

export type RootState = ReturnType<typeof store.getState>;
export type AppDispatch = typeof store.dispatch;
