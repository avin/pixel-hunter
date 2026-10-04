import { Provider } from "react-redux";
import type { AppStore } from "@/store";
import { ScreenshotPicker } from "./ScreenshotPicker/ScreenshotPicker";
import { ScreenshotViewer } from "./ScreenshotViewer/ScreenshotViewer";

interface Props {
  store: AppStore;
  /** Query string of the overlay window URL, set by the Rust side. */
  search: string;
}

export function isScreenOverlayView(search: string) {
  const view = new URLSearchParams(search).get("view");
  return view === "picker" || view === "viewer";
}

export function ScreenOverlayApp({ store, search }: Props) {
  const params = new URLSearchParams(search);
  const profileId = params.get("profileId") ?? "";
  const pixelId = params.get("pixelId") ?? "";

  return (
    <Provider store={store}>
      {params.get("view") === "picker" ? (
        <ScreenshotPicker profileId={profileId} pixelId={pixelId} />
      ) : (
        <ScreenshotViewer
          profileId={profileId}
          pixelId={pixelId}
          pixel={{ x: Number(params.get("x")), y: Number(params.get("y")) }}
        />
      )}
    </Provider>
  );
}
