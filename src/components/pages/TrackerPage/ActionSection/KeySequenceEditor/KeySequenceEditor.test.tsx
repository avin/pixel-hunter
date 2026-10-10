import { cleanup, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { KeyStep } from "@/store/types";
import {
  createFakeBackend,
  keySequenceSteps,
  keyStepFixture,
  profileFixture,
  trackerFixture,
} from "@/test/fakeBackend";
import { renderApp } from "@/test/renderApp";

async function openTracker(steps: KeyStep[] = []) {
  const rendered = renderApp(
    createFakeBackend({
      profiles: [
        profileFixture({
          trackers: [trackerFixture({ action: { type: "keySequence", sound: { id: null, volumePercent: 50 }, steps } })],
        }),
      ],
    }),
  );
  await rendered.user.click(await screen.findByRole("link", { name: "Diablo" }));
  await rendered.user.click(screen.getByRole("link", { name: "Здоровье" }));
  await screen.findByRole("button", { name: "Добавить нажатие" });
  return rendered;
}

const savedSteps = (backend: ReturnType<typeof createFakeBackend>) =>
  keySequenceSteps(backend.state.profiles[0].trackers[0].action);
const stepItems = () =>
  within(screen.getByRole("table", { name: "Шаги последовательности" })).getAllByRole(
    "row",
  ).slice(1);
const delayInput = (step: number, bound: "минимум" | "максимум") =>
  screen.getByRole("spinbutton", { name: `Задержка перед шагом ${step}: ${bound}, мс` });
const holdInput = (step: number, bound: "минимум" | "максимум") =>
  screen.getByRole("spinbutton", { name: `Удержание шага ${step}: ${bound}, мс` });
const addButton = () => screen.getByRole("button", { name: "Добавить нажатие" });

const q = keyStepFixture();
const ctrl1 = keyStepFixture({
  id: "step-2",
  delayBeforeMs: { min: 300, max: 600 },
  combo: { modifiers: ["ctrl"], key: "Digit1" },
});
const f5 = keyStepFixture({
  id: "step-3",
  delayBeforeMs: { min: 100, max: 200 },
  combo: { modifiers: [], key: "F5" },
});
const w = keyStepFixture({
  id: "step-2",
  delayBeforeMs: { min: 300, max: 600 },
  holdMs: { min: 1000, max: 1500 },
  combo: { modifiers: [], key: "KeyW" },
});

describe("KeySequenceEditor", () => {
  it("lists steps in order with the delay before every step but the first", async () => {
    await openTracker([q, ctrl1, f5]);

    const [first, second, third] = stepItems();
    const table = screen.getByRole("table", { name: "Шаги последовательности" });
    expect(within(table).getByRole("columnheader", { name: "Сочетание" })).toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Задержка, мс" })).toBeInTheDocument();
    expect(first).toHaveTextContent("Q");
    expect(within(first).getByLabelText("Без задержки")).toHaveTextContent("—");
    expect(within(first).queryByRole("spinbutton", { name: /Задержка/ })).not.toBeInTheDocument();
    expect(within(table).getByRole("columnheader", { name: "Удержание, мс" })).toBeInTheDocument();
    for (const step of [1, 2, 3]) {
      expect(holdInput(step, "минимум")).toHaveValue(40);
      expect(holdInput(step, "максимум")).toHaveValue(90);
    }
    expect(second).toHaveTextContent("Ctrl + 1");
    expect(delayInput(2, "минимум")).toHaveValue(300);
    expect(delayInput(2, "максимум")).toHaveValue(600);
    expect(third).toHaveTextContent("F5");
    expect(delayInput(3, "минимум")).toHaveValue(100);
    expect(delayInput(3, "максимум")).toHaveValue(200);
  });

  it("explains the step delay only after activating its info button", async () => {
    const { user } = await openTracker([q, w]);
    const description =
      "Перед каждым нажатием, кроме первого, выдерживается случайная пауза из этого диапазона.";
    const info = screen.getByRole("button", { name: "Информация: Задержка перед нажатием" });

    expect(screen.queryByText(description)).not.toBeInTheDocument();
    await user.hover(info);
    expect(screen.queryByText(description)).not.toBeInTheDocument();
    await user.click(info);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(description);
  });

  it("shows an empty sequence as not configured", async () => {
    await openTracker();

    expect(screen.getByText("Нажатия не настроены")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Шаги последовательности" })).not.toBeInTheDocument();
  });

  it("explains how the main key and modifiers are held", async () => {
    const { user } = await openTracker([q]);
    await user.click(screen.getByRole("button", { name: "Информация: Удержание клавиши" }));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(
      "Основная клавиша удерживается до отпускания в течение случайного времени из этого диапазона. Модификаторы остаются нажатыми вместе с ней.",
    );
  });

  it("adds the first step without a delay", async () => {
    const { user, backend } = await openTracker();

    await user.click(addButton());
    await user.keyboard("q");

    await waitFor(() => expect(stepItems()).toHaveLength(1));
    expect(stepItems()[0]).toHaveTextContent("Q");
    expect(savedSteps(backend)).toEqual([
      { id: expect.any(String), delayBeforeMs: null, holdMs: { min: 40, max: 90 }, combo: { modifiers: [], key: "KeyQ" } },
    ]);
  });

  it("adds a later step with a 100–200 ms delay", async () => {
    const { user, backend } = await openTracker([q]);

    await user.click(addButton());
    await user.keyboard("w");

    await waitFor(() => expect(stepItems()).toHaveLength(2));
    expect(stepItems()[1]).toHaveTextContent("W");
    expect(delayInput(2, "минимум")).toHaveValue(100);
    expect(delayInput(2, "максимум")).toHaveValue(200);
    expect(savedSteps(backend)[1]).toMatchObject({
      delayBeforeMs: { min: 100, max: 200 },
      holdMs: { min: 40, max: 90 },
      combo: { modifiers: [], key: "KeyW" },
    });
  });

  it("adds nothing when the recording is cancelled", async () => {
    const { user, backend } = await openTracker([q]);

    await user.click(addButton());
    await user.click(screen.getByRole("button", { name: "Нажмите клавишу…" }));

    expect(addButton()).toBeInTheDocument();
    expect(stepItems()).toHaveLength(1);
    expect(backend.callsOf("action_update")).toEqual([]);
  });

  it("tells that a key is not supported", async () => {
    const { user, backend } = await openTracker();

    await user.click(addButton());
    await user.keyboard("{Meta}");

    expect(screen.getByRole("alert")).toHaveTextContent("Эта клавиша не поддерживается");
    expect(backend.callsOf("action_update")).toEqual([]);

    await user.click(addButton());
    await user.keyboard("q");
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
  });

  it("drops the delay of the step that becomes first", async () => {
    const { user, backend } = await openTracker([q, w]);

    await user.click(screen.getByRole("button", { name: "Удалить шаг 1" }));

    await waitFor(() => expect(stepItems()).toHaveLength(1));
    expect(stepItems()[0]).toHaveTextContent("W");
    expect(within(stepItems()[0]).queryByRole("spinbutton", { name: /Задержка/ })).not.toBeInTheDocument();
    expect(holdInput(1, "минимум")).toHaveValue(1000);
    expect(holdInput(1, "максимум")).toHaveValue(1500);
    expect(savedSteps(backend)).toEqual([{ ...w, delayBeforeMs: null }]);
    const persistedSteps = structuredClone(savedSteps(backend));
    cleanup();
    await openTracker(persistedSteps);
    expect(holdInput(1, "минимум")).toHaveValue(1000);
    expect(holdInput(1, "максимум")).toHaveValue(1500);
  });

  it("re-records a step keeping its delay", async () => {
    const { user, backend } = await openTracker([q, w]);

    await user.click(screen.getByRole("button", { name: "Перезаписать шаг 2" }));
    await user.keyboard("e");

    await waitFor(() => expect(stepItems()[1]).toHaveTextContent("E"));
    expect(savedSteps(backend)[1]).toEqual({
      ...w,
      combo: { modifiers: [], key: "KeyE" },
    });
    const persistedSteps = structuredClone(savedSteps(backend));
    cleanup();
    await openTracker(persistedSteps);
    expect(holdInput(2, "минимум")).toHaveValue(1000);
    expect(holdInput(2, "максимум")).toHaveValue(1500);
  });

  it("saves a changed delay when focus leaves the range", async () => {
    const { user, backend } = await openTracker([q, w]);

    await user.clear(delayInput(2, "минимум"));
    await user.type(delayInput(2, "минимум"), "150");
    await user.tab();
    expect(backend.callsOf("action_update")).toEqual([]);
    await user.tab();

    await waitFor(() =>
      expect(savedSteps(backend)[1].delayBeforeMs).toEqual({ min: 150, max: 600 }),
    );
  });

  it("saves a changed delay on Enter", async () => {
    const { user, backend } = await openTracker([q, w]);

    await user.clear(delayInput(2, "максимум"));
    await user.type(delayInput(2, "максимум"), "900{Enter}");

    await waitFor(() =>
      expect(savedSteps(backend)[1].delayBeforeMs).toEqual({ min: 300, max: 900 }),
    );
  });

  it("saves holds independently on Enter and leaving the range, then reloads them", async () => {
    const { user, backend } = await openTracker([q, w]);
    await user.clear(holdInput(1, "минимум"));
    await user.type(holdInput(1, "минимум"), "2000");
    await user.clear(holdInput(1, "максимум"));
    await user.type(holdInput(1, "максимум"), "3000{Enter}");
    await waitFor(() => expect(savedSteps(backend)[0].holdMs).toEqual({ min: 2000, max: 3000 }));
    expect(savedSteps(backend)[1]).toEqual(w);

    await user.clear(holdInput(2, "минимум"));
    await user.type(holdInput(2, "минимум"), "1200");
    await user.tab();
    expect(savedSteps(backend)[1].holdMs).toEqual(w.holdMs);
    await user.clear(holdInput(2, "максимум"));
    await user.type(holdInput(2, "максимум"), "1200");
    await user.tab();
    await waitFor(() => expect(savedSteps(backend)[1].holdMs).toEqual({ min: 1200, max: 1200 }));
    expect(savedSteps(backend)[1].delayBeforeMs).toEqual(w.delayBeforeMs);
    expect(savedSteps(backend)[0].delayBeforeMs).toBeNull();

    const persistedSteps = structuredClone(savedSteps(backend));
    cleanup();
    await openTracker(persistedSteps);
    expect(holdInput(1, "минимум")).toHaveValue(2000);
    expect(holdInput(1, "максимум")).toHaveValue(3000);
    expect(holdInput(2, "минимум")).toHaveValue(1200);
    expect(holdInput(2, "максимум")).toHaveValue(1200);
  });

  it.each([0, 4_294_967_295])("accepts a fixed hold of %i ms", async (duration) => {
    const { user, backend } = await openTracker([q]);
    await user.clear(holdInput(1, "минимум"));
    await user.type(holdInput(1, "минимум"), String(duration));
    await user.clear(holdInput(1, "максимум"));
    await user.type(holdInput(1, "максимум"), `${duration}{Enter}`);
    await waitFor(() => expect(savedSteps(backend)[0].holdMs).toEqual({ min: duration, max: duration }));
  });

  it.each([
    ["", "90", "Введите целое неотрицательное число"],
    ["-1", "90", "Введите целое неотрицательное число"],
    ["1.5", "90", "Введите целое неотрицательное число"],
    ["40", "4294967296", "Слишком большое значение"],
    ["3000", "2000", "Минимум не может быть больше максимума"],
  ])("rejects hold %s–%s and reveals errors until corrected", async (min, max, error) => {
    const { user, backend } = await openTracker([q]);
    await user.clear(holdInput(1, "минимум"));
    if (min) await user.type(holdInput(1, "минимум"), min);
    await user.clear(holdInput(1, "максимум"));
    await user.type(holdInput(1, "максимум"), max);
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.keyboard("{Enter}");
    expect(holdInput(1, "минимум")).toHaveAccessibleDescription(error);
    expect(holdInput(1, "максимум")).toBeInvalid();
    expect(savedSteps(backend)[0]).toEqual(q);
    expect(backend.callsOf("action_update")).toEqual([]);

    await user.clear(holdInput(1, "минимум"));
    await user.type(holdInput(1, "минимум"), "0");
    await user.clear(holdInput(1, "максимум"));
    await user.type(holdInput(1, "максимум"), "90");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.clear(holdInput(1, "максимум"));
    expect(holdInput(1, "максимум")).toBeInvalid();
    await user.type(holdInput(1, "максимум"), "90{Enter}");
    await waitFor(() => expect(savedSteps(backend)[0].holdMs).toEqual({ min: 0, max: 90 }));
  });

  it("rejects a delay with minimum above maximum", async () => {
    const { user, backend } = await openTracker([q, w]);
    const error = "Минимум не может быть больше максимума";

    await user.clear(delayInput(2, "минимум"));
    await user.type(delayInput(2, "минимум"), "600");
    await user.clear(delayInput(2, "максимум"));
    await user.type(delayInput(2, "максимум"), "300");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.click(screen.getByRole("combobox", { name: "Действие" }));

    expect(delayInput(2, "минимум")).toHaveAccessibleDescription(error);
    expect(delayInput(2, "максимум")).toBeInvalid();
    expect(backend.callsOf("action_update")).toEqual([]);

    await user.clear(delayInput(2, "максимум"));
    await user.type(delayInput(2, "максимум"), "700");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
  });

  it("captures keys while an action step is recorded", async () => {
    const { user, backend } = await openTracker([q]);
    const rewrite = () => screen.getByRole("button", { name: "Перезаписать шаг 1" });
    await user.click(rewrite());
    await waitFor(() => expect(rewrite()).toHaveAttribute("aria-pressed", "true"));
    expect(backend.callsOf("key_capture_begin")).toHaveLength(1);
    await user.keyboard("{Escape}");
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(1));
    expect(backend.callsOf("work_pause_toggle")).toEqual([]);
    expect(backend.state.profiles[0].trackers[0].isRunning).toBe(false);

    await user.click(addButton());
    await user.click(await screen.findByRole("button", { name: "Нажмите клавишу…" }));
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(2));

    await user.click(addButton());
    await screen.findByRole("button", { name: "Нажмите клавишу…" });
    cleanup();
    await waitFor(() => expect(backend.callsOf("key_capture_end")).toHaveLength(3));
  });
});
