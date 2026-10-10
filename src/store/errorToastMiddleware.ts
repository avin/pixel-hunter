import { isRejectedWithValue, type Middleware } from "@reduxjs/toolkit";
import { toast } from "sonner";
import type { TauriBaseQueryError } from "./tauriBaseQuery";
import { tauriApi } from "./api";

export const errorToastMiddleware: Middleware = () => (next) => (action) => {
  if (isRejectedWithValue(action)) {
    // Frame failures are addressed by picker_frame_failed. A late rejection
    // after cancellation must not surface an unrelated global toast.
    if (tauriApi.endpoints.pickerPixels.matchRejected(action)) return next(action);
    const error = action.payload as TauriBaseQueryError;
    toast.error(error.message);
  }
  return next(action);
};
