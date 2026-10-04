export interface DelayRange {
  min: number;
  max: number;
}

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export type PixelCondition = "equals" | "notEquals";

/** Point in physical pixels of the primary monitor. */
export interface Pixel {
  id: string;
  x: number;
  y: number;
  color: Rgb;
  condition: PixelCondition;
  tolerancePercent: number;
}

export interface PixelInput {
  condition: PixelCondition;
  tolerancePercent: number;
}

/** Payload of the `pixel-capture://status` event. */
export type CaptureStatus =
  | { phase: "waitingForGame"; remainingMs: number }
  | { phase: "countdown"; remainingMs: number }
  | { phase: "picking" }
  | { phase: "done" }
  | { phase: "cancelled" }
  | { phase: "timeout" }
  | { phase: "failed"; reason: string };

export type Modifier = "ctrl" | "shift" | "alt";

/** `key` is a physical key in `KeyboardEvent.code` terms, e.g. `KeyQ` or `ShiftLeft`. */
export interface KeyCombo {
  modifiers: Modifier[];
  key: string;
}

/** Only the first step has no delay. */
export interface KeyStep {
  id: string;
  delayBeforeMs: DelayRange | null;
  combo: KeyCombo;
}

export type Action = { type: "keySequence"; steps: KeyStep[] };

export type ActionType = Action["type"];

export interface KeyCodeInfo {
  code: string;
  label: string;
}

export interface Tracker {
  id: string;
  name: string;
  preActionDelayMs: DelayRange;
  resumeDelayMs: number;
  pixels: Pixel[];
  action: Action;
  /** Saved choice of the user; only a tracker with pixels and action steps can be running. */
  isRunning: boolean;
}

/** Payload of the `engine://status` event, sent when the game window gains or loses focus. */
export interface EngineStatus {
  profileId: string;
  gameActive: boolean;
}

export interface Profile {
  id: string;
  name: string;
  processName: string;
  trackers: Tracker[];
}

export interface WindowedProcess {
  exeName: string;
  windowTitles: string[];
}

export interface ProfileInput {
  name: string;
  processName: string;
}

export interface TrackerInput {
  name: string;
  preActionDelayMs: DelayRange;
  resumeDelayMs: number;
}

/** Latest request sent to the reusable screenshot viewer. */
export interface ScreenshotViewRequest {
  sequence: number;
  profileId: string;
  pixelId: string;
  x: number;
  y: number;
}
