import type { ComponentProps } from "react"
import { cn } from "cn"

interface Props extends ComponentProps<"kbd"> {}

export function Kbd({ className, ...props }: Props) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        "pointer-events-none inline-flex h-5 w-fit min-w-5 items-center justify-center gap-1 whitespace-nowrap rounded-none border bg-muted px-1.5 align-baseline font-mono text-xs font-semibold text-foreground select-none in-data-[slot=tooltip-content]:bg-background/20 in-data-[slot=tooltip-content]:text-background dark:in-data-[slot=tooltip-content]:bg-background/10 [&_svg:not([class*='size-'])]:size-3",
        className
      )}
      {...props}
    />
  )
}

export function KbdGroup({ className, ...props }: Props) {
  return (
    <kbd
      data-slot="kbd-group"
      className={cn("inline-flex items-center gap-1", className)}
      {...props}
    />
  )
}
