import { isRejectedWithValue, type Middleware } from "@reduxjs/toolkit";
import { toast } from "sonner";
import type { TauriBaseQueryError } from "./tauriBaseQuery";

export const errorToastMiddleware: Middleware = () => (next) => (action) => {
  if (isRejectedWithValue(action)) {
    const error = action.payload as TauriBaseQueryError;
    toast.error(error.message);
  }
  return next(action);
};
