import { HttpMiddleware } from "@effect/platform";
import { makeHttpRouter } from "@rjdellecese/confect/server";
import { flow } from "effect";
import { OpenAgentsApiLive } from "./http-api";

export default makeHttpRouter({
  "/api/v1/": {
    apiLive: OpenAgentsApiLive,
    middleware: flow(HttpMiddleware.cors(), HttpMiddleware.logger),
  },
});