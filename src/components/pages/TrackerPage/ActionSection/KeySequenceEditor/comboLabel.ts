import type { KeyCombo, Modifier } from "@/store/types";

const MODIFIER_LABELS: Record<Modifier, string> = {
  ctrl: "Ctrl",
  shift: "Shift",
  alt: "Alt",
};

/** `Ctrl + Shift + 1`; a key missing from `keyLabels` is shown by its code. */
export function comboLabel(combo: KeyCombo, keyLabels: ReadonlyMap<string, string>) {
  return [
    ...combo.modifiers.map((modifier) => MODIFIER_LABELS[modifier]),
    keyLabels.get(combo.key) ?? combo.key,
  ].join(" + ");
}
