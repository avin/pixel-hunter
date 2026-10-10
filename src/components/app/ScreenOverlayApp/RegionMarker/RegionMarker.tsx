import type { PhysicalRect } from "@/store/types";
import type { Size } from "../physicalPoint";

interface Props {
  rect: PhysicalRect;
  size: Size;
  kind: "search" | "template";
  isPreview?: boolean;
}

export function RegionMarker({ rect, size, kind, isPreview = false }: Props) {
  const label = kind === "search" ? "Область поиска" : "Образец иконки";
  const style = {
    left: `${rect.x / size.width * 100}%`,
    top: `${rect.y / size.height * 100}%`,
    width: `${rect.width / size.width * 100}%`,
    height: `${rect.height / size.height * 100}%`,
  };

  if (!isPreview) {
    return <div role="img" aria-label={label} className="pointer-events-none absolute border-2 border-orange-400" style={style} />;
  }

  return (
    <svg role="img" aria-label={label} className="pointer-events-none absolute overflow-visible" style={style}>
      <g fill="none">
        <rect width="100%" height="100%" stroke="var(--color-black)" strokeWidth={9} />
        <rect width="100%" height="100%" stroke="var(--color-white)" strokeWidth={6} />
        <g className="animate-region-marker-flow">
          <rect width="100%" height="100%" stroke="var(--color-pixel-marker-ring)" strokeWidth={4} className="animate-preview-marker-color" />
          <rect width="100%" height="100%" stroke="var(--color-pixel-marker-cross)" strokeWidth={4} strokeDasharray="12 12" className="animate-preview-marker-color" style={{ animationDelay: "-1.5s" }} />
        </g>
      </g>
    </svg>
  );
}
