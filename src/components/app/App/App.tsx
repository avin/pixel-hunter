import { Provider } from "react-redux";
import { AppRouter } from "@/components/app/AppRouter/AppRouter";
import { Toaster } from "@/components/ui/sonner";
import { usePixelCaptureEvents } from "@/hooks/usePixelCaptureEvents";
import type { AppStore } from "@/store";

interface Props {
  store: AppStore;
}

export function App({ store }: Props) {
  return (
    <Provider store={store}>
      <PixelCaptureEvents />
      <AppRouter />
      <Toaster />
    </Provider>
  );
}

function PixelCaptureEvents() {
  usePixelCaptureEvents();
  return null;
}
