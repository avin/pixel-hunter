import { Provider } from "react-redux";
import { AppRouter } from "@/components/app/AppRouter/AppRouter";
import { Toaster } from "@/components/ui/sonner";
import { useEngineEvents } from "@/hooks/useEngineEvents";
import { useWorkEvents } from "@/hooks/useWorkEvents";
import { usePixelCaptureEvents } from "@/hooks/usePixelCaptureEvents";
import { useScreenshotViewErrors } from "@/hooks/useScreenshotViewErrors";
import type { AppStore } from "@/store";

interface Props {
  store: AppStore;
  router?: React.ComponentProps<typeof AppRouter>["router"];
}

export function App({ store, router }: Props) {
  return (
    <Provider store={store}>
      <BackendEvents />
      <AppRouter router={router} />
      <Toaster />
    </Provider>
  );
}

function BackendEvents() {
  usePixelCaptureEvents();
  useScreenshotViewErrors();
  useEngineEvents();
  useWorkEvents();
  return null;
}
