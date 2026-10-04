import { mockConvertFileSrc, mockWindows } from "@tauri-apps/api/mocks";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { createAppStore } from "@/store";
import { createFakeBackend } from "@/test/fakeBackend";
import { ScreenOverlayApp } from "./ScreenOverlayApp";

function setDevicePixelRatio(value: number) {
  Object.defineProperty(window, "devicePixelRatio", { value, configurable: true });
}

function renderOverlay(search: string, devicePixelRatio = 1) {
  setDevicePixelRatio(devicePixelRatio);
  mockWindows("screen-overlay");
  mockConvertFileSrc("windows");
  const backend = createFakeBackend();
  backend.install();
  const user = userEvent.setup();
  render(<ScreenOverlayApp store={createAppStore()} search={search} />);
  return { user, backend };
}

/** jsdom does not decode images; the overlay learns the physical size from `load`. */
async function loadScreenshot(width: number, height: number) {
  const image = await screen.findByRole("img", { name: "Снимок экрана" });
  Object.defineProperty(image, "naturalWidth", { value: width });
  Object.defineProperty(image, "naturalHeight", { value: height });
  fireEvent.load(image);
  return image;
}

afterEach(() => setDevicePixelRatio(1));

describe("ScreenOverlayApp picker", () => {
  const search = "?view=picker&profileId=profile-1&pixelId=pixel-1";
  const picker = () =>
    screen.getByRole("application", { name: /Выберите точку на снимке/ });

  it("shows the screenshot of the pixel one to one", async () => {
    renderOverlay(search, 1.5);

    const image = await loadScreenshot(3840, 2160);

    expect(image).toHaveAttribute(
      "src",
      expect.stringContaining(encodeURIComponent("C:\\data\\screenshots\\profile-1\\pixel-1.png")),
    );
    expect(image).toHaveStyle({ width: "2560px", height: "1440px" });
  });

  it("picks the physical pixel under the cursor", async () => {
    const { user, backend } = renderOverlay(search, 1.5);
    await loadScreenshot(3840, 2160);

    await user.pointer({
      keys: "[MouseLeft>]",
      target: picker(),
      coords: { clientX: 541.6, clientY: 693.9 },
    });

    await waitFor(() =>
      expect(backend.callsOf("pixel_capture_pick")).toEqual([{ x: 812, y: 1040 }]),
    );
  });

  it("picks only once", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });
    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 9, clientY: 9 } });

    await waitFor(() => expect(backend.callsOf("pixel_capture_pick")).toHaveLength(1));
  });

  it("ignores clicks before the screenshot is shown", async () => {
    const { user, backend } = renderOverlay(search);
    await screen.findByRole("img", { name: "Снимок экрана" });

    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });

    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);
  });

  it("cancels the capture on Escape", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.keyboard("{Escape}");

    await waitFor(() => expect(backend.callsOf("pixel_capture_cancel")).toHaveLength(1));
    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);
  });
});

describe("ScreenOverlayApp viewer", () => {
  const search = "?view=viewer&profileId=profile-1&pixelId=pixel-1&x=812&y=1040";

  it("marks the pixel with a cross one physical pixel thick", async () => {
    renderOverlay(search, 1.5);
    await loadScreenshot(3840, 2160);

    const marker = screen.getByRole("img", { name: "Пиксель (812, 1040)" });
    const [horizontal, vertical] = marker.querySelectorAll("line");
    expect(horizontal).toHaveAttribute("y1", String(1040.5 / 1.5));
    expect(vertical).toHaveAttribute("x1", String(812.5 / 1.5));
    expect(marker.querySelector("g")).toHaveAttribute("stroke-width", String(1 / 1.5));
  });

  it("closes on a click", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.pointer({ keys: "[MouseLeft]", target: document.body });

    await waitFor(() => expect(backend.callsOf("plugin:window|close")).toHaveLength(1));
  });

  it("closes on any key", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.keyboard("a");

    await waitFor(() => expect(backend.callsOf("plugin:window|close")).toHaveLength(1));
  });
});
