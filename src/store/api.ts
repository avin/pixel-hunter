import { createApi } from "@reduxjs/toolkit/query/react";
import { tauriBaseQuery } from "./tauriBaseQuery";
import type {
  Profile,
  ProfileInput,
  Tracker,
  TrackerInput,
  WindowedProcess,
} from "./types";

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
  useListWindowedProcessesQuery,
} = tauriApi;
