import { useEffect, useEffectEvent, useLayoutEffect, useState } from "react";
import { screenshotSrc } from "@/lib/screenshots";
import { useScreenshotsDirQuery } from "@/store/api";
import type { Size } from "../physicalPoint";

interface Props {
  profileId: string;
  pixelId: string;
  isFitted?: boolean;
  /** Receives the physical size of the loaded screenshot. */
  onLoad?: (size: Size) => void;
  onReady?: () => void;
  onError?: () => void;
  /** Rendered over the image once its physical size is known. */
  children?: (size: Size) => React.ReactNode;
}

/** Picking uses physical pixels; preview fits the image within 90% of the viewport. */
export function Screenshot({ profileId, pixelId, isFitted = false, onLoad, onReady, onError, children }: Props) {
  const { data: dir } = useScreenshotsDirQuery();
  const [size, setSize] = useState<Size>();
  const dpr = window.devicePixelRatio;
  const [viewport, setViewport] = useState(() => ({ width: window.innerWidth, height: window.innerHeight }));

  useEffect(() => {
    if (!isFitted) return;
    const resize = () => setViewport({ width: window.innerWidth, height: window.innerHeight });
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, [isFitted]);

  const ready = useEffectEvent(() => onReady?.());
  // Hidden WebViews can suspend animation frames. Notify after the decoded image
  // and its dimensions have been committed instead of waiting for a frame.
  useLayoutEffect(() => {
    if (size) ready();
  }, [size]);

  const scale = size && (isFitted
    ? Math.min(viewport.width * 0.9 / size.width, viewport.height * 0.9 / size.height)
    : 1 / dpr);

  if (!dir) {
    return null;
  }
  return (
    <div className={isFitted ? "relative shrink-0" : undefined} style={isFitted && size && scale ? { width: size.width * scale, height: size.height * scale } : undefined}>
      <img
        src={screenshotSrc(dir, profileId, pixelId, "full")}
        alt="Снимок экрана"
        draggable={false}
        onLoad={async (event) => {
          const image = event.currentTarget;
          if (onReady && typeof image.decode === "function") {
            try { await image.decode(); } catch { onError?.(); return; }
          }
          const { naturalWidth, naturalHeight } = image;
          const loaded = { width: naturalWidth, height: naturalHeight };
          setSize(loaded);
          onLoad?.(loaded);
        }}
        onError={onError}
        style={size && scale ? { width: size.width * scale, height: size.height * scale } : undefined}
        className={size ? "absolute top-0 left-0 max-w-none select-none" : "invisible"}
      />
      {size && children?.(size)}
    </div>
  );
}
