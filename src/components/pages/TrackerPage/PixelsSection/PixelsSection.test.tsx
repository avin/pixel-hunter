import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CAPTURE_STATUS_EVENT } from "@/hooks/usePixelCaptureEvents";
import type { CaptureStatus, Icon, Pixel } from "@/store/types";
import {
  createFakeBackend,
  pixelFixture,
  iconFixture,
  keyStepFixture,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

async function openTracker(pixels: Pixel[] = [], icons: Icon[] = []) {
  const rendered = renderApp(
    createFakeBackend({
      profiles: [profileFixture({ trackers: [trackerFixture({ pixels, icons })] })],
    }),
  );
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await rendered.user.click(screen.getByRole("link", { name: "Здоровье" }));
  await screen.findByRole("button", { name: "Добавить пиксель" });
  return rendered;
}

async function emitStatus(status: CaptureStatus) {
  await act(() => emit(CAPTURE_STATUS_EVENT, status));
}

const addButton = () => screen.getByRole("button", { name: "Добавить пиксель" });
const addIconButton = () => screen.getByRole("button", { name: "Добавить иконку" });
const iconCondition = () => screen.getByRole("combobox", { name: "Условие иконки (40, 50)" });
const iconThreshold = () => screen.getByRole("spinbutton", { name: "Сходство иконки (40, 50), %" });
const row = (point: string) =>
  screen.getByRole("combobox", { name: `Условие пикселя (${point})` }).closest("tr") as HTMLElement;
const conditionSelect = (point = "(812, 1040)") =>
  screen.getByRole("combobox", { name: `Условие пикселя ${point}` });
const toleranceInput = (point = "(812, 1040)") =>
  screen.getByRole("spinbutton", { name: `Допуск пикселя ${point}, %` });

describe("Pixel capture on TrackerPage", () => {
  it("waits for the game with a countdown from 10 seconds", async () => {
    const { user, backend } = await openTracker();

    await user.click(addButton());

    expect(backend.callsOf("pixel_capture_start")).toEqual([
      { profileId: "profile-1", trackerId: "tracker-1" },
    ]);
    expect(screen.getByRole("status", { name: "Состояние захвата" })).toHaveTextContent(
      "Переключитесь в окно игры game.exe",
    );
    expect(screen.getByText("10 с")).toBeInTheDocument();
    expect(addButton()).toBeDisabled();
  });

  it("follows the capture phases and shows the added pixel", async () => {
    const { user, backend } = await openTracker();
    await user.click(addButton());

    await emitStatus({ phase: "waitingForGame", remainingMs: 7_400 });
    expect(screen.getByText("8 с")).toBeInTheDocument();

    await emitStatus({ phase: "countdown", remainingMs: 200 });
    expect(screen.getByRole("status", { name: "Состояние захвата" })).toHaveTextContent(
      "Делаем снимок экрана...",
    );
    expect(screen.queryByText("200 мс")).not.toBeInTheDocument();

    await emitStatus({ phase: "picking" });
    expect(screen.getByRole("status", { name: "Состояние захвата" })).toHaveTextContent("Выберите точку на снимке");
    expect(addButton()).toBeDisabled();

    backend.state.profiles[0].trackers[0].pixels.push(pixelFixture());
    await emitStatus({ phase: "done" });

    expect(await screen.findByRole("combobox", { name: "Условие пикселя (812, 1040)" })).toBeInTheDocument();
    expect(conditionSelect()).toHaveTextContent("Равен цвету");
    expect(toleranceInput()).toHaveValue(10);
    expect(screen.queryByRole("status", { name: "Состояние захвата" })).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
  });

  it("cancels the capture", async () => {
    const { user, backend } = await openTracker();
    await user.click(addButton());

    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(backend.callsOf("pixel_capture_cancel")).toHaveLength(1);

    await emitStatus({ phase: "cancelled" });
    expect(screen.queryByRole("status", { name: "Состояние захвата" })).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
    expect(await screen.findByText("Пикселей и иконок пока нет")).toBeInTheDocument();
  });

  it("reports that the game window was not activated", async () => {
    const { user } = await openTracker();
    await user.click(addButton());

    await emitStatus({ phase: "timeout" });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Окно игры не стало активным за 10 секунд",
    );
    expect(addButton()).toBeEnabled();

    await user.click(screen.getByRole("button", { name: "Закрыть" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("reports a failed snapshot", async () => {
    const { user } = await openTracker();
    await user.click(addButton());

    await emitStatus({ phase: "failed", reason: "Не удалось захватить экран: доступ запрещён" });

    expect(screen.getByRole("alert")).toHaveTextContent(
      "Не удалось захватить экран: доступ запрещён",
    );
    expect(addButton()).toBeEnabled();
  });

  it("keeps the button disabled after leaving the tracker during a capture", async () => {
    const { user } = await openTracker();
    await user.click(addButton());

    await user.click(screen.getByRole("link", { name: "Назад" }));
    await user.click(screen.getByRole("link", { name: "Здоровье" }));

    expect(await screen.findByRole("button", { name: "Добавить пиксель" })).toBeDisabled();
    expect(screen.getByRole("status", { name: "Состояние захвата" })).toHaveTextContent("Переключитесь в окно игры");
  });

  it("shows the error when a capture cannot start", async () => {
    const { user, backend } = await openTracker();
    backend.failOn("pixel_capture_start", "Захват пикселя уже идёт");

    await user.click(addButton());

    expect(await screen.findByText("Захват пикселя уже идёт")).toBeInTheDocument();
    await waitFor(() => expect(addButton()).toBeEnabled());
    expect(screen.queryByRole("status", { name: "Состояние захвата" })).not.toBeInTheDocument();
  });
});

describe("Pixel table on TrackerPage", () => {
  const secondPixel = pixelFixture({
    id: "pixel-2",
    x: 10,
    y: 20,
    color: { r: 0, g: 128, b: 255 },
    condition: "notEquals",
    tolerancePercent: 0,
  });

  it("lists pixels and icons together without column headings", async () => {
    await openTracker([pixelFixture(), secondPixel], [iconFixture()]);

    const table = screen.getByRole("table", { name: "Визуальные условия" });
    expect(screen.getAllByRole("table")).toHaveLength(1);
    expect(within(table).queryByRole("columnheader")).not.toBeInTheDocument();
    expect(within(table).getAllByRole("cell", { name: "Пиксель" })).toHaveLength(2);
    expect(within(table).getByRole("cell", { name: "Иконка" })).toBeInTheDocument();
    expect(iconCondition()).toHaveTextContent("Должна присутствовать");
    expect(iconThreshold()).toHaveValue(90);

    const first = row("812, 1040");
    const thumbnail = within(first).getByRole("button", {
      name: "Открыть снимок пикселя (812, 1040)",
    });
    expect(thumbnail.querySelector("img")).toHaveAttribute(
      "src",
      expect.stringContaining(
        encodeURIComponent("C:\\data\\screenshots\\profile-1\\pixel-1.thumb.png"),
      ),
    );
    expect(within(first).getByText("#C81E1E")).toBeInTheDocument();
    expect(conditionSelect()).toHaveTextContent("Равен цвету");
    expect(toleranceInput()).toHaveValue(10);

    expect(within(row("10, 20")).getByText("#0080FF")).toBeInTheDocument();
    expect(conditionSelect("(10, 20)")).toHaveTextContent("Не равен цвету");
    expect(toleranceInput("(10, 20)")).toHaveValue(0);
  });

  it("offers to add a pixel when there are none", async () => {
    await openTracker();

    expect(screen.getByText("Пикселей и иконок пока нет")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
  });

  it("opens the screenshot viewer from the thumbnail", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.pointer({
      keys: "[MouseLeft]",
      target: screen.getByRole("button", { name: "Открыть снимок пикселя (812, 1040)" }),
      coords: { clientX: 168, clientY: 320 },
    });

    expect(backend.callsOf("pixel_view_open")).toEqual([
      { profileId: "profile-1", trackerId: "tracker-1", pixelId: "pixel-1", origin: { x: 168, y: 320 } },
    ]);
  });

  it("keeps the row editable when the screenshot file is missing", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);
    const thumbnail = screen.getByRole("button", {
      name: "Открыть снимок пикселя (812, 1040)",
    });

    fireEvent.error(thumbnail.querySelector("img") as HTMLImageElement);

    expect(screen.getByText("Снимок недоступен")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Открыть снимок пикселя (812, 1040)" }),
    ).not.toBeInTheDocument();
    expect(screen.getByText("#C81E1E")).toBeInTheDocument();

    await user.click(conditionSelect());
    await user.click(await screen.findByRole("option", { name: "Не равен цвету" }));

    await waitFor(() =>
      expect(backend.state.profiles[0].trackers[0].pixels[0].condition).toBe("notEquals"),
    );
  });

  it("saves the chosen condition", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.click(conditionSelect());
    await user.click(await screen.findByRole("option", { name: "Не равен цвету" }));

    await waitFor(() => expect(conditionSelect()).toHaveTextContent("Не равен цвету"));
    expect(backend.state.profiles[0].trackers[0].pixels[0]).toMatchObject({
      condition: "notEquals",
      tolerancePercent: 10,
    });
  });

  it("saves the tolerance when the field loses focus", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.clear(toleranceInput());
    await user.type(toleranceInput(), "15");
    await user.tab();

    await waitFor(() =>
      expect(backend.state.profiles[0].trackers[0].pixels[0].tolerancePercent).toBe(15),
    );
  });

  it("saves the tolerance on Enter", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.clear(toleranceInput());
    await user.type(toleranceInput(), "100{Enter}");

    await waitFor(() =>
      expect(backend.state.profiles[0].trackers[0].pixels[0].tolerancePercent).toBe(100),
    );
  });

  it("rejects a tolerance above 100", async () => {
    const pixel = pixelFixture();
    const { user, backend } = await openTracker([pixel]);
    const error = "Введите целое число от 0 до 100";

    await user.clear(toleranceInput());
    await user.type(toleranceInput(), "120");
    expect(screen.queryByText(error)).not.toBeInTheDocument();

    await user.tab();

    expect(toleranceInput()).toHaveAccessibleDescription(error);
    expect(toleranceInput()).toBeInvalid();
    expect(backend.callsOf("pixel_update")).toEqual([]);
    expect(backend.state.profiles[0].trackers[0].pixels[0]).toEqual(pixel);
  });

  it("shows the tolerance error on Enter and follows later edits live", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);
    const error = "Введите целое число от 0 до 100";

    await user.clear(toleranceInput());
    await user.type(toleranceInput(), "{Enter}");
    expect(toleranceInput()).toHaveAccessibleDescription(error);

    await user.type(toleranceInput(), "5");
    expect(screen.queryByText(error)).not.toBeInTheDocument();

    await user.type(toleranceInput(), "000");
    expect(toleranceInput()).toHaveAccessibleDescription(error);
    expect(backend.callsOf("pixel_update")).toEqual([]);
  });

  it("deletes a pixel", async () => {
    const { user, backend } = await openTracker([pixelFixture(), secondPixel]);

    await user.click(screen.getByRole("button", { name: "Удалить пиксель (812, 1040)" }));

    await waitFor(() =>
      expect(screen.queryByRole("combobox", { name: "Условие пикселя (812, 1040)" })).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("combobox", { name: "Условие пикселя (10, 20)" })).toBeInTheDocument();
    expect(backend.callsOf("pixel_delete")).toEqual([
      { profileId: "profile-1", trackerId: "tracker-1", pixelId: "pixel-1" },
    ]);
  });
});

describe("Icon conditions on TrackerPage", () => {
  it("starts a separate icon capture and blocks both buttons through both selections", async () => {
    const { user, backend } = await openTracker();
    await user.click(addIconButton());
    expect(backend.callsOf("icon_capture_start")).toEqual([{ profileId: "profile-1", trackerId: "tracker-1" }]);
    expect(backend.callsOf("pixel_capture_start")).toEqual([]);
    for (const phase of ["selectingSearch", "selectingTemplate"] as const) {
      await emitStatus({ phase });
      expect(addButton()).toBeDisabled();
      expect(addIconButton()).toBeDisabled();
      expect(screen.getByRole("status", { name: "Состояние захвата" })).toHaveTextContent(
        phase === "selectingSearch" ? /область поиска/i : /образец/i,
      );
    }
    await emitStatus({ phase: "cancelled" });
    expect(addButton()).toBeEnabled();
    expect(addIconButton()).toBeEnabled();
  });

  it("blocks icon capture while picking a pixel", async () => {
    const { user } = await openTracker();
    await user.click(addButton());
    await emitStatus({ phase: "picking" });
    expect(addIconButton()).toBeDisabled();
  });

  it("shows an icon without pixels and opens its own screenshot", async () => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    expect(screen.queryByText("Пикселей и иконок пока нет")).not.toBeInTheDocument();
    expect(iconThreshold()).toHaveValue(90);
    expect(iconCondition()).toHaveTextContent("Должна присутствовать");
    await user.pointer({ keys: "[MouseLeft]", target: screen.getByRole("button", { name: "Открыть снимок иконки (40, 50)" }), coords: { clientX: 120, clientY: 240 } });
    expect(backend.callsOf("icon_view_open")).toEqual([{ profileId: "profile-1", trackerId: "tracker-1", iconId: "icon-1", origin: { x: 120, y: 240 } }]);
  });

  it("saves absence immediately while preserving a pending name and delay draft", async () => {
    const { user, backend } = await openTracker([pixelFixture()], [iconFixture()]);
    const name = screen.getByRole("textbox", { name: "Имя" });
    const minimum = screen.getByRole("spinbutton", { name: "Минимум" });
    await user.clear(name);
    await user.type(name, "Мана");
    await user.clear(minimum);
    await user.type(minimum, "125");
    await user.click(iconCondition());
    await user.click(await screen.findByRole("option", { name: "Не должна присутствовать" }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].icons[0].condition).toBe("absent"));
    expect(name).toHaveValue("Мана");
    expect(minimum).toHaveValue(125);
    expect(backend.state.profiles[0].trackers[0].pixels).toEqual([pixelFixture()]);
  });

  it.each([1, 100])("saves the inclusive threshold %s", async (value) => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    await user.clear(iconThreshold());
    await user.type(iconThreshold(), String(value));
    await user.tab();
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].icons[0].similarityPercent).toBe(value));
  });

  it.each(["0", "101", "1.5"])("rejects threshold %s after blur and follows later edits live", async (value) => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    const error = "Введите целое число от 1 до 100";
    await user.clear(iconThreshold());
    await user.type(iconThreshold(), value);
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.tab();
    expect(iconThreshold()).toBeInvalid();
    expect(iconThreshold()).toHaveAccessibleDescription(error);
    expect(backend.callsOf("icon_update")).toEqual([]);
    await user.clear(iconThreshold());
    await user.type(iconThreshold(), "5");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.clear(iconThreshold());
    expect(iconThreshold()).toHaveAccessibleDescription(error);
  });

  it("reveals threshold error on Enter", async () => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    await user.clear(iconThreshold());
    await user.keyboard("{Enter}");
    expect(iconThreshold()).toHaveAccessibleDescription("Введите целое число от 1 до 100");
    expect(backend.callsOf("icon_update")).toEqual([]);
  });

  it("keeps the condition editable when its viewing screenshot is missing", async () => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    const thumbnail = screen.getByRole("button", { name: "Открыть снимок иконки (40, 50)" });
    fireEvent.error(thumbnail.querySelector("img") as HTMLImageElement);
    expect(screen.getByText("Снимок недоступен")).toBeInTheDocument();
    await user.click(iconCondition());
    await user.click(await screen.findByRole("option", { name: "Не должна присутствовать" }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].icons[0].condition).toBe("absent"));
  });

  it("shows a placeholder when only the full screenshot is missing and keeps the template condition", async () => {
    const fullScreenshot = document.createElement("img");
    const probe = vi.spyOn(window, "Image").mockImplementation(class {
      constructor() { return fullScreenshot; }
    } as typeof Image);
    try {
      const { user, backend } = await openTracker([], [iconFixture()]);
      expect(screen.getByRole("button", { name: "Открыть снимок иконки (40, 50)" })).toBeInTheDocument();
      expect(fullScreenshot.src).toContain(encodeURIComponent("C:\\data\\screenshots\\profile-1\\icon-1.png"));
      fireEvent.error(fullScreenshot);
      expect(await screen.findByText("Снимок недоступен")).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Открыть снимок иконки (40, 50)" })).not.toBeInTheDocument();
      expect(iconThreshold()).toHaveValue(90);
      await user.click(iconCondition());
      await user.click(await screen.findByRole("option", { name: "Не должна присутствовать" }));
      await waitFor(() => expect(backend.state.profiles[0].trackers[0].icons[0].condition).toBe("absent"));
      expect(backend.state.profiles[0].trackers[0].icons[0].templateRect).toEqual(iconFixture().templateRect);
      expect(probe).toHaveBeenCalledTimes(1);
      expect(screen.getByText("Снимок недоступен")).toBeInTheDocument();
    } finally {
      probe.mockRestore();
    }
  });

  it("starts with one icon and stops after its last visual condition is deleted", async () => {
    const tracker = trackerFixture({ icons: [iconFixture()], action: { type: "keySequence", steps: [keyStepFixture()], sound: { id: null, volumePercent: 50 } } });
    const { user, backend } = renderApp(createFakeBackend({ profiles: [profileFixture({ trackers: [tracker] })] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await user.click(screen.getByRole("button", { name: /Старт/ }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true));
    await user.click(screen.getByRole("link", { name: "Здоровье" }));
    await user.click(await screen.findByRole("button", { name: "Удалить иконку (40, 50)" }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false));
    expect(await screen.findByText("Пикселей и иконок пока нет")).toBeInTheDocument();
  });

  it("hides both visual groups for the hotkey trigger and restores their saved conditions", async () => {
    const { user, backend } = await openTracker([pixelFixture()], [iconFixture()]);
    await user.click(screen.getByRole("combobox", { name: "Условие" }));
    await user.click(await screen.findByRole("option", { name: "Хоткей" }));
    await screen.findByRole("button", { name: "Назначить" });
    expect(screen.queryByRole("button", { name: "Добавить пиксель" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Добавить иконку" })).not.toBeInTheDocument();
    expect(screen.queryByRole("spinbutton", { name: "Сходство иконки (40, 50), %" })).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0].icons).toEqual([iconFixture()]);
    await user.click(screen.getByRole("combobox", { name: "Условие" }));
    await user.click(await screen.findByRole("option", { name: "Визуальное" }));
    expect(await screen.findByRole("spinbutton", { name: "Сходство иконки (40, 50), %" })).toHaveValue(90);
    expect(toleranceInput()).toHaveValue(10);
  });

  it("shows an unavailable sample error and clears it when availability returns", async () => {
    await openTracker([], [iconFixture()]);
    const status = { profileId: "profile-1", trackerId: "tracker-1", iconId: "icon-1", error: "Файл образца недоступен" };
    await act(() => emit("engine://icon-status", status));
    expect(await screen.findByRole("alert")).toHaveTextContent(status.error);
    expect(iconThreshold()).toBeEnabled();
    await act(() => emit("engine://icon-status", { ...status, error: null }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("keeps a failed threshold edit visible and preserves the persisted condition", async () => {
    const { user, backend } = await openTracker([], [iconFixture()]);
    backend.failOn("icon_update", "Не удалось сохранить иконку");
    await user.clear(iconThreshold());
    await user.type(iconThreshold(), "75");
    await user.tab();
    await waitFor(() => expect(backend.callsOf("icon_update")).toHaveLength(1));
    expect(iconThreshold()).toHaveValue(75);
    expect(backend.state.profiles[0].trackers[0].icons[0]).toEqual(iconFixture());
  });
});
