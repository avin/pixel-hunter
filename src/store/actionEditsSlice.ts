import { createSlice, type PayloadAction } from "@reduxjs/toolkit";
import { tauriApi } from "./api";
import type { AppDispatch, RootState } from "./index";
import type { Action } from "./types";

interface Edit {
  action: Action;
  revision: number;
  isSaving: boolean;
  error?: string;
}

export const actionEditKey = (profileId: string, trackerId: string) => JSON.stringify([profileId, trackerId]);

export const actionEditsSlice = createSlice({
  name: "actionEdits",
  initialState: {} as Record<string, Edit>,
  reducers: {
    setDesiredAction(state, { payload }: PayloadAction<{ key: string; action: Action }>) {
      const previous = state[payload.key];
      state[payload.key] = {
        action: payload.action,
        revision: (previous?.revision ?? 0) + 1,
        isSaving: previous?.isSaving ?? false,
      };
    },
    setActionSaving(state, { payload }: PayloadAction<string>) {
      state[payload].isSaving = true;
      delete state[payload].error;
    },
    setActionSaved(state, { payload }: PayloadAction<{ key: string; revision: number; action: Action }>) {
      const edit = state[payload.key];
      if (edit.revision === payload.revision) {
        edit.action = payload.action;
        edit.isSaving = false;
      }
    },
    setActionSaveFailed(state, { payload }: PayloadAction<{ key: string; error: string }>) {
      state[payload.key].isSaving = false;
      state[payload.key].error = payload.error;
    },
  },
});

export const { setDesiredAction, setActionSaving, setActionSaved, setActionSaveFailed } = actionEditsSlice.actions;

/** The save loop belongs to the store so navigation cannot discard the last edit. */
export const saveDesiredAction = (profileId: string, trackerId: string) =>
  async (dispatch: AppDispatch, getState: () => RootState) => {
    const key = actionEditKey(profileId, trackerId);
    if (!getState().actionEdits[key] || getState().actionEdits[key].isSaving) return;
    dispatch(setActionSaving(key));
    while (true) {
      const { action, revision } = getState().actionEdits[key];
      const mutation = dispatch(tauriApi.endpoints.actionUpdate.initiate({ profileId, trackerId, action }));
      const result = await mutation;
      mutation.reset();
      if (result.error) {
        const error = "message" in result.error ? String(result.error.message) : "Не удалось сохранить действие";
        dispatch(setActionSaveFailed({ key, error }));
        return;
      }
      dispatch(setActionSaved({ key, revision, action: result.data! }));
      if (!getState().actionEdits[key].isSaving) return;
    }
  };
