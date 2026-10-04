import { mockIPC } from "@tauri-apps/api/mocks";
import type {
  Action,
  KeyCodeInfo,
  KeyStep,
  Pixel,
  PixelInput,
  Profile,
  ProfileInput,
  Tracker,
  TrackerInput,
  WindowedProcess,
} from "@/store/types";

interface Options {
  profiles?: Profile[];
  processes?: WindowedProcess[];
}

type Args = Record<string, unknown>;

export const SCREENSHOTS_DIR = "C:\\data\\screenshots";

/** A subset of the Rust key table. */
export const KEY_CODES: KeyCodeInfo[] = [
  { code: "Escape", label: "Esc" },
  { code: "Digit1", label: "1" },
  { code: "Enter", label: "Enter" },
  { code: "KeyQ", label: "Q" },
  { code: "KeyW", label: "W" },
  { code: "KeyE", label: "E" },
  { code: "KeyA", label: "A" },
  { code: "ControlLeft", label: "Ctrl" },
  { code: "ShiftLeft", label: "Shift" },
  { code: "AltLeft", label: "Alt" },
  { code: "F5", label: "F5" },
];

/** In-memory stand-in for the Rust commands, installed through `mockIPC`. */
export function createFakeBackend({ profiles = [], processes = [] }: Options = {}) {
  const state = {
    profiles: structuredClone(profiles),
    processes: structuredClone(processes),
  };
  const failures = new Map<string, string>();
  const calls: { command: string; args: Args }[] = [];
  let lastId = 0;
  const nextId = () => `generated-${++lastId}`;

  const findProfile = (id: unknown) => {
    const profile = state.profiles.find((candidate) => candidate.id === id);
    if (!profile) throw new Error(`Unknown profile ${String(id)}`);
    return profile;
  };

  const findTracker = (profileId: unknown, trackerId: unknown) => {
    const tracker = findProfile(profileId).trackers.find(
      (candidate) => candidate.id === trackerId,
    );
    if (!tracker) throw new Error(`Unknown tracker ${String(trackerId)}`);
    return tracker;
  };

  const missingPart = (tracker: Tracker) => {
    if (tracker.pixels.length === 0) return "Добавьте пиксель, чтобы запустить отслеживание";
    if (tracker.action.steps.length === 0) {
      return "Добавьте шаг действия, чтобы запустить отслеживание";
    }
    return undefined;
  };

  const stopIfIncomplete = (tracker: Tracker) => {
    if (missingPart(tracker)) tracker.isRunning = false;
  };

  const findPixel = (args: Args) => {
    const pixel = findTracker(args.profileId, args.trackerId).pixels.find(
      (candidate) => candidate.id === args.pixelId,
    );
    if (!pixel) throw new Error(`Unknown pixel ${String(args.pixelId)}`);
    return pixel;
  };

  function handle(command: string, args: Args): unknown {
    switch (command) {
      case "profiles_list":
        return structuredClone(state.profiles);
      case "profile_create": {
        const profile: Profile = {
          id: nextId(),
          ...(args.input as ProfileInput),
          trackers: [],
        };
        state.profiles.push(profile);
        return structuredClone(profile);
      }
      case "profile_update": {
        const profile = findProfile(args.id);
        Object.assign(profile, args.input as ProfileInput);
        return structuredClone(profile);
      }
      case "profile_delete":
        findProfile(args.id);
        state.profiles = state.profiles.filter(({ id }) => id !== args.id);
        return null;
      case "tracker_create": {
        const tracker: Tracker = {
          id: nextId(),
          name: args.name as string,
          preActionDelayMs: { min: 100, max: 300 },
          resumeDelayMs: 1000,
          pixels: [],
          action: { type: "keySequence", steps: [] },
          isRunning: false,
        };
        findProfile(args.profileId).trackers.push(tracker);
        return structuredClone(tracker);
      }
      case "tracker_update": {
        const tracker = findTracker(args.profileId, args.trackerId);
        Object.assign(tracker, args.input as TrackerInput);
        return structuredClone(tracker);
      }
      case "tracker_delete": {
        const profile = findProfile(args.profileId);
        findTracker(args.profileId, args.trackerId);
        profile.trackers = profile.trackers.filter(
          ({ id }) => id !== args.trackerId,
        );
        return null;
      }
      case "action_update": {
        const tracker = findTracker(args.profileId, args.trackerId);
        const action = structuredClone(args.action as Action);
        if (action.steps[0]) action.steps[0].delayBeforeMs = null;
        tracker.action = action;
        stopIfIncomplete(tracker);
        return structuredClone(action);
      }
      case "tracker_set_running": {
        const tracker = findTracker(args.profileId, args.trackerId);
        const missing = missingPart(tracker);
        if (args.isRunning && missing) throw new Error(missing);
        tracker.isRunning = args.isRunning as boolean;
        return structuredClone(tracker);
      }
      case "engine_activate":
        findProfile(args.profileId);
        return null;
      case "engine_deactivate":
        return null;
      case "key_codes_list":
        return structuredClone(KEY_CODES);
      case "pixel_update": {
        const pixel = findPixel(args);
        Object.assign(pixel, args.input as PixelInput);
        return structuredClone(pixel);
      }
      case "pixel_delete": {
        const tracker = findTracker(args.profileId, args.trackerId);
        findPixel(args);
        tracker.pixels = tracker.pixels.filter(({ id }) => id !== args.pixelId);
        stopIfIncomplete(tracker);
        return null;
      }
      case "pixel_capture_start":
        findTracker(args.profileId, args.trackerId);
        return null;
      case "pixel_capture_cancel":
      case "pixel_capture_pick":
      case "plugin:window|close":
        return null;
      case "pixel_view_open":
        findPixel(args);
        return null;
      case "screenshots_dir":
        return SCREENSHOTS_DIR;
      case "windowed_processes_list":
        return structuredClone(state.processes);
      default:
        throw new Error(`Unexpected command ${command}`);
    }
  }

  return {
    state,
    /** Arguments of every call of `command`, oldest first. */
    callsOf(command: string) {
      return calls
        .filter((call) => call.command === command)
        .map((call) => call.args);
    },
    /**
     * Makes every following call of `command` fail the way a Rust error does;
     * `undefined` lets it succeed again.
     */
    failOn(command: string, message: string | undefined) {
      if (message === undefined) {
        failures.delete(command);
      } else {
        failures.set(command, message);
      }
    },
    /** Events sent with `emit` from `@tauri-apps/api/event` reach the app's listeners. */
    install() {
      mockIPC(
        (command, payload) => {
          const args = (payload ?? {}) as Args;
          calls.push({ command, args });
          const failure = failures.get(command);
          if (failure !== undefined) {
            return Promise.reject(failure);
          }
          return handle(command, args);
        },
        { shouldMockEvents: true },
      );
    },
  };
}

export type FakeBackend = ReturnType<typeof createFakeBackend>;

export function profileFixture(overrides: Partial<Profile> = {}): Profile {
  return {
    id: "profile-1",
    name: "Diablo",
    processName: "game.exe",
    trackers: [],
    ...overrides,
  };
}

export function trackerFixture(overrides: Partial<Tracker> = {}): Tracker {
  return {
    id: "tracker-1",
    name: "Здоровье",
    preActionDelayMs: { min: 100, max: 300 },
    resumeDelayMs: 1000,
    pixels: [],
    action: { type: "keySequence", steps: [] },
    isRunning: false,
    ...overrides,
  };
}

export function keyStepFixture(overrides: Partial<KeyStep> = {}): KeyStep {
  return {
    id: "step-1",
    delayBeforeMs: null,
    combo: { modifiers: [], key: "KeyQ" },
    ...overrides,
  };
}

export function pixelFixture(overrides: Partial<Pixel> = {}): Pixel {
  return {
    id: "pixel-1",
    x: 812,
    y: 1040,
    color: { r: 200, g: 30, b: 30 },
    condition: "equals",
    tolerancePercent: 10,
    ...overrides,
  };
}
