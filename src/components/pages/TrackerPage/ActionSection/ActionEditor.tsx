import type { Action } from "@/store/types";
import { actionTypes } from "./actionTypes";

interface Props {
  profileId: string;
  trackerId: string;
  action: Action;
  onChange: (action: Action) => void;
}

export function ActionEditor({ profileId, trackerId, action, onChange }: Props) {
  switch (action.type) {
    case "keySequence": {
      const Editor = actionTypes.keySequence.Editor;
      return <Editor profileId={profileId} trackerId={trackerId} action={action} onChange={onChange} />;
    }
    case "networkDisconnect": {
      const Editor = actionTypes.networkDisconnect.Editor;
      return <Editor profileId={profileId} trackerId={trackerId} action={action} onChange={onChange} />;
    }
  }
}
