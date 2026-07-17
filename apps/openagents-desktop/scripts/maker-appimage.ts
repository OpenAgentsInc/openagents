import MakerBase, { type MakerOptions } from "@electron-forge/maker-base";
import type { ForgePlatform } from "@electron-forge/shared-types";
import { buildForge, type Configuration } from "app-builder-lib";
import path from "node:path";

export type MakerAppImageConfig = Readonly<{
  artifactName: string;
  appId: string;
  executableName: string;
  productName: string;
  startupWmClass: string;
}>;

/**
 * Forge 7 adapter for electron-builder's AppImage target.
 *
 * Electron Forge does not ship an AppImage maker. Keeping this small adapter
 * in-repo lets the existing descriptor-first Forge hooks remain the package
 * authority while electron-builder is used only for the outer AppImage.
 */
export class MakerAppImage extends MakerBase<MakerAppImageConfig> {
  name = "appimage";
  defaultPlatforms: ForgePlatform[] = ["linux"];

  isSupportedOnCurrentPlatform(): boolean {
    return process.platform === "linux";
  }

  async make(options: MakerOptions): Promise<string[]> {
    const output = path.join(options.makeDir, "appimage", options.targetArch);
    const config: Configuration = {
      appId: this.config.appId,
      productName: this.config.productName,
      executableName: this.config.executableName,
      artifactName: this.config.artifactName,
      directories: { output },
      linux: {
        category: "Development",
        desktop: { entry: { StartupWMClass: this.config.startupWmClass } },
        target: ["AppImage"],
      },
    };
    const artifacts = await buildForge(
      { dir: options.dir },
      {
        config,
        linux: [`appimage:${options.targetArch}`],
      },
    );
    const appImages = artifacts.filter((artifact) => artifact.endsWith(".AppImage"));
    if (appImages.length !== 1) {
      throw new Error(
        `AppImage maker expected exactly one .AppImage artifact, observed ${appImages.length}`,
      );
    }
    return appImages;
  }
}
