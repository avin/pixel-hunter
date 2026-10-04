import { Provider } from "react-redux";
import { AppRouter } from "@/components/app/AppRouter/AppRouter";
import { Toaster } from "@/components/ui/sonner";
import type { AppStore } from "@/store";

interface Props {
  store: AppStore;
}

export function App({ store }: Props) {
  return (
    <Provider store={store}>
      <AppRouter />
      <Toaster />
    </Provider>
  );
}
