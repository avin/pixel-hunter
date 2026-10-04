import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { KeyRecorder } from "./KeyRecorder";

const SUPPORTED = new Set(["Digit1", "Escape", "KeyQ", "ShiftLeft", "ControlLeft"]);

function renderRecorder() {
  const onRecord = vi.fn();
  const onUnsupported = vi.fn();
  render(
    <>
      <KeyRecorder
        supportedCodes={SUPPORTED}
        onRecord={onRecord}
        onUnsupported={onUnsupported}
      >
        Записать
      </KeyRecorder>
      <button type="button">Другая кнопка</button>
    </>,
  );
  return { user: userEvent.setup(), onRecord, onUnsupported };
}

const idleButton = () => screen.getByRole("button", { name: "Записать" });
const recordingButton = () => screen.getByRole("button", { name: "Нажмите клавишу…" });

describe("KeyRecorder", () => {
  it("waits for a key after the click", async () => {
    const { user } = renderRecorder();

    await user.click(idleButton());

    expect(recordingButton()).toHaveAttribute("aria-pressed", "true");
  });

  it("records a held modifier with a key as a combo", async () => {
    const { user, onRecord } = renderRecorder();
    await user.click(idleButton());

    await user.keyboard("{Control>}1{/Control}");

    expect(onRecord).toHaveBeenCalledExactlyOnceWith({ modifiers: ["ctrl"], key: "Digit1" });
    expect(idleButton()).toHaveAttribute("aria-pressed", "false");
  });

  it("records a modifier pressed and released alone", async () => {
    const { user, onRecord } = renderRecorder();
    await user.click(idleButton());

    await user.keyboard("{Shift>}");
    expect(onRecord).not.toHaveBeenCalled();
    await user.keyboard("{/Shift}");

    expect(onRecord).toHaveBeenCalledExactlyOnceWith({ modifiers: [], key: "ShiftLeft" });
  });

  it("records Esc instead of cancelling", async () => {
    const { user, onRecord } = renderRecorder();
    await user.click(idleButton());

    await user.keyboard("{Escape}");

    expect(onRecord).toHaveBeenCalledExactlyOnceWith({ modifiers: [], key: "Escape" });
  });

  it("cancels on a second click", async () => {
    const { user, onRecord } = renderRecorder();
    await user.click(idleButton());

    await user.click(recordingButton());
    await user.keyboard("q");

    expect(idleButton()).toBeInTheDocument();
    expect(onRecord).not.toHaveBeenCalled();
  });

  it("cancels when focus leaves the button", async () => {
    const { user, onRecord } = renderRecorder();
    await user.click(idleButton());

    await user.click(screen.getByRole("button", { name: "Другая кнопка" }));
    await user.keyboard("q");

    expect(idleButton()).toBeInTheDocument();
    expect(onRecord).not.toHaveBeenCalled();
  });

  it("rejects the Win key", async () => {
    const { user, onRecord, onUnsupported } = renderRecorder();
    await user.click(idleButton());

    await user.keyboard("{Meta}");

    expect(onUnsupported).toHaveBeenCalledOnce();
    expect(onRecord).not.toHaveBeenCalled();
    expect(idleButton()).toBeInTheDocument();
  });

  it("keeps recorded keys away from other handlers", async () => {
    const { user } = renderRecorder();
    const appHandler = vi.fn();
    window.addEventListener("keydown", appHandler);
    await user.click(idleButton());

    await user.keyboard("q");
    window.removeEventListener("keydown", appHandler);

    expect(appHandler).not.toHaveBeenCalled();
  });
});
