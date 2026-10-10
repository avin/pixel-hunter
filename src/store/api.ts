import { createApi } from "@reduxjs/toolkit/query/react";
import { tauriBaseQuery } from "./tauriBaseQuery";
import type {
  Action,
  Icon,
  IconInput,
  IconSelection,
  PhysicalRect,
  ActionSoundInfo,
  AppSettings,
  KeyCodeInfo,
  KeyCombo,
  LastScreen,
  Pixel,
  PixelInput,
  Profile,
  ProfileInput,
  PickerFrame,
  ScreenshotViewRequest,
  Tracker,
  TrackerInput,
  Trigger,
  WindowedProcess,
  WorkPause,
} from "./types";

interface PixelArgs {
  profileId: string;
  trackerId: string;
  pixelId: string;
}

const PROFILE_LIST = { type: "Profile", id: "LIST" } as const;

export const tauriApi = createApi({
  reducerPath: "tauriApi",
  baseQuery: tauriBaseQuery,
  tagTypes: ["Profile", "Settings"],
  endpoints: (builder) => ({
    listProfiles: builder.query<Profile[], void>({
      query: () => ({ command: "profiles_list" }),
      providesTags: (profiles = []) => [
        PROFILE_LIST,
        ...profiles.map(({ id }) => ({ type: "Profile" as const, id })),
      ],
    }),
    createProfile: builder.mutation<Profile, ProfileInput>({
      query: (input) => ({ command: "profile_create", args: { input } }),
      invalidatesTags: [PROFILE_LIST],
    }),
    updateProfile: builder.mutation<
      Profile,
      { id: string; input: ProfileInput }
    >({
      query: ({ id, input }) => ({
        command: "profile_update",
        args: { id, input },
      }),
      invalidatesTags: (_result, _error, { id }) => [
        PROFILE_LIST,
        { type: "Profile", id },
      ],
    }),
    deleteProfile: builder.mutation<void, { id: string }>({
      query: ({ id }) => ({ command: "profile_delete", args: { id } }),
      invalidatesTags: (_result, _error, { id }) => [
        PROFILE_LIST,
        { type: "Profile", id },
      ],
    }),
    createTracker: builder.mutation<Tracker, { profileId: string; name: string }>(
      {
        query: ({ profileId, name }) => ({
          command: "tracker_create",
          args: { profileId, name },
        }),
        invalidatesTags: (_result, _error, { profileId }) => [
          { type: "Profile", id: profileId },
        ],
        async onQueryStarted({ profileId }, { dispatch, queryFulfilled }) {
          try {
            const { data: tracker } = await queryFulfilled;
            dispatch(
              tauriApi.util.updateQueryData("listProfiles", undefined, (profiles) => {
                const profile = profiles.find(({ id }) => id === profileId);
                if (profile && !profile.trackers.some(({ id }) => id === tracker.id)) {
                  profile.trackers.push(tracker);
                }
              }),
            );
          } catch {
            // Failed creation leaves the profile unchanged.
          }
        },
      },
    ),
    updateTracker: builder.mutation<
      Tracker,
      { profileId: string; trackerId: string; input: TrackerInput }
    >({
      query: ({ profileId, trackerId, input }) => ({
        command: "tracker_update",
        args: { profileId, trackerId, input },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    deleteTracker: builder.mutation<
      void,
      { profileId: string; trackerId: string }
    >({
      query: ({ profileId, trackerId }) => ({
        command: "tracker_delete",
        args: { profileId, trackerId },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    setTrackerTrigger: builder.mutation<
      Tracker,
      { profileId: string; trackerId: string; trigger: Trigger }
    >({
      query: ({ profileId, trackerId, trigger }) => ({
        command: "tracker_set_trigger",
        args: { profileId, trackerId, trigger },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    setTrackerHotkey: builder.mutation<
      Tracker,
      { profileId: string; trackerId: string; hotkey: KeyCombo | null }
    >({
      query: ({ profileId, trackerId, hotkey }) => ({
        command: "tracker_set_hotkey",
        args: { profileId, trackerId, hotkey },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    beginKeyCapture: builder.mutation<void, void>({
      query: () => ({ command: "key_capture_begin" }),
    }),
    endKeyCapture: builder.mutation<void, void>({
      query: () => ({ command: "key_capture_end" }),
    }),
    trackerSetRunning: builder.mutation<
      Tracker,
      { profileId: string; trackerId: string; isRunning: boolean }
    >({
      query: ({ profileId, trackerId, isRunning }) => ({
        command: "tracker_set_running",
        args: { profileId, trackerId, isRunning },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    engineActivate: builder.mutation<void, { profileId: string; epoch: number }>({
      query: ({ profileId, epoch }) => ({
        command: "engine_activate",
        args: { profileId, epoch },
      }),
    }),
    engineDeactivate: builder.mutation<void, { epoch: number }>({
      query: ({ epoch }) => ({
        command: "engine_deactivate",
        args: { epoch },
      }),
    }),
    getSettings: builder.query<AppSettings, void>({
      query: () => ({ command: "settings_get" }),
      providesTags: ["Settings"],
    }),
    setLastScreen: builder.mutation<LastScreen, LastScreen>({
      query: (lastScreen) => ({
        command: "settings_set_last_screen",
        args: { lastScreen },
      }),
      async onQueryStarted(lastScreen, { dispatch, queryFulfilled }) {
        try {
          await queryFulfilled;
          dispatch(
            tauriApi.util.updateQueryData("getSettings", undefined, (draft) => {
              draft.lastScreen = lastScreen;
            }),
          );
        } catch {
          // The footer keeps showing the last settings that were read.
        }
      },
    }),
    setPauseHotkey: builder.mutation<KeyCombo, KeyCombo>({
      query: (pauseHotkey) => ({
        command: "settings_set_pause_hotkey",
        args: { pauseHotkey },
      }),
      invalidatesTags: ["Settings"],
      async onQueryStarted(_pauseHotkey, { dispatch, queryFulfilled }) {
        try {
          const { data } = await queryFulfilled;
          dispatch(
            tauriApi.util.updateQueryData("getSettings", undefined, (draft) => {
              draft.pauseHotkey = data;
            }),
          );
        } catch {
          // A failed assignment leaves the saved combo in place.
        }
      },
    }),
    clearPauseHotkey: builder.mutation<void, void>({
      query: () => ({ command: "settings_clear_pause_hotkey" }),
      invalidatesTags: ["Settings"],
      async onQueryStarted(_arg, { dispatch, queryFulfilled }) {
        try {
          await queryFulfilled;
          dispatch(
            tauriApi.util.updateQueryData("getSettings", undefined, (draft) => {
              draft.pauseHotkey = null;
            }),
          );
        } catch {
          // A failed clear leaves the saved combo in place.
        }
      },
    }),
    beginPauseHotkey: builder.mutation<void, void>({
      query: () => ({ command: "pause_hotkey_begin" }),
    }),
    cancelPauseHotkey: builder.mutation<void, void>({
      query: () => ({ command: "pause_hotkey_cancel" }),
    }),
    workPauseToggle: builder.mutation<WorkPause, void>({
      query: () => ({ command: "work_pause_toggle" }),
    }),
    actionUpdate: builder.mutation<
      Action,
      { profileId: string; trackerId: string; action: Action }
    >({
      query: ({ profileId, trackerId, action }) => ({
        command: "action_update",
        args: { profileId, trackerId, action },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    keyCodesList: builder.query<KeyCodeInfo[], void>({
      query: () => ({ command: "key_codes_list" }),
      keepUnusedDataFor: Infinity,
    }),
    actionSoundsList: builder.query<ActionSoundInfo[], void>({
      query: () => ({ command: "action_sounds_list" }),
      keepUnusedDataFor: Infinity,
    }),
    actionSoundPreview: builder.mutation<void, { id: string; volumePercent: number }>({
      query: (args) => ({ command: "action_sound_preview", args }),
    }),
    actionSoundStopPreview: builder.mutation<void, void>({
      query: () => ({ command: "action_sound_stop_preview" }),
    }),
    pixelUpdate: builder.mutation<Pixel, PixelArgs & { input: PixelInput }>({
      query: ({ profileId, trackerId, pixelId, input }) => ({
        command: "pixel_update",
        args: { profileId, trackerId, pixelId, input },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    iconUpdate: builder.mutation<Icon, { profileId: string; trackerId: string; iconId: string; input: IconInput }>({
      query: (args) => ({ command: "icon_update", args }),
      invalidatesTags: (_result, _error, { profileId }) => [{ type: "Profile", id: profileId }],
    }),
    iconDelete: builder.mutation<void, { profileId: string; trackerId: string; iconId: string }>({
      query: (args) => ({ command: "icon_delete", args }),
      invalidatesTags: (_result, _error, { profileId }) => [{ type: "Profile", id: profileId }],
    }),
    iconCaptureStart: builder.mutation<void, { profileId: string; trackerId: string; sessionId: string }>({
      query: (args) => ({ command: "icon_capture_start", args }),
    }),
    iconCaptureSelect: builder.mutation<IconSelection, { pixelId: string; rect: PhysicalRect; stage: "search" | "template" }>({
      query: (args) => ({ command: "icon_capture_select", args }),
    }),
    iconViewOpen: builder.mutation<void, { profileId: string; trackerId: string; iconId: string; origin: { x: number; y: number } }>({
      query: (args) => ({ command: "icon_view_open", args }),
    }),
    pixelDelete: builder.mutation<void, PixelArgs>({
      query: ({ profileId, trackerId, pixelId }) => ({
        command: "pixel_delete",
        args: { profileId, trackerId, pixelId },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
    }),
    pixelCaptureStart: builder.mutation<
      void,
      { profileId: string; trackerId: string; sessionId: string }
    >({
      query: (args) => ({
        command: "pixel_capture_start",
        args,
      }),
    }),
    pixelCaptureCancel: builder.mutation<void, { sessionId: string }>({
      query: (args) => ({ command: "pixel_capture_cancel", args }),
    }),
    pixelCapturePick: builder.mutation<void, { sessionId: string; x: number; y: number }>({
      query: (args) => ({ command: "pixel_capture_pick", args }),
    }),
    pixelViewOpen: builder.mutation<void, PixelArgs & { origin: { x: number; y: number } }>({
      query: ({ profileId, trackerId, pixelId, origin }) => ({
        command: "pixel_view_open",
        args: { profileId, trackerId, pixelId, origin },
      }),
    }),
    screenshotViewRequest: builder.query<ScreenshotViewRequest | null, void>({
      query: () => ({ command: "screenshot_view_request" }),
      keepUnusedDataFor: 0,
    }),
    screenshotViewReady: builder.mutation<void, { sequence: number }>({
      query: (args) => ({ command: "screenshot_view_ready", args }),
    }),
    screenshotViewClose: builder.mutation<void, { sequence: number }>({
      query: (args) => ({ command: "screenshot_view_close", args }),
    }),
    screenshotViewFailed: builder.mutation<void, { sequence: number }>({
      query: (args) => ({ command: "screenshot_view_failed", args }),
    }),
    screenshotsDir: builder.query<string, void>({
      query: () => ({ command: "screenshots_dir" }),
      keepUnusedDataFor: Infinity,
    }),
    pickerReady: builder.mutation<void, void>({
      query: () => ({ command: "picker_ready" }),
    }),
    pickerFrame: builder.query<PickerFrame | null, void>({
      query: () => ({ command: "picker_frame" }),
      keepUnusedDataFor: 0,
    }),
    pickerPixels: builder.mutation<ArrayBuffer, PickerFrame>({
      query: ({ sessionId, profileId, pixelId }) => ({
        command: "picker_pixels",
        args: { sessionId, profileId, pixelId },
      }),
    }),
    pickerFrameDrawn: builder.mutation<void, PickerFrame>({
      query: ({ profileId, pixelId, sessionId }) => ({
        command: "picker_frame_drawn",
        args: { profileId, pixelId, sessionId },
      }),
    }),
    pickerFrameFailed: builder.mutation<void, PickerFrame>({
      query: ({ profileId, pixelId, sessionId }) => ({
        command: "picker_frame_failed",
        args: { profileId, pixelId, sessionId },
      }),
    }),
    listWindowedProcesses: builder.query<WindowedProcess[], void>({
      query: () => ({ command: "windowed_processes_list" }),
      keepUnusedDataFor: 0,
    }),
  }),
});

export const {
  useListProfilesQuery,
  useCreateProfileMutation,
  useUpdateProfileMutation,
  useDeleteProfileMutation,
  useCreateTrackerMutation,
  useUpdateTrackerMutation,
  useDeleteTrackerMutation,
  useTrackerSetRunningMutation,
  useSetTrackerTriggerMutation,
  useSetTrackerHotkeyMutation,
  useBeginKeyCaptureMutation,
  useEndKeyCaptureMutation,
  useEngineActivateMutation,
  useEngineDeactivateMutation,
  useGetSettingsQuery,
  useSetLastScreenMutation,
  useSetPauseHotkeyMutation,
  useClearPauseHotkeyMutation,
  useBeginPauseHotkeyMutation,
  useCancelPauseHotkeyMutation,
  useWorkPauseToggleMutation,
  useActionUpdateMutation,
  useKeyCodesListQuery,
  useActionSoundsListQuery,
  useActionSoundPreviewMutation,
  useActionSoundStopPreviewMutation,
  usePixelUpdateMutation,
  useIconUpdateMutation,
  useIconDeleteMutation,
  useIconCaptureStartMutation,
  useIconCaptureSelectMutation,
  useIconViewOpenMutation,
  usePixelDeleteMutation,
  usePixelCaptureStartMutation,
  usePixelCaptureCancelMutation,
  usePixelCapturePickMutation,
  usePixelViewOpenMutation,
  useLazyScreenshotViewRequestQuery,
  useScreenshotViewReadyMutation,
  useScreenshotViewCloseMutation,
  useScreenshotViewFailedMutation,
  useScreenshotsDirQuery,
  usePickerReadyMutation,
  useLazyPickerFrameQuery,
  usePickerPixelsMutation,
  usePickerFrameDrawnMutation,
  usePickerFrameFailedMutation,
  useListWindowedProcessesQuery,
} = tauriApi;
