import { screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { KeyStep } from "@/store/types";
import {
  createFakeBackend,
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
  backend.state.profiles[0].trackers[0].action.steps;
const stepItems = () =>
  within(screen.getByRole("table", { name: "Шаги последовательности" })).getAllByRole(
    "row",
  ).slice(1);
const delayInput = (step: number, bound: "минимум" | "максимум") =>
  screen.getByRole("spinbutton", { name: `Задержка перед шагом ${step}: ${bound}, мс` });
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
    expect(within(first).queryByRole("spinbutton")).not.toBeInTheDocument();
    expect(second).toHaveTextContent("Ctrl + 1");
    expect(delayInput(2, "минимум")).toHaveValue(300);
    expect(delayInput(2, "максимум")).toHaveValue(600);
    expect(third).toHaveTextContent("F5");
    expect(delayInput(3, "минимум")).toHaveValue(100);
    expect(delayInput(3, "максимум")).toHaveValue(200);
  });

  it("shows an empty sequence as not configured", async () => {
    await openTracker();

    expect(screen.getByText("Нажатия не настроены")).toBeInTheDocument();
    expect(screen.queryByRole("table", { name: "Шаги последовательности" })).not.toBeInTheDocument();
  });

  it("adds the first step without a delay", async () => {
    const { user, backend } = await openTracker();

    await user.click(addButton());
    await user.keyboard("q");

    await waitFor(() => expect(stepItems()).toHaveLength(1));
    expect(stepItems()[0]).toHaveTextContent("Q");
    expect(savedSteps(backend)).toEqual([
      { id: expect.any(String), delayBeforeMs: null, combo: { modifiers: [], key: "KeyQ" } },
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
    expect(within(stepItems()[0]).queryByRole("spinbutton")).not.toBeInTheDocument();
    expect(savedSteps(backend)).toEqual([{ ...w, delayBeforeMs: null }]);
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

  it("rejects a delay with minimum above maximum", async () => {
    const { user, backend } = await openTracker([q, w]);
    const error = "Минимум не может быть больше максимума";

    await user.clear(delayInput(2, "минимум"));
    await user.type(delayInput(2, "минимум"), "600");
    await user.clear(delayInput(2, "максимум"));
    await user.type(delayInput(2, "максимум"), "300");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
    await user.click(screen.getByRole("heading", { name: "Действие" }));

    expect(delayInput(2, "минимум")).toHaveAccessibleDescription(error);
    expect(delayInput(2, "максимум")).toBeInvalid();
    expect(backend.callsOf("action_update")).toEqual([]);

    await user.clear(delayInput(2, "максимум"));
    await user.type(delayInput(2, "максимум"), "700");
    expect(screen.queryByText(error)).not.toBeInTheDocument();
  });
});
