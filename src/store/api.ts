import { createApi } from "@reduxjs/toolkit/query/react";
import { tauriBaseQuery } from "./tauriBaseQuery";
import type {
  Action,
  KeyCodeInfo,
  Pixel,
  PixelInput,
  Profile,
  ProfileInput,
  ScreenshotViewRequest,
  Tracker,
  TrackerInput,
  WindowedProcess,
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
  tagTypes: ["Profile"],
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
    engineActivate: builder.mutation<void, { profileId: string }>({
      query: ({ profileId }) => ({
        command: "engine_activate",
        args: { profileId },
      }),
    }),
    engineDeactivate: builder.mutation<void, void>({
      query: () => ({ command: "engine_deactivate" }),
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
    pixelUpdate: builder.mutation<Pixel, PixelArgs & { input: PixelInput }>({
      query: ({ profileId, trackerId, pixelId, input }) => ({
        command: "pixel_update",
        args: { profileId, trackerId, pixelId, input },
      }),
      invalidatesTags: (_result, _error, { profileId }) => [
        { type: "Profile", id: profileId },
      ],
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
      { profileId: string; trackerId: string }
    >({
      query: ({ profileId, trackerId }) => ({
        command: "pixel_capture_start",
        args: { profileId, trackerId },
      }),
    }),
    pixelCaptureCancel: builder.mutation<void, void>({
      query: () => ({ command: "pixel_capture_cancel" }),
    }),
    pixelCapturePick: builder.mutation<void, { x: number; y: number }>({
      query: ({ x, y }) => ({ command: "pixel_capture_pick", args: { x, y } }),
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
  useEngineActivateMutation,
  useEngineDeactivateMutation,
  useActionUpdateMutation,
  useKeyCodesListQuery,
  usePixelUpdateMutation,
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
  useListWindowedProcessesQuery,
} = tauriApi;
