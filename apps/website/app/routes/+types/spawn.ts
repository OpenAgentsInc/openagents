import type { Location, Params, LoaderFunctionArgs } from "react-router";

export namespace Route {
  export interface MetaArgs {
    params: Params;
    data: unknown;
    location: Location;
  }

  export type LoaderArgs = LoaderFunctionArgs;
}