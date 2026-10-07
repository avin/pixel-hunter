import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TrackerSplit } from "./TrackerSplit";

const storageKey = "pixel-hunter.tracker-editor.pixels-height-percent.v1";
let height = 612;
let onResize: () => void;

function renderSplit() {
  return render(
    <TrackerSplit
      pixels={<section aria-label="Пиксели">Пиксели</section>}
      action={<section aria-label="Действие">Действие</section>}
    />,
  );
}

function separator() {
  return screen.getByRole("separator", { name: "Высота области пикселей" });
}

beforeEach(() => {
  localStorage.clear();
  height = 612;
  vi.stubGlobal("ResizeObserver", class {
    constructor(callback: () => void) { onResize = callback; }
    observe() {}
    disconnect() {}
  });
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (this: HTMLElement) {
    return { height: this.getAttribute("role") === "separator" ? 12 : height } as DOMRect;
  });
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  localStorage.clear();
});

describe("TrackerSplit", () => {
  it("skips the separator when tabbing between section controls", async () => {
    const user = userEvent.setup();
    render(
      <TrackerSplit
        pixels={<button>Пиксели</button>}
        action={<button>Действие</button>}
      />,
    );
    await user.tab();
    expect(screen.getByRole("button", { name: "Пиксели" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "Действие" })).toHaveFocus();
    separator().focus();
    expect(screen.getByRole("button", { name: "Действие" })).toHaveFocus();
  });

  it("keeps both sections visible at the stored ratio limits", () => {
    localStorage.setItem(storageKey, "20");
    const rendered = renderSplit();
    // 144 px of the available 600 px must remain visible.
    expect(separator()).toHaveAttribute("aria-valuenow", "24");
    expect(separator()).toHaveAttribute("aria-valuemin", "24");
    rendered.unmount();
    localStorage.setItem(storageKey, "80");
    renderSplit();
    expect(separator()).toHaveAttribute("aria-valuenow", "76");
    expect(separator()).toHaveAttribute("aria-valuemax", "76");
  });

  it.each([null, "", "broken", "NaN", "Infinity", "19", "81"])("falls back to equal heights for %s", (stored) => {
    if (stored !== null) localStorage.setItem(storageKey, stored);
    renderSplit();
    expect(separator()).toHaveAttribute("aria-valuenow", "50");
  });

  it("temporarily constrains a small window and restores the preferred ratio after resize", () => {
    localStorage.setItem(storageKey, "35");
    renderSplit();
    expect(separator()).toHaveAttribute("aria-valuenow", "35");
    height = 312;
    act(() => onResize());
    expect(separator()).toHaveAttribute("aria-valuenow", "48");
    expect(localStorage.getItem(storageKey)).toBe("35");
    height = 1212;
    act(() => onResize());
    expect(separator()).toHaveAttribute("aria-valuenow", "35");
    expect(localStorage.getItem(storageKey)).toBe("35");
  });

  it("moves while storage access is unavailable and keeps the ratio for the session", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => { throw new Error("Unavailable"); });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => { throw new Error("Unavailable"); });
    const user = userEvent.setup();
    const rendered = renderSplit();
    const handle = separator();
    handle.setPointerCapture = vi.fn();
    handle.releasePointerCapture = vi.fn();
    const initial = Number(separator().getAttribute("aria-valuenow"));
    await user.pointer({ keys: "[MouseLeft>]", target: handle, coords: { clientY: 300 } });
    await user.pointer({ target: handle, coords: { clientY: 294 } });
    await user.pointer({ keys: "[/MouseLeft]", target: handle });
    expect(separator()).toHaveAttribute("aria-valuenow", String(initial - 1));
    rendered.unmount();
    renderSplit();
    expect(separator()).toHaveAttribute("aria-valuenow", String(initial - 1));
  });

  it.each(["release", "cancel"])("captures the pointer, clamps dragging, and saves on %s", async (ending) => {
    const user = userEvent.setup();
    const rendered = renderSplit();
    const handle = separator();
    const capture = vi.fn();
    const release = vi.fn();
    handle.setPointerCapture = capture;
    handle.releasePointerCapture = release;
    await user.pointer({ keys: "[MouseLeft>]", target: handle, coords: { clientY: 300 } });
    expect(handle).not.toHaveFocus();
    await user.pointer({ target: handle, coords: { clientY: 210 } });
    expect(separator()).toHaveAttribute("aria-valuenow", "35");
    expect(localStorage.getItem(storageKey)).toBeNull();
    await user.pointer({ target: handle, coords: { clientY: 1000 } });
    expect(separator()).toHaveAttribute("aria-valuenow", "76");
    expect(capture).toHaveBeenCalledOnce();
    if (ending === "release") {
      await user.pointer({ keys: "[/MouseLeft]", target: handle });
    } else {
      act(() => handle.dispatchEvent(new PointerEvent("pointercancel", { bubbles: true, pointerId: capture.mock.calls[0][0] })));
    }
    expect(release).toHaveBeenCalledOnce();
    expect(localStorage.getItem(storageKey)).toBe("76");
    await user.pointer({ target: handle, coords: { clientY: 100 } });
    expect(separator()).toHaveAttribute("aria-valuenow", "76");
    rendered.unmount();
    renderSplit();
    expect(separator()).toHaveAttribute("aria-valuenow", "76");
  });
});
