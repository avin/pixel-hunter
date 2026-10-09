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
  const view = new URLSearchParams(search).get("view");

  return (
    <Provider store={store}>
      {view === "picker" ? <ScreenshotPicker /> : <ScreenshotViewer />}
    </Provider>
  );
}
