import { useState } from "react";
import { cn } from "cn";
import { Settings } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useWorkPauseToggleMutation } from "@/store/api";
import { useAppSelector } from "@/store/hooks";
import { PauseHotkeyDialog } from "./PauseHotkeyDialog/PauseHotkeyDialog";

export function WorkControls() {
  const paused = useAppSelector((state) => state.work.paused);
  const [togglePause] = useWorkPauseToggleMutation();
  const [isHotkeyOpen, setIsHotkeyOpen] = useState(false);

  return (
    <div className="flex min-w-0 items-center gap-2">
      <p
        role="status"
        aria-label={paused ? "На паузе" : "Работает"}
        className={cn(paused && "text-destructive")}
      >
        {paused ? "На паузе" : "Работает"}
      </p>
      <Button
        variant="outline"
        size="xs"
        aria-pressed={paused}
        onClick={() => void togglePause()}
      >
        {paused ? "Снять паузу" : "Пауза"}
      </Button>
      <Button
        variant="ghost"
        size="icon-xs"
        aria-label="Настроить паузу"
        onClick={() => setIsHotkeyOpen(true)}
      >
        <Settings />
      </Button>
      {isHotkeyOpen && <PauseHotkeyDialog onClose={() => setIsHotkeyOpen(false)} />}
    </div>
  );
}
