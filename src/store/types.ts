export interface DelayRange {
  min: number;
  max: number;
}

export interface Tracker {
  id: string;
  name: string;
  preActionDelayMs: DelayRange;
  resumeDelayMs: number;
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
