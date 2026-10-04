import { useEffect, useEffectEvent, useState } from "react";
import { Button } from "@/components/ui/button";
import type { KeyCombo, Modifier } from "@/store/types";

interface Props
  extends Omit<React.ComponentProps<typeof Button>, "onClick" | "onBlur"> {
  supportedCodes: ReadonlySet<string>;
  onRecord: (combo: KeyCombo) => void;
  onUnsupported: () => void;
}

const MODIFIER_CODES: Partial<Record<string, Modifier>> = {
  ControlLeft: "ctrl",
  ControlRight: "ctrl",
  ShiftLeft: "shift",
  ShiftRight: "shift",
  AltLeft: "alt",
  AltRight: "alt",
};

function heldModifiers(event: KeyboardEvent, released?: Modifier) {
  const held: Modifier[] = [];
  if (event.ctrlKey) held.push("ctrl");
  if (event.shiftKey) held.push("shift");
  if (event.altKey) held.push("alt");
  return held.filter((modifier) => modifier !== released);
}

/**
 * While recording, every key event is swallowed in the capture phase so neither
 * WebView accelerators nor app handlers react to it. A modifier pressed and
 * released alone is recorded as a key of its own.
 */
export function KeyRecorder({
  supportedCodes,
  onRecord,
  onUnsupported,
  children,
  ...rest
}: Props) {
  const [isRecording, setIsRecording] = useState(false);

  const finish = useEffectEvent((event: KeyboardEvent, combo: KeyCombo) => {
    setIsRecording(false);
    if (supportedCodes.has(event.code)) {
      onRecord(combo);
    } else {
      onUnsupported();
    }
  });

  useEffect(() => {
    if (!isRecording) return;
    let pendingModifier: string | undefined;

    function handleKeyDown(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      if (event.repeat) return;
      if (MODIFIER_CODES[event.code]) {
        pendingModifier = event.code;
        return;
      }
      finish(event, { modifiers: heldModifiers(event), key: event.code });
    }

    function handleKeyUp(event: KeyboardEvent) {
      event.preventDefault();
      event.stopPropagation();
      if (event.code === pendingModifier) {
        const released = MODIFIER_CODES[event.code];
        finish(event, { modifiers: heldModifiers(event, released), key: event.code });
      }
    }

    window.addEventListener("keydown", handleKeyDown, true);
    window.addEventListener("keyup", handleKeyUp, true);
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
      window.removeEventListener("keyup", handleKeyUp, true);
    };
  }, [isRecording]);

  return (
    <Button
      aria-pressed={isRecording}
      onClick={() => setIsRecording((current) => !current)}
      onBlur={() => setIsRecording(false)}
      {...rest}
    >
      {isRecording ? "Нажмите клавишу…" : children}
    </Button>
  );
}
