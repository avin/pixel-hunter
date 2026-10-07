import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { Provider } from "react-redux";
import { describe, expect, it, vi } from "vitest";
import { createAppStore } from "@/store";
import type { SoundSettings as Settings } from "@/store/types";
import { createFakeBackend } from "@/test/fakeBackend";
import { SoundSettings } from "./SoundSettings";

function SettingsHarness() {
  const [sound, setSound] = useState<Settings>({ id: "soft-ping", volumePercent: 50 });
  return <SoundSettings sound={sound} onChange={setSound} />;
}

describe("sound volume preview", () => {
  async function setup() {
    const backend = createFakeBackend();
    backend.install();
    const rendered = render(<Provider store={createAppStore()}><SettingsHarness /></Provider>);
    const user = userEvent.setup();
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await screen.findByRole("button", { name: "Прослушать Soft Ping" });
    await user.keyboard("{Escape}");
    return { backend, ...rendered };
  }

  it("previews the current volume on click even when it does not change", async () => {
    const { backend } = await setup();
    const user = userEvent.setup();
    const slider = screen.getByRole("slider", { name: "Громкость" });
    await user.pointer({ target: slider, keys: "[MouseLeft>]" });
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
    await user.pointer({ keys: "[/MouseLeft]" });
    expect(slider).toHaveValue("50");
    expect(backend.callsOf("action_sound_preview")).toEqual([{ id: "soft-ping", volumePercent: 50 }]);
    await user.click(slider);
    expect(backend.callsOf("action_sound_preview")).toHaveLength(2);
  });

  it("updates volume while dragging and previews only the final value on release", async () => {
    const { backend } = await setup();
    vi.useFakeTimers();
    const slider = screen.getByRole("slider", { name: "Громкость" });
    fireEvent.pointerDown(slider, { pointerId: 1 });
    fireEvent.change(slider, { target: { value: "20" } });
    fireEvent.change(slider, { target: { value: "30" } });
    expect(slider).toHaveValue("30");
    expect(slider).toHaveAttribute("aria-valuetext", "30%");
    await act(() => vi.advanceTimersByTimeAsync(500));
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
    fireEvent.pointerUp(slider, { pointerId: 1 });
    expect(backend.callsOf("action_sound_preview")).toEqual([{ id: "soft-ping", volumePercent: 30 }]);
    fireEvent.change(slider, { target: { value: "35" } });
    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(backend.callsOf("action_sound_preview")).toHaveLength(1);
    fireEvent.pointerUp(slider, { pointerId: 1 });
    expect(backend.callsOf("action_sound_preview")[1]).toEqual({ id: "soft-ping", volumePercent: 35 });
    fireEvent.blur(slider);
    expect(backend.callsOf("action_sound_preview")).toHaveLength(2);
  });

  it("does not preview zero volume or unfinished edits after unmount", async () => {
    const { backend, unmount } = await setup();
    vi.useFakeTimers();
    const slider = screen.getByRole("slider", { name: "Громкость" });
    fireEvent.change(slider, { target: { value: "20" } });
    fireEvent.change(slider, { target: { value: "0" } });
    fireEvent.pointerDown(slider);
    fireEvent.pointerUp(slider);
    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
    expect(backend.callsOf("action_sound_stop_preview")).toHaveLength(1);
    fireEvent.change(slider, { target: { value: "30" } });
    unmount();
    await act(() => vi.advanceTimersByTimeAsync(100));
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
    expect(backend.callsOf("action_sound_stop_preview")).toHaveLength(2);
  });

  it("does not preview a cancelled drag", async () => {
    const { backend } = await setup();
    const slider = screen.getByRole("slider", { name: "Громкость" });
    fireEvent.pointerDown(slider, { pointerId: 1 });
    fireEvent.change(slider, { target: { value: "25" } });
    fireEvent.pointerCancel(slider, { pointerId: 1 });
    fireEvent.blur(slider);
    expect(slider).toHaveValue("25");
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
  });

  it("previews keyboard changes only when the adjustment key is released", async () => {
    const { backend } = await setup();
    const user = userEvent.setup();
    const slider = screen.getByRole("slider", { name: "Громкость" });
    slider.focus();
    await user.keyboard("{ArrowLeft>2}");
    expect(slider).toHaveValue("48");
    expect(slider).toHaveAttribute("aria-valuetext", "48%");
    expect(backend.callsOf("action_sound_preview")).toHaveLength(0);
    await user.keyboard("{/ArrowLeft}");
    await waitFor(() => expect(backend.callsOf("action_sound_preview")).toEqual([{ id: "soft-ping", volumePercent: 48 }]));
  });

  it("stops preview on silence and preserves the volume while allowing other menu previews", async () => {
    const { backend } = await setup();
    const user = userEvent.setup();
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await user.click(await screen.findByRole("button", { name: "Без звука" }));
    expect(screen.getByRole("slider", { name: "Громкость" })).toBeDisabled();
    expect(screen.getByRole("slider", { name: "Громкость" })).toHaveValue("50");
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await user.click(await screen.findByRole("button", { name: "Прослушать Bright Bell" }));
    expect(backend.callsOf("action_sound_preview")).toEqual([{ id: "bright-bell", volumePercent: 50 }]);
  });
});
