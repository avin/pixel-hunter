import type { ComponentType } from "react";
import type { Action, ActionType } from "@/store/types";
import { KeySequenceEditor } from "./KeySequenceEditor/KeySequenceEditor";
import { NetworkDisconnectEditor } from "./NetworkDisconnectEditor/NetworkDisconnectEditor";

type ActionOf<T extends ActionType> = Extract<Action, { type: T }>;

export interface ActionEditorProps<T extends ActionType> {
  profileId: string;
  trackerId: string;
  action: ActionOf<T>;
  onChange: (action: ActionOf<T>) => void;
}

interface ActionTypeDefinition<T extends ActionType> {
  label: string;
  createDefault(): ActionOf<T>;
  Editor: ComponentType<ActionEditorProps<T>>;
}

/** A new action type is a new `Action` variant, an entry here, and an arm in `ActionEditor`. */
export const actionTypes: { [T in ActionType]: ActionTypeDefinition<T> } = {
  keySequence: {
    label: "Нажатие клавиш",
    createDefault: () => ({ type: "keySequence", steps: [], sound: { id: null, volumePercent: 50 } }),
    Editor: KeySequenceEditor,
  },
  networkDisconnect: {
    label: "Разрыв сетевого соединения",
    createDefault: () => ({ type: "networkDisconnect", sound: { id: null, volumePercent: 50 } }),
    Editor: NetworkDisconnectEditor,
  },
};
