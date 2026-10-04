import { emit } from "@tauri-apps/api/event";
import { act, fireEvent, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { CAPTURE_STATUS_EVENT } from "@/hooks/usePixelCaptureEvents";
import type { CaptureStatus, Pixel } from "@/store/types";
import {
  createFakeBackend,
  pixelFixture,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

async function openTracker(pixels: Pixel[] = []) {
  const rendered = renderApp(
    createFakeBackend({
      profiles: [profileFixture({ trackers: [trackerFixture({ pixels })] })],
    }),
  );
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await rendered.user.click(screen.getByRole("link", { name: "Здоровье" }));
  await screen.findByRole("heading", { name: "Пиксели" });
  return rendered;
}

async function emitStatus(status: CaptureStatus) {
  await act(() => emit(CAPTURE_STATUS_EVENT, status));
}

const addButton = () => screen.getByRole("button", { name: "Добавить пиксель" });
const row = (point: string) =>
  screen.getByRole("cell", { name: point }).closest("tr") as HTMLElement;
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
    expect(screen.getByRole("status")).toHaveTextContent(
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

    await emitStatus({ phase: "countdown", remainingMs: 2_100 });
    expect(screen.getByRole("status")).toHaveTextContent("Снимок экрана через");
    expect(screen.getByText("3 с")).toBeInTheDocument();

    await emitStatus({ phase: "picking" });
    expect(screen.getByRole("status")).toHaveTextContent("Выберите точку на снимке");
    expect(addButton()).toBeDisabled();

    backend.state.profiles[0].trackers[0].pixels.push(pixelFixture());
    await emitStatus({ phase: "done" });

    expect(await screen.findByRole("cell", { name: "812, 1040" })).toBeInTheDocument();
    expect(conditionSelect()).toHaveTextContent("Равно");
    expect(toleranceInput()).toHaveValue(10);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
  });

  it("cancels the capture", async () => {
    const { user, backend } = await openTracker();
    await user.click(addButton());

    await user.click(screen.getByRole("button", { name: "Отмена" }));
    expect(backend.callsOf("pixel_capture_cancel")).toHaveLength(1);

    await emitStatus({ phase: "cancelled" });
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
    expect(screen.getByText("Пикселей пока нет")).toBeInTheDocument();
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
    expect(screen.getByRole("status")).toHaveTextContent("Переключитесь в окно игры");
  });

  it("shows the error when a capture cannot start", async () => {
    const { user, backend } = await openTracker();
    backend.failOn("pixel_capture_start", "Захват пикселя уже идёт");

    await user.click(addButton());

    expect(await screen.findByText("Захват пикселя уже идёт")).toBeInTheDocument();
    await waitFor(() => expect(addButton()).toBeEnabled());
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
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

  it("lists every pixel with its thumbnail, color, condition and tolerance", async () => {
    await openTracker([pixelFixture(), secondPixel]);

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
    expect(conditionSelect()).toHaveTextContent("Равно");
    expect(toleranceInput()).toHaveValue(10);

    expect(within(row("10, 20")).getByText("#0080FF")).toBeInTheDocument();
    expect(conditionSelect("(10, 20)")).toHaveTextContent("Не равно");
    expect(toleranceInput("(10, 20)")).toHaveValue(0);
  });

  it("offers to add a pixel when there are none", async () => {
    await openTracker();

    expect(screen.getByText("Пикселей пока нет")).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
    expect(addButton()).toBeEnabled();
  });

  it("opens the screenshot viewer from the thumbnail", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.click(
      screen.getByRole("button", { name: "Открыть снимок пикселя (812, 1040)" }),
    );

    expect(backend.callsOf("pixel_view_open")).toEqual([
      { profileId: "profile-1", trackerId: "tracker-1", pixelId: "pixel-1" },
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
    await user.click(await screen.findByRole("option", { name: "Не равно" }));

    await waitFor(() =>
      expect(backend.state.profiles[0].trackers[0].pixels[0].condition).toBe("notEquals"),
    );
  });

  it("saves the chosen condition", async () => {
    const { user, backend } = await openTracker([pixelFixture()]);

    await user.click(conditionSelect());
    await user.click(await screen.findByRole("option", { name: "Не равно" }));

    await waitFor(() => expect(conditionSelect()).toHaveTextContent("Не равно"));
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
      expect(screen.queryByRole("cell", { name: "812, 1040" })).not.toBeInTheDocument(),
    );
    expect(screen.getByRole("cell", { name: "10, 20" })).toBeInTheDocument();
    expect(backend.callsOf("pixel_delete")).toEqual([
      { profileId: "profile-1", trackerId: "tracker-1", pixelId: "pixel-1" },
    ]);
  });
});
