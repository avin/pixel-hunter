import { Hourglass, MonitorCheck } from "lucide-react";
import { cn } from "cn";
import { gameActiveSelector } from "@/store/engineSlice";
import { useAppSelector } from "@/store/hooks";

interface Props {
  profileId: string;
  processName: string;
}

export function GameWindowStatus({ profileId, processName }: Props) {
  const isGameActive = useAppSelector((state) =>
    gameActiveSelector(state, profileId),
  );

  return (
    <p
      role="status"
      className={cn(
        "flex min-w-0 items-center gap-2 text-xs",
        !isGameActive && "text-muted-foreground",
      )}
    >
      {isGameActive ? (
        <MonitorCheck className="size-4 shrink-0" />
      ) : (
        <Hourglass className="size-4 shrink-0" />
      )}
      <span className="truncate">
        {isGameActive
          ? "Окно игры активно"
          : `Ожидание окна игры ${processName}`}
      </span>
    </p>
  );
}
