import { emit } from "@tauri-apps/api/event";
import { mockConvertFileSrc, mockWindows } from "@tauri-apps/api/mocks";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { createAppStore } from "@/store";
import { createFakeBackend } from "@/test/fakeBackend";
import { ScreenOverlayApp } from "./ScreenOverlayApp";

function setDevicePixelRatio(value: number) {
  Object.defineProperty(window, "devicePixelRatio", { value, configurable: true });
}

function renderOverlay(search: string, devicePixelRatio = 1) {
  setDevicePixelRatio(devicePixelRatio);
  const isViewer = new URLSearchParams(search).get("view") === "viewer";
  mockWindows(isViewer ? "screenshot-viewer" : "screen-overlay");
  mockConvertFileSrc("windows");
  const backend = createFakeBackend({ viewerRequest: isViewer ? { sequence: 1, profileId: "profile-1", pixelId: "pixel-1", x: 812, y: 1040 } : undefined });
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

afterEach(() => {
  setDevicePixelRatio(1);
  Object.defineProperty(window, "innerWidth", { value: 1024, configurable: true });
  Object.defineProperty(window, "innerHeight", { value: 768, configurable: true });
});

describe("ScreenOverlayApp picker", () => {
  const search = "?view=picker";
  const frame = { profileId: "profile-1", pixelId: "pixel-1" };
  const picker = () =>
    screen.getByRole("application", { name: /Выберите точку на снимке/ });

  async function renderPicker(devicePixelRatio = 1) {
    const rendered = renderOverlay(search, devicePixelRatio);
    await waitFor(() => expect(rendered.backend.callsOf("picker_ready")).toHaveLength(1));
    return rendered;
  }

  async function showFrame(profileId: string, pixelId: string) {
    await act(async () => {
      await emit("screen-overlay://frame", { profileId, pixelId });
    });
  }

  it("reports ready while the window is still hidden", async () => {
    const { backend } = await renderPicker();

    expect(backend.callsOf("screenshots_dir").length).toBeGreaterThan(0);
    await waitFor(() => expect(backend.callsOf("picker_frame")).toHaveLength(1));
    expect(backend.state.pickerVisible).toBe(false);
    expect(screen.queryByRole("img", { name: "Снимок экрана" })).not.toBeInTheDocument();
  });

  it("shows the screenshot of the pixel one to one after the frame event", async () => {
    const { backend } = await renderPicker(1.5);
    await showFrame(frame.profileId, frame.pixelId);

    const image = await loadScreenshot(3840, 2160);

    expect(image).toHaveAttribute(
      "src",
      expect.stringContaining(encodeURIComponent("C:\\data\\screenshots\\profile-1\\pixel-1.png")),
    );
    expect(image).toHaveStyle({ width: "2560px", height: "1440px" });
    await waitFor(() => expect(backend.callsOf("picker_frame_drawn")).toEqual([frame]));
    expect(backend.state.pickerVisible).toBe(true);
  });

  it("picks the physical pixel under the cursor", async () => {
    const { user, backend } = await renderPicker(1.5);
    await showFrame(frame.profileId, frame.pixelId);
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
    const { user, backend } = await renderPicker();
    await showFrame(frame.profileId, frame.pixelId);
    await loadScreenshot(1920, 1080);

    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });
    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 9, clientY: 9 } });

    await waitFor(() => expect(backend.callsOf("pixel_capture_pick")).toHaveLength(1));
  });

  it("ignores clicks before the frame is decoded", async () => {
    const { user, backend } = await renderPicker();

    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });
    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);

    await showFrame(frame.profileId, frame.pixelId);
    await screen.findByRole("img", { name: "Снимок экрана" });
    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });

    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);
    expect(backend.callsOf("picker_frame_drawn")).toEqual([]);
  });

  it("reports a show error when the screenshot fails to load", async () => {
    const { user, backend } = await renderPicker();
    await showFrame(frame.profileId, frame.pixelId);
    const image = await screen.findByRole("img", { name: "Снимок экрана" });

    fireEvent.error(image);

    await waitFor(() => expect(backend.callsOf("picker_frame_failed")).toEqual([frame]));
    expect(backend.callsOf("picker_frame_drawn")).toEqual([]);
    expect(backend.state.pickerVisible).toBe(false);
    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });
    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);
  });

  it("cancels the capture on Escape", async () => {
    const { user, backend } = await renderPicker();
    await showFrame(frame.profileId, frame.pixelId);
    await loadScreenshot(1920, 1080);

    await user.keyboard("{Escape}");

    await waitFor(() => expect(backend.callsOf("pixel_capture_cancel")).toHaveLength(1));
    expect(backend.callsOf("pixel_capture_pick")).toEqual([]);
  });

  it("replaces the screenshot when a new frame arrives without reloading", async () => {
    const { user, backend } = await renderPicker();
    await showFrame(frame.profileId, frame.pixelId);
    await loadScreenshot(1920, 1080);
    await waitFor(() => expect(backend.callsOf("picker_frame_drawn")).toEqual([frame]));

    const next = { profileId: "profile-1", pixelId: "pixel-2" };
    await showFrame(next.profileId, next.pixelId);
    const image = await loadScreenshot(1920, 1080);

    expect(image).toHaveAttribute("src", expect.stringContaining("pixel-2.png"));
    expect(backend.callsOf("picker_ready")).toHaveLength(1);
    expect(screen.getAllByRole("img", { name: "Снимок экрана" })).toHaveLength(1);
    await user.pointer({ keys: "[MouseLeft]", target: picker(), coords: { clientX: 5, clientY: 5 } });
    await waitFor(() => expect(backend.callsOf("pixel_capture_pick")).toEqual([{ x: 5, y: 5 }]));
  });
});

describe("ScreenOverlayApp viewer", () => {
  const search = "?view=viewer";

  it("keeps the viewer hidden until the screenshot is decoded", async () => {
    const { backend } = renderOverlay(search);
    const image = await screen.findByRole("img", { name: "Снимок экрана" });
    let resolve!: () => void;
    const decoded = new Promise<void>((done) => { resolve = done; });
    Object.defineProperty(image, "decode", { value: vi.fn(() => decoded) });
    await loadScreenshot(1920, 1080);
    expect(backend.callsOf("screenshot_view_ready")).toHaveLength(0);
    expect(backend.state.viewerVisible).toBe(false);
    await act(async () => { resolve(); await decoded; });
    await waitFor(() => expect(backend.state.viewerVisible).toBe(true));
    expect(backend.callsOf("screenshot_view_ready")).toEqual([{ sequence: 1 }]);
  });

  it("hides and reuses the viewer for the next screenshot", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);
    await waitFor(() => expect(backend.state.viewerVisible).toBe(true));
    await user.keyboard("{Escape}");
    await waitFor(() => expect(backend.state.viewerVisible).toBe(false));
    expect(backend.callsOf("plugin:window|close")).toHaveLength(0);
    const next = { sequence: 2, profileId: "profile-1", pixelId: "next-pixel", x: 25, y: 40 };
    backend.state.viewerRequest = next;
    await act(async () => { await emit("screenshot-viewer://open", next); });
    const image = await loadScreenshot(1000, 2000);
    expect(image).toHaveAttribute("src", expect.stringContaining("next-pixel.png"));
    await waitFor(() => expect(backend.state.viewerVisible).toBe(true));
    expect(screen.getByRole("img", { name: "Пиксель (25, 40)" })).toBeInTheDocument();
    await user.keyboard("a");
    await waitFor(() => expect(backend.callsOf("screenshot_view_close")).toEqual([{ sequence: 1 }, { sequence: 2 }]));
  });

  it("waits for the cleared frame before hiding the reusable window", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);
    await waitFor(() => expect(backend.state.viewerVisible).toBe(true));
    await act(async () => {
      await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    });
    const frames: FrameRequestCallback[] = [];
    const frame = vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
      frames.push(callback);
      return frames.length;
    });
    try {
      await user.keyboard("{Escape}");
      expect(backend.callsOf("screenshot_view_close")).toHaveLength(0);
      await act(async () => { frames.shift()?.(0); });
      expect(backend.callsOf("screenshot_view_close")).toHaveLength(0);
      await act(async () => { frames.shift()?.(16); });
      await waitFor(() => expect(backend.state.viewerVisible).toBe(false));
      expect(backend.callsOf("screenshot_view_close")).toEqual([{ sequence: 1 }]);
    } finally {
      frame.mockRestore();
    }
  });

  it("does not show an old image that finishes decoding after a new request", async () => {
    const { backend } = renderOverlay(search);
    const image = await screen.findByRole("img", { name: "Снимок экрана" });
    let resolve!: () => void;
    const decoded = new Promise<void>((done) => { resolve = done; });
    Object.defineProperty(image, "decode", { value: vi.fn(() => decoded) });
    await loadScreenshot(1920, 1080);
    const next = { sequence: 2, profileId: "profile-1", pixelId: "next-pixel", x: 25, y: 40 };
    backend.state.viewerRequest = next;
    await act(async () => { await emit("screenshot-viewer://open", next); });
    await act(async () => { resolve(); await decoded; });
    expect(backend.callsOf("screenshot_view_ready")).toHaveLength(0);
    await loadScreenshot(1920, 1080);
    await waitFor(() => expect(backend.callsOf("screenshot_view_ready")).toEqual([{ sequence: 2 }]));
  });

  it("keeps the viewer hidden when an image fails to load", async () => {
    const { backend } = renderOverlay(search);
    const image = await screen.findByRole("img", { name: "Снимок экрана" });
    fireEvent.error(image);
    await waitFor(() => expect(backend.callsOf("screenshot_view_failed")).toEqual([{ sequence: 1 }]));
    expect(backend.state.viewerVisible).toBe(false);
    expect(backend.callsOf("screenshot_view_ready")).toHaveLength(0);
  });

  it("keeps the marker on the selected image pixel in a scaled preview", async () => {
    renderOverlay(search, 1.5);
    await loadScreenshot(3840, 2160);

    const marker = screen.getByRole("img", { name: "Пиксель (812, 1040)" });
    expect(marker).toHaveStyle({
      left: `${812.5 / 3840 * 100}%`,
      top: `${1040.5 / 2160 * 100}%`,
    });
  });

  it.each([
    [1920, 1080, 900, 506.25],
    [1000, 2000, 360, 720],
  ])("fits a %s by %s screenshot within 90 percent of the screen", async (width, height, shownWidth, shownHeight) => {
    Object.defineProperty(window, "innerWidth", { value: 1000, configurable: true });
    Object.defineProperty(window, "innerHeight", { value: 800, configurable: true });
    renderOverlay(search, 1.5);
    const image = await loadScreenshot(width, height);
    expect(image).toHaveStyle({ width: `${shownWidth}px`, height: `${shownHeight}px` });
    Object.defineProperty(window, "innerWidth", { value: 500, configurable: true });
    Object.defineProperty(window, "innerHeight", { value: 400, configurable: true });
    fireEvent(window, new Event("resize"));
    expect(image).toHaveStyle({ width: `${shownWidth / 2}px`, height: `${shownHeight / 2}px` });
  });

  it("closes when the screenshot itself is clicked", async () => {
    const { user, backend } = renderOverlay(search);
    const image = await loadScreenshot(1920, 1080);
    await user.pointer({ keys: "[MouseRight]", target: image });
    await waitFor(() => expect(backend.callsOf("screenshot_view_close")).toHaveLength(1));
  });

  it("closes on a click", async () => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.pointer({ keys: "[MouseLeft]", target: document.body });

    await waitFor(() => expect(backend.callsOf("screenshot_view_close")).toHaveLength(1));
  });

  it.each(["a", "{Escape}", "{Enter}", "{Shift}"])("closes on key %s", async (key) => {
    const { user, backend } = renderOverlay(search);
    await loadScreenshot(1920, 1080);

    await user.keyboard(key);

    await waitFor(() => expect(backend.callsOf("screenshot_view_close")).toHaveLength(1));
  });
});
