import type { Location, Params, LoaderFunctionArgs, ActionFunctionArgs } from "react-router";

export namespace Route {
  export interface MetaArgs {
    params: Params;
    data: unknown;
    location: Location;
  }

  export type LoaderArgs = LoaderFunctionArgs;
  export type ActionArgs = ActionFunctionArgs;
}