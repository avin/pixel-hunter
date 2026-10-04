import { act, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Tracker } from "@/store/types";
import {
  createFakeBackend,
  profileFixture,
  trackerFixture,
  pixelFixture,
  keyStepFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

async function openTracker(tracker: Tracker = trackerFixture()) {
  const rendered = renderApp(
    createFakeBackend({ profiles: [profileFixture({ trackers: [tracker] })] }),
  );
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await rendered.user.click(screen.getByRole("link", { name: tracker.name }));
  return rendered;
}

const nameInput = () => screen.getByRole("textbox", { name: "Имя" });
const minInput = () => screen.getByRole("spinbutton", { name: "Минимум" });
const maxInput = () => screen.getByRole("spinbutton", { name: "Максимум" });
const pauseInput = () =>
  screen.getByRole("spinbutton", { name: "Пауза до возобновления опроса, мс" });
const exitLink = () => screen.getByRole("link", { name: "Назад" });

describe("TrackerPage", () => {
  it("shows default timings of a new tracker", async () => {
    const { user } = renderApp(createFakeBackend({ profiles: [profileFixture()] }));
    await user.click(await screen.findByRole("link", { name: "Diablo" }));
    await user.click(screen.getByRole("button", { name: "Создать отслеживание" }));
    await user.type(screen.getByRole("textbox", { name: "Имя" }), "Здоровье");
    await user.click(screen.getByRole("button", { name: "Создать" }));
    await user.click(await screen.findByRole("link", { name: "Здоровье" }));

    expect(minInput()).toHaveValue(100);
    expect(maxInput()).toHaveValue(300);
    expect(pauseInput()).toHaveValue(1000);
  });

  it("saves a renamed tracker with new timings", async () => {
    const { user, backend } = await openTracker();

    await user.clear(nameInput());
    await user.type(nameInput(), "Мана");
    await user.clear(minInput());
    await user.type(minInput(), "50");
    await user.clear(maxInput());
    await user.type(maxInput(), "80");
    await user.click(exitLink());

    await screen.findByRole("heading", { level: 1, name: "Diablo" });
    expect(backend.state.profiles[0].trackers[0]).toMatchObject({
      name: "Мана",
      preActionDelayMs: { min: 50, max: 80 },
      resumeDelayMs: 1000,
    });
  });

  it("saves a zero pause", async () => {
    const { user, backend } = await openTracker();

    await user.clear(pauseInput());
    await user.type(pauseInput(), "0");
    await user.click(exitLink());

    await screen.findByRole("heading", { level: 1, name: "Diablo" });
    expect(backend.state.profiles[0].trackers[0].resumeDelayMs).toBe(0);
  });

  it("rejects a delay range with minimum above maximum", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(minInput());
    await user.type(minInput(), "500");
    await user.clear(maxInput());
    await user.type(maxInput(), "200");
    await user.click(exitLink());

    const error = "Минимум не может быть больше максимума";
    expect(screen.getByText(error)).toBeInTheDocument();
    expect(minInput()).toHaveAccessibleDescription(error);
    expect(maxInput()).toHaveAccessibleDescription(error);
    expect(minInput()).toHaveFocus();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });

  it("shows the range error only after focus leaves the range", async () => {
    const { user } = await openTracker();

    await user.clear(minInput());
    await user.type(minInput(), "500");
    await user.click(maxInput());
    expect(screen.queryByText("Минимум не может быть больше максимума")).not.toBeInTheDocument();

    await user.click(pauseInput());
    expect(screen.getByText("Минимум не может быть больше максимума")).toBeInTheDocument();

    await user.clear(maxInput());
    await user.type(maxInput(), "600");
    expect(screen.queryByText("Минимум не может быть больше максимума")).not.toBeInTheDocument();
  });

  it("rejects a negative pause", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(pauseInput());
    await user.type(pauseInput(), "-100");
    await user.tab();

    expect(pauseInput()).toHaveAccessibleDescription(
      expect.stringContaining("Введите целое неотрицательное число"),
    );

    await user.click(exitLink());

    expect(pauseInput()).toHaveFocus();
    expect(screen.queryByText("Отслеживание сохранено")).not.toBeInTheDocument();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });

  it("rejects an empty name", async () => {
    const tracker = trackerFixture();
    const { user, backend } = await openTracker(tracker);

    await user.clear(nameInput());
    await user.click(exitLink());

    expect(screen.getByText("Введите имя отслеживания")).toBeInTheDocument();
    expect(nameInput()).toHaveFocus();
    expect(backend.state.profiles[0].trackers[0]).toEqual(tracker);
  });
  it("keeps typing, blur and Enter local and skips unchanged writes", async () => {
    const { user, backend } = await openTracker();
    expect(screen.queryByRole("button", { name: "Сохранить" })).not.toBeInTheDocument();
    await user.type(nameInput(), " новый{Enter}");
    await user.tab();
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    await user.clear(nameInput());
    await user.type(nameInput(), "Здоровье");
    await user.click(exitLink());
    await screen.findByRole("heading", { name: "Diablo" });
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
  });

  it("saves before history navigation", async () => {
    const { user, backend, router } = await openTracker();
    await user.type(nameInput(), " новый");
    await act(async () => { await router.navigate(-1); });
    await screen.findByRole("heading", { name: "Diablo" });
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье новый");
  });

  it("keeps the original destination during a delayed save", async () => {
    const { user, backend, router } = await openTracker();
    const release = backend.hold("tracker_update");
    await user.type(nameInput(), " новый");
    await user.click(exitLink());
    await screen.findByText("Сохранение…");
    expect(nameInput()).toBeDisabled();
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await act(async () => { await router.navigate("/"); });
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await act(async () => { release(); });
    await screen.findByRole("heading", { name: "Diablo" });
    expect(router.state.location.pathname).toBe("/profiles/profile-1");
  });

  it("blocks profile switching after a write error and retries", async () => {
    const backend = createFakeBackend({ profiles: [
      profileFixture({ trackers: [trackerFixture()] }),
      profileFixture({ id: "other", name: "Другой" }),
    ] });
    const { user, router } = renderApp(backend, ["/profiles/profile-1/trackers/tracker-1"]);
    await screen.findByRole("textbox", { name: "Имя" });
    await user.type(nameInput(), " новый");
    backend.failOn("tracker_update", "Диск недоступен");
    await act(async () => { await router.navigate("/profiles/other"); });
    await screen.findByText("Диск недоступен", { selector: "p" });
    expect(nameInput()).toHaveValue("Здоровье новый");
    expect(router.state.location.pathname).toBe("/profiles/profile-1/trackers/tracker-1");
    expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "profile-1" }]);
    backend.failOn("tracker_update", undefined);
    await act(async () => { await router.navigate("/profiles/other"); });
    await screen.findByRole("heading", { name: "Другой" });
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье новый");
  });

  it("preserves draft through immediate pixel, action and run changes", async () => {
    const { user, backend } = await openTracker(trackerFixture({
      pixels: [pixelFixture()], action: { type: "keySequence", steps: [keyStepFixture()] },
    }));
    await user.type(nameInput(), " новый");
    const tolerance = screen.getByRole("spinbutton", { name: "Допуск пикселя (812, 1040), %" });
    await user.clear(tolerance);
    await user.type(tolerance, "25");
    await user.tab();
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].pixels[0].tolerancePercent).toBe(25));
    await user.click(screen.getByRole("button", { name: "Добавить нажатие" }));
    await user.keyboard("w");
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].action.steps).toHaveLength(2));
    await user.click(screen.getByRole("button", { name: "Запуск отслеживания «Здоровье»" }));
    await waitFor(() => expect(backend.state.profiles[0].trackers[0].isRunning).toBe(true));
    expect(nameInput()).toHaveValue("Здоровье новый");
    await user.click(exitLink());
    await screen.findByRole("heading", { name: "Diablo" });
    expect(backend.state.profiles[0].trackers[0]).toMatchObject({ name: "Здоровье новый", isRunning: true, pixels: [{ tolerancePercent: 25 }], action: { steps: [{}, { combo: { key: "KeyW" } }] } });
  });

  it("saves the source draft before opening another tracker", async () => {
    const backend = createFakeBackend({ profiles: [profileFixture({ trackers: [
      trackerFixture(), trackerFixture({ id: "second", name: "Мана" }),
    ] })] });
    const { user } = renderApp(backend, ["/profiles/profile-1/trackers/tracker-1"]);
    await screen.findByRole("textbox", { name: "Имя" });
    await user.clear(nameInput());
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Мана»" }));
    expect(nameInput()).toHaveFocus();
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    await user.type(nameInput(), "Новое имя");
    await user.click(screen.getByRole("link", { name: "Открыть отслеживание «Мана»" }));
    await screen.findByRole("heading", { name: "Мана" });
    expect(nameInput()).toHaveValue("Мана");
    expect(backend.state.profiles[0].trackers[0].name).toBe("Новое имя");
    expect(backend.callsOf("engine_activate")).toHaveLength(1);
  });

});
