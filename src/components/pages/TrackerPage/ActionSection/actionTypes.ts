import type { ComponentType } from "react";
import type { Action, ActionType } from "@/store/types";
import { KeySequenceEditor } from "./KeySequenceEditor/KeySequenceEditor";

type ActionOf<T extends ActionType> = Extract<Action, { type: T }>;

export interface ActionEditorProps<T extends ActionType> {
  profileId: string;
  trackerId: string;
  action: ActionOf<T>;
}

interface ActionTypeDefinition<T extends ActionType> {
  label: string;
  createDefault(): ActionOf<T>;
  Editor: ComponentType<ActionEditorProps<T>>;
}

/** A new action type is a new `Action` variant plus an entry here. */
export const actionTypes: { [T in ActionType]: ActionTypeDefinition<T> } = {
  keySequence: {
    label: "Нажатие клавиш",
    createDefault: () => ({ type: "keySequence", steps: [] }),
    Editor: KeySequenceEditor,
  },
};
