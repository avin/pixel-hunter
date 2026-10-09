import { act, cleanup, configure, getConfig, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
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

const asyncWrapper = getConfig().asyncWrapper;
afterEach(() => {
  vi.useRealTimers();
  configure({ asyncWrapper });
});

function timedUser() {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] });
  configure({ asyncWrapper: async (callback) => {
    let result!: Awaited<ReturnType<typeof callback>>;
    await act(async () => { result = await callback(); });
    return result;
  } });
  return userEvent.setup({ advanceTimers: vi.advanceTimersByTime });
}

async function advance(ms: number) {
  await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
}

describe("TrackerPage", () => {
  it("shows timing descriptions only after activating an info button", async () => {
    const { user } = await openTracker();
    const delayDescription = "Перед действием выдерживается случайная пауза из этого диапазона.";
    const resumeDescription = "Сколько ждать после действия, прежде чем снова проверять пиксели.";

    expect(screen.queryByText(delayDescription)).not.toBeInTheDocument();
    expect(screen.queryByText(resumeDescription)).not.toBeInTheDocument();
    const delayInfo = screen.getByRole("button", { name: "Информация: Задержка перед действием" });
    await user.hover(delayInfo);
    expect(screen.queryByText(delayDescription)).not.toBeInTheDocument();
    await user.click(delayInfo);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(delayDescription);
    await user.unhover(delayInfo);
    expect(screen.getByRole("tooltip")).toHaveTextContent(delayDescription);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());

    await user.click(pauseInput());
    await user.tab({ shift: true });
    expect(screen.getByRole("button", { name: "Информация: Пауза до возобновления опроса" })).toHaveFocus();
    expect(screen.queryByText(resumeDescription)).not.toBeInTheDocument();
    await user.keyboard("{Enter}");
    expect(await screen.findByRole("tooltip")).toHaveTextContent(resumeDescription);
    await user.click(nameInput());
    await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
    expect(minInput()).toBeInTheDocument();
    expect(maxInput()).toBeInTheDocument();
  });

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

  it("shows and clears range errors during typing", async () => {
    const { user } = await openTracker();

    await user.clear(minInput());
    await user.type(minInput(), "500");
    expect(minInput()).toHaveFocus();
    expect(screen.getByText("Минимум не может быть больше максимума")).toBeInTheDocument();

    await user.clear(maxInput());
    await user.type(maxInput(), "600");
    expect(screen.queryByText("Минимум не может быть больше максимума")).not.toBeInTheDocument();
  });

  it("debounces the latest settings without blur or exit", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    await user.type(nameInput(), " новый{Enter}");
    await user.tab();
    await advance(499);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    await user.clear(pauseInput());
    await user.type(pauseInput(), "2000");
    await advance(499);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    await advance(1);
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    expect(backend.state.profiles[0].trackers[0]).toMatchObject({ name: "Здоровье новый", resumeDelayMs: 2000 });
    await user.click(exitLink());
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
  });

  it("skips equivalent and reverted drafts", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    await user.type(nameInput(), " новый");
    await advance(300);
    await user.clear(nameInput());
    await user.type(nameInput(), " Здоровье ");
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
  });

  it.each(["", "-1", "1.5", "4294967296"])("keeps invalid pause %s local until corrected", async (value) => {
    const { backend } = await openTracker();
    const user = timedUser();
    await user.clear(pauseInput());
    if (value) await user.type(pauseInput(), value);
    expect(pauseInput()).toHaveFocus();
    expect(pauseInput()).toHaveAttribute("aria-invalid", "true");
    expect(pauseInput()).toHaveAccessibleDescription();
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    await user.clear(pauseInput());
    await user.type(pauseInput(), "0");
    expect(pauseInput()).not.toHaveAttribute("aria-invalid", "true");
    await advance(500);
    expect(backend.state.profiles[0].trackers[0].resumeDelayMs).toBe(0);
  });

  it("does not partially save a draft with an empty name", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    await user.clear(pauseInput());
    await user.type(pauseInput(), "50");
    await user.clear(nameInput());
    expect(nameInput()).toHaveAttribute("aria-invalid", "true");
    await advance(500);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
    expect(backend.state.profiles[0].trackers[0].resumeDelayMs).toBe(1000);
  });

  it("keeps new input while saving and serializes the next draft", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    const release = backend.hold("tracker_update");
    await user.type(nameInput(), " первый");
    await advance(500);
    expect(nameInput()).toBeEnabled();
    expect(screen.getByText("Сохранение…")).toBeInTheDocument();
    await user.type(nameInput(), " второй");
    await advance(300);
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await act(async () => { release(); });
    expect(nameInput()).toHaveValue("Здоровье первый второй");
    await advance(199);
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await advance(1);
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье первый второй");
  });

  it("preserves invalid new input after a successful older save", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    const release = backend.hold("tracker_update");
    await user.type(nameInput(), " новый");
    await advance(500);
    await user.clear(nameInput());
    await advance(500);
    await act(async () => { release(); });
    await advance(1000);
    expect(nameInput()).toHaveValue("");
    expect(nameInput()).toHaveAttribute("aria-invalid", "true");
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
  });

  it("flushes the latest draft after an in-flight save before leaving", async () => {
    const { backend, router } = await openTracker();
    const user = timedUser();
    const release = backend.hold("tracker_update");
    await user.type(nameInput(), " первый");
    await advance(500);
    await user.type(nameInput(), " второй");
    await user.click(exitLink());
    expect(nameInput()).toBeDisabled();
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await act(async () => { release(); });
    await advance(0);
    expect(router.state.location.pathname).toBe("/profiles/profile-1");
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье первый второй");
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
  });

  it("stops failed background retries and retries after input or exit", async () => {
    const { backend, router } = await openTracker();
    const user = timedUser();
    backend.failOn("tracker_update", "Диск недоступен");
    await user.type(nameInput(), " новый");
    await advance(500);
    expect(screen.getByText("Диск недоступен", { selector: "p" })).toBeInTheDocument();
    await advance(5000);
    expect(backend.callsOf("tracker_update")).toHaveLength(1);
    await user.type(nameInput(), " ещё");
    await advance(500);
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    backend.failOn("tracker_update", undefined);
    await user.click(exitLink());
    await advance(0);
    expect(router.state.location.pathname).toBe("/profiles/profile-1");
    expect(backend.callsOf("tracker_update")).toHaveLength(3);
  });

  it("retries the new draft after an older held request fails", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    const release = backend.hold("tracker_update");
    backend.failOn("tracker_update", "Диск недоступен");
    await user.type(nameInput(), " первый");
    await advance(500);
    await user.type(nameInput(), " второй");
    await advance(300);
    await act(async () => { release(); });
    expect(nameInput()).toHaveValue("Здоровье первый второй");
    backend.failOn("tracker_update", undefined);
    await advance(200);
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье первый второй");
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
  });

  it("cancels pending debounce when the editor unmounts", async () => {
    const { backend } = await openTracker();
    const user = timedUser();
    await user.type(nameInput(), " новый");
    cleanup();
    await advance(1000);
    expect(backend.callsOf("tracker_update")).toHaveLength(0);
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
    expect(backend.callsOf("engine_activate")).toEqual([{ profileId: "profile-1", epoch: 1 }]);
    backend.failOn("tracker_update", undefined);
    await act(async () => { await router.navigate("/profiles/other"); });
    await screen.findByRole("heading", { name: "Другой" });
    expect(backend.callsOf("tracker_update")).toHaveLength(2);
    expect(backend.state.profiles[0].trackers[0].name).toBe("Здоровье новый");
  });

  it("preserves draft through immediate pixel, action and run changes", async () => {
    const { user, backend } = await openTracker(trackerFixture({
      pixels: [pixelFixture()], action: { type: "keySequence", sound: { id: null, volumePercent: 50 }, steps: [keyStepFixture()] },
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
    await user.click(screen.getByRole("button", { name: /^Старт отслеживания/ }));
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
