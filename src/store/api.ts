import { createApi } from "@reduxjs/toolkit/query/react";
import { tauriBaseQuery } from "./tauriBaseQuery";

export const tauriApi = createApi({
  reducerPath: "tauriApi",
  baseQuery: tauriBaseQuery,
  endpoints: (builder) => ({
    greet: builder.mutation<string, { name: string }>({
      query: ({ name }) => ({
        command: "greet",
        args: { name },
      }),
    }),
  }),
});

export const { useGreetMutation } = tauriApi;
