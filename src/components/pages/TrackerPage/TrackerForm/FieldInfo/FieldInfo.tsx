import { Info } from "lucide-react";
import { useId, useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

interface Props {
  label: string;
  children: React.ReactNode;
}

export function FieldInfo({ label, children }: Props) {
  const [isOpen, setIsOpen] = useState(false);
  const triggerId = useId();
  const descriptionId = useId();
  return (
    <Tooltip
      open={isOpen}
      triggerId={triggerId}
      onOpenChange={(open, details) => {
        if (details.reason === "trigger-hover" || details.reason === "trigger-focus") {
          details.cancel();
          return;
        }
        setIsOpen(open);
      }}
    >
      <TooltipTrigger
        id={triggerId}
        type="button"
        closeOnClick={false}
        aria-label={`Информация: ${label}`}
        aria-describedby={isOpen ? descriptionId : undefined}
        aria-expanded={isOpen}
        onClick={() => setIsOpen((open) => !open)}
        className="inline-flex shrink-0 cursor-pointer items-center text-muted-foreground outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring"
      >
        <Info className="size-3.5" />
      </TooltipTrigger>
      <TooltipContent id={descriptionId} role="tooltip">{children}</TooltipContent>
    </Tooltip>
  );
}
