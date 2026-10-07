import { fireEvent, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { createFakeBackend, profileFixture, trackerFixture } from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

describe("ActionSection", () => {
  async function openTracker() {
    const backend = createFakeBackend({ profiles: [profileFixture({ trackers: [trackerFixture(), trackerFixture({ id: "tracker-2", name: "Мана" })] })] });
    const rendered = renderApp(backend);
    await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
    await rendered.user.click(screen.getByRole("link", { name: "Здоровье" }));
    await screen.findByRole("button", { name: "Добавить нажатие" });
    return rendered;
  }

  it("previews a separate menu item by pointer and keyboard without selecting or closing", async () => {
    const { user, backend } = await openTracker();
    const sound = screen.getByRole("combobox", { name: "Звук" });
    expect(screen.getByRole("slider", { name: "Громкость" })).toBeDisabled();
    await user.click(sound);
    const play = await screen.findByRole("button", { name: "Прослушать Bright Bell" });
    await user.click(play);
    await waitFor(() => expect(backend.callsOf("action_sound_preview")).toHaveLength(1));
    expect(sound).toHaveTextContent("Без звука");
    expect(play).toBeInTheDocument();
    play.focus();
    await user.keyboard("{Enter}");
    await waitFor(() => expect(backend.callsOf("action_sound_preview")).toHaveLength(2));
    expect(backend.callsOf("action_sound_preview")[1]).toEqual({ id: "bright-bell", volumePercent: 50 });
    await user.click(screen.getByRole("button", { name: "Bright Bell" }));
    await waitFor(() => expect(sound).toHaveTextContent("Bright Bell"));
    expect(screen.queryByRole("button", { name: "Прослушать Bright Bell" })).not.toBeInTheDocument();
    expect(backend.callsOf("action_sound_preview")).toHaveLength(2);
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].action.sound.id).toBe("bright-bell"));
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Мана»" }));
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }));
    expect(screen.getByRole("combobox", { name: "Звук" })).toHaveTextContent("Bright Bell");
    expect(backend.callsOf("tracker_set_running")).toHaveLength(0);
  });

  it("coalesces sound and step edits while saving and finishes the original tracker after navigation", async () => {
    const { user, backend } = await openTracker();
    const release = backend.hold("action_update");
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await user.click(await screen.findByRole("button", { name: "Soft Ping" }));
    fireEvent.change(screen.getByRole("slider", { name: "Громкость" }), { target: { value: "35" } });
    await user.click(screen.getByRole("button", { name: "Добавить нажатие" }));
    await user.keyboard("q");
    expect(screen.getByRole("table", { name: "Шаги последовательности" })).toHaveTextContent("Q");
    expect(backend.callsOf("action_update")).toHaveLength(1);
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Мана»" }));
    release();
    await waitFor(() => {
      expect(backend.state.profiles[0].trackers[0].action.sound).toEqual({ id: "soft-ping", volumePercent: 35 });
      expect(backend.state.profiles[0].trackers[0].action.steps[0].combo.key).toBe("KeyQ");
    });
    expect(backend.callsOf("action_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[1].action.steps).toHaveLength(0);
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Здоровье»" }));
    expect(screen.getByRole("slider", { name: "Громкость" })).toHaveValue("35");
  });

  it("keeps failed desired edits visible and retries saving without claiming persistence", async () => {
    const { user, backend } = await openTracker();
    backend.failOn("action_update", "Диск недоступен");
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await user.click(await screen.findByRole("button", { name: "Soft Ping" }));
    expect(await screen.findByText("Не удалось сохранить действие: Диск недоступен")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Звук" })).toHaveTextContent("Soft Ping");
    expect(backend.state.profiles[0].trackers[0].action.sound.id).toBeNull();
    backend.failOn("action_update", undefined);
    await user.click(screen.getByRole("button", { name: "Повторить сохранение" }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].action.sound.id).toBe("soft-ping"));
    expect(screen.queryByText("Не удалось сохранить действие: Диск недоступен")).not.toBeInTheDocument();
  });

  it("shows preview failures without changing the saved sound", async () => {
    const { user, backend } = await openTracker();
    backend.failOn("action_sound_preview", "Нет устройства");
    await user.click(screen.getByRole("combobox", { name: "Звук" }));
    await user.click(await screen.findByRole("button", { name: "Прослушать Soft Ping" }));
    expect(await screen.findByText("Не удалось прослушать звук. Проверьте аудиоустройство.")).toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].action.sound.id).toBeNull();
  });

  it("gives a new tracker an empty key sequence", async () => {
    const { user } = renderApp(createFakeBackend({ profiles: [profileFixture()] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await user.click(screen.getByRole("button", { name: "Создать отслеживание" }));
    await user.type(screen.getByRole("textbox", { name: "Имя" }), "Здоровье");
    await user.click(screen.getByRole("button", { name: "Создать" }));
    await user.click(await screen.findByRole("link", { name: "Здоровье" }));

    const typeSelect = screen.getByRole("combobox", { name: "Тип" });
    expect(typeSelect).toHaveTextContent("Нажатие клавиш");
    expect(await screen.findByText("Нажатия не настроены")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Добавить нажатие" })).toBeEnabled();

    await user.click(typeSelect);
    expect(
      await screen.findByRole("option", { name: "Нажатие клавиш" }),
    ).toBeInTheDocument();
  });
});
