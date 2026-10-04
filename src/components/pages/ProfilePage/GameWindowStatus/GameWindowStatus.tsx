import { Hourglass, MonitorCheck } from "lucide-react";
import { cn } from "cn";
import { gameActiveSelector } from "@/store/engineSlice";
import { useAppSelector } from "@/store/hooks";

interface Props {
  profileId: string;
  processName: string;
}

export function GameWindowStatus({ profileId, processName }: Props) {
  const isGameActive = useAppSelector((state) => gameActiveSelector(state, profileId));

  return (
    <p
      role="status"
      className={cn(
        "flex items-center gap-2 rounded-lg border px-3 py-2 text-sm",
        isGameActive ? "border-primary/40" : "text-muted-foreground",
      )}
    >
      {isGameActive ? <MonitorCheck className="size-4" /> : <Hourglass className="size-4" />}
      {isGameActive
        ? "Окно игры активно, отслеживания работают"
        : `Ожидание окна игры ${processName}`}
    </p>
  );
}
