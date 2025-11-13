"use client";

import { type FC, type PropsWithChildren } from "react";
import {
  AssistantProvider,
  useAssistantApi,
  useExtendedAssistantApi,
} from "../react/AssistantApiContext";
import { DerivedScope } from "../../utils/tap-store/derived-scopes";

export const PartByIndexProvider: FC<
  PropsWithChildren<{
    index: number;
  }>
> = ({ index, children }) => {
  const baseApi = useAssistantApi();
  const api = useExtendedAssistantApi({
    part: DerivedScope({
      source: "message",
      query: { type: "index", index },
      get: () => baseApi.message().part({ index }),
    }),
  });

  const Provider = AssistantProvider as unknown as FC<
    PropsWithChildren<{ api: ReturnType<typeof useExtendedAssistantApi> }>
  >;
  return <Provider api={api}>{children}</Provider>;
};
