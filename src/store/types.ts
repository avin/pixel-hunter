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

/** Half-open rectangle in physical pixels. */
export interface PhysicalRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export type IconCondition = "present" | "absent";

export interface Icon {
  id: string;
  searchRect: PhysicalRect;
  templateRect: PhysicalRect;
  condition: IconCondition;
  similarityPercent: number;
}

export interface IconInput {
  condition: IconCondition;
  similarityPercent: number;
}

export interface IconStatus {
  profileId: string;
  trackerId: string;
  iconId: string;
  error: string | null;
}

/** Payload of the `pixel-capture://status` event. */
export interface CaptureSource {
  sessionId: string;
  profileId: string;
  trackerId: string;
  mode: "pixel" | "icon";
}

export type CaptureStatus = CaptureSource & (
  | { phase: "starting" }
  | { phase: "waitingForPrintScreen" }
  | { phase: "capturing" }
  | { phase: "picking" }
  | { phase: "selectingSearch" }
  | { phase: "selectingTemplate" }
  | { phase: "saving" }
  | { phase: "done" }
  | { phase: "cancelled" }
  | { phase: "failed"; reason: string });

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
  holdMs: DelayRange;
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
  icons: Icon[];
  /** Shared addition order. Older configs use pixels followed by icons. */
  visualOrder?: string[];
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
  sessionId: string;
  profileId: string;
  pixelId: string;
  mode?: "pixel" | "icon";
}

export interface IconSelection {
  stage: "template" | "done";
  searchRect?: PhysicalRect;
}

/** Latest request sent to the reusable screenshot viewer. */
export interface ScreenshotViewRequest {
  sequence: number;
  profileId: string;
  pixelId: string;
  x: number;
  y: number;
  marker?: { kind: "pixel"; x: number; y: number } | { kind: "icon"; searchRect: PhysicalRect; templateRect: PhysicalRect };
  /** Opening point in the viewer's logical viewport coordinates. */
  origin?: { x: number; y: number } | null;
}
