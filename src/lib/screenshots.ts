import { convertFileSrc } from "@tauri-apps/api/core";

type ScreenshotKind = "full" | "thumbnail";

/** Mirrors the Rust layout: `<dir>/<profileId>/<pixelId>.png` and `<pixelId>.thumb.png`. */
export function screenshotSrc(
  dir: string,
  profileId: string,
  pixelId: string,
  kind: ScreenshotKind,
) {
  const fileName = kind === "full" ? `${pixelId}.png` : `${pixelId}.thumb.png`;
  return convertFileSrc(`${dir}\\${profileId}\\${fileName}`);
}
