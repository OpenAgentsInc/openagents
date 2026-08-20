import { Effect, Layer } from "effect";
import * as Context from "effect/Context";
import { ChildProcess, ChildProcessSpawner } from "effect/unstable/process";

export interface BrowserLauncherInterface {
  readonly open: (url: string) => Effect.Effect<boolean>;
}

export class BrowserLauncher extends Context.Service<BrowserLauncher, BrowserLauncherInterface>()(
  "@openagentsinc/cli/BrowserLauncher",
) {}

export const browserLauncherLayer = Layer.effect(
  BrowserLauncher,
  Effect.gen(function* () {
    const spawner = yield* ChildProcessSpawner.ChildProcessSpawner;
    const open = Effect.fn("BrowserLauncher.open")((url: string) => {
      const command =
        process.platform === "darwin"
          ? ChildProcess.make("open", [url], { stdout: "ignore", stderr: "ignore" })
          : process.platform === "win32"
            ? ChildProcess.make("cmd.exe", ["/c", "start", "", url], {
                stdout: "ignore",
                stderr: "ignore",
              })
            : ChildProcess.make("xdg-open", [url], { stdout: "ignore", stderr: "ignore" });

      return spawner.exitCode(command).pipe(
        Effect.map((code) => Number(code) === 0),
        Effect.orElseSucceed(() => false),
      );
    });
    return BrowserLauncher.of({ open });
  }),
);

export const browserLauncherTestLayer = (open: BrowserLauncherInterface["open"]) =>
  Layer.succeed(BrowserLauncher, BrowserLauncher.of({ open }));
