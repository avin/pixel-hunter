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

/** Stable ID from the backend sound catalog. */
export type SoundId = string;

export interface SoundSettings {
  id: SoundId | null;
  volumePercent: number;
}

export interface ActionSoundInfo {
  id: SoundId;
  name: string;
}

export type Action =
  | { type: "keySequence"; steps: KeyStep[]; sound: SoundSettings }
  | { type: "networkDisconnect"; sound: SoundSettings };

export type ActionType = Action["type"];

/** A key sequence with no steps is not configured. Network disconnect has no fields of its own. */
export function actionIsEmpty(action: Action): boolean {
  return action.type === "keySequence" && action.steps.length === 0;
}

export interface KeyCodeInfo {
  code: string;
  label: string;
}

export type Trigger = "pixels" | "hotkey";

export interface Tracker {
  id: string;
  name: string;
  preActionDelayMs: DelayRange;
  resumeDelayMs: number;
  pixels: Pixel[];
  /** Missing data from an older config means pixels. */
  trigger: Trigger;
  hotkey: KeyCombo | null;
  action: Action;
  /** Saved choice of the user; only a complete tracker can be running. */
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

/** Screen open when the app last quit. Startup opens the named profile, never a tracker editor. Missing data means the profile list. */
export type LastScreen =
  | { kind: "profiles" }
  | { kind: "profile"; profileId: string }
  | { kind: "editor"; profileId: string; trackerId: string };

export interface AppSettings {
  pauseHotkey: KeyCombo | null;
  lastScreen: LastScreen;
}

/** Payload of the `work://pause` event and the pause toggle command. */
export interface WorkPause {
  paused: boolean;
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

/** Frame delivered to the prepared picker page. */
export interface PickerFrame {
  profileId: string;
  pixelId: string;
}

/** Latest request sent to the reusable screenshot viewer. */
export interface ScreenshotViewRequest {
  sequence: number;
  profileId: string;
  pixelId: string;
  x: number;
  y: number;
  /** Opening point in the viewer's logical viewport coordinates. */
  origin?: { x: number; y: number } | null;
}
