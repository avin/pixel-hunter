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

export interface Tracker {
  id: string;
  name: string;
  preActionDelayMs: DelayRange;
  resumeDelayMs: number;
  pixels: Pixel[];
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
