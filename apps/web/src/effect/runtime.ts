import * as ManagedRuntime from 'effect/ManagedRuntime';
import { makeAppLayer } from './layer';
import type { AppServices } from './layer';
import type { AppConfig } from './config';

export type AppRuntime = ManagedRuntime.ManagedRuntime<AppServices, never>;

export const makeAppRuntime = (config: AppConfig): AppRuntime =>
  ManagedRuntime.make(makeAppLayer(config));
