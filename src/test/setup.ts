import "@testing-library/jest-dom/vitest";
import { clearMocks } from "@tauri-apps/api/mocks";
import { cleanup } from "@testing-library/react";
import { toast } from "sonner";
import { afterEach, vi } from "vitest";

// jsdom has no matchMedia; sonner reads it to resolve the "system" theme.
vi.stubGlobal(
  "matchMedia",
  vi.fn((query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn(),
  })),
);

afterEach(async () => {
  vi.useRealTimers();
  // Sonner keeps active toasts in module state and replays them to the next Toaster.
  toast.dismiss();
  cleanup();
  // Tauri `unlisten` callbacks resolve after unmount and still need the mocked internals.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});
