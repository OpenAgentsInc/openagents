import type { MetaFunction } from "react-router";

export namespace Route {
  export type MetaArgs = Parameters<MetaFunction>[0];
  export type LinksFunction = () => {
    rel: string;
    href: string;
    crossOrigin?: string;
  }[];
  export type ErrorBoundaryProps = { error: unknown };
}
