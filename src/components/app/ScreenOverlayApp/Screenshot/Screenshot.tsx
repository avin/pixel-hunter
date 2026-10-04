import { useState } from "react";
import { screenshotSrc } from "@/lib/screenshots";
import { useScreenshotsDirQuery } from "@/store/api";
import type { Size } from "../physicalPoint";

interface Props {
  profileId: string;
  pixelId: string;
  /** Receives the physical size of the loaded screenshot. */
  onLoad?: (size: Size) => void;
  /** Rendered over the image once its physical size is known. */
  children?: (size: Size) => React.ReactNode;
}

/** Shows the screenshot from the top-left corner, one image pixel per physical pixel. */
export function Screenshot({ profileId, pixelId, onLoad, children }: Props) {
  const { data: dir } = useScreenshotsDirQuery();
  const [size, setSize] = useState<Size>();
  const dpr = window.devicePixelRatio;

  if (!dir) {
    return null;
  }
  return (
    <>
      <img
        src={screenshotSrc(dir, profileId, pixelId, "full")}
        alt="Снимок экрана"
        draggable={false}
        onLoad={(event) => {
          const { naturalWidth, naturalHeight } = event.currentTarget;
          const loaded = { width: naturalWidth, height: naturalHeight };
          setSize(loaded);
          onLoad?.(loaded);
        }}
        style={size && { width: size.width / dpr, height: size.height / dpr }}
        className={size ? "absolute top-0 left-0 max-w-none select-none" : "invisible"}
      />
      {size && children?.(size)}
    </>
  );
}
