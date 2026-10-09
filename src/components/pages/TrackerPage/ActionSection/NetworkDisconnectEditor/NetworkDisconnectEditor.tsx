import type { ActionEditorProps } from "../actionTypes";

type Props = ActionEditorProps<"networkDisconnect">;

export function NetworkDisconnectEditor(_props: Props) {
  return (
    <p className="px-3 py-3 text-xs text-muted-foreground">
      При срабатывании обрываются TCP-соединения процесса профиля.
    </p>
  );
}
