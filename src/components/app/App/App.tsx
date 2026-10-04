import { Provider } from "react-redux";
import { AppRouter } from "@/components/app/AppRouter/AppRouter";
import { Toaster } from "@/components/ui/sonner";
import { useEngineEvents } from "@/hooks/useEngineEvents";
import { usePixelCaptureEvents } from "@/hooks/usePixelCaptureEvents";
import type { AppStore } from "@/store";

interface Props {
  store: AppStore;
}

export function App({ store }: Props) {
  return (
    <Provider store={store}>
      <BackendEvents />
      <AppRouter />
      <Toaster />
    </Provider>
  );
}

function BackendEvents() {
  usePixelCaptureEvents();
  useEngineEvents();
  return null;
}
