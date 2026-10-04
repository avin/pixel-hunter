import { createContext, useContext } from "react";

export const WorkspaceNavigationContext = createContext({
  isLeaving: false,
  setIsLeaving: (_value: boolean) => {},
});
export function useWorkspaceNavigation() {
  return useContext(WorkspaceNavigationContext);
}
