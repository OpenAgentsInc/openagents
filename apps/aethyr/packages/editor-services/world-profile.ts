/** Deployment-selected profiles. Content cannot choose executable or asset origins. */
import { readAsset } from "../assets/src/asset-download.ts";
export function selectWorldProfile(value: string | undefined) {
  if (value === "open-ground")
    return {
      id: "open-ground",
      name: "Open Ground",
      groundY: 0,
      scene: "/private-assets/scene-ground.json",
      nightScene: "/private-assets/scene-ground-night.json",
      database: "aethyrforge-local-open-ground",
      key: "open-ground",
      mode: "walk" as const,
      readOnly: false,
      presentationURL: undefined as string | undefined,
    };
  if (value === "starter")
    return {
      id: "starter-courtyard",
      name: "Starter Courtyard",
      groundY: 0,
      scene: "/starter/scene.json",
      nightScene: "/starter/scene-night.json",
      database: "aethyrforge-local-starter",
      key: "starter",
      mode: "walk" as const,
      readOnly: false,
      presentationURL: undefined as string | undefined,
    };
  if (value === "haunted-house")
    return {
      id: "haunted-house",
      name: "Haunted House",
      groundY: 0,
      scene: "/private-assets/scene-haunt-day.json",
      nightScene: "/private-assets/scene-haunt-night.json",
      database: "aethyrforge-local-haunted-house",
      key: "haunted-house",
      mode: "walk" as const,
      readOnly: false,
      presentationURL: undefined as string | undefined,
    };
  if (value === "personal-planet")
    return {
      id: "personal-planet",
      name: "Your planet",
      groundY: 0,
      scene: "/private-assets/scene-planet-day.json",
      nightScene: "/private-assets/scene-planet-night.json",
      database: "aethyrforge-local-personal",
      key: "personal-planet",
      mode: "walk" as const,
      readOnly: false,
      presentationURL: undefined as string | undefined,
    };
  if (value === "medieval-overview")
    return {
      id: "medieval-overview",
      name: "Medieval Overview",
      groundY: 0,
      scene: "/private-assets/scene.json",
      nightScene: "/private-assets/scene.json",
      database: "aethyrforge-local-overview",
      key: "overview",
      mode: "fly" as const,
      readOnly: true,
      presentationURL: "/private-assets/overview-presentation.json",
    };
  if (value !== undefined && value !== "medieval-town")
    throw new Error("This world is not available in this engine release.");
  return {
    id: "medieval-town-lab",
    name: "Medieval Town",
    groundY: 0,
    scene: "/private-assets/scene.json",
    nightScene: "/private-assets/scene-night.json",
    database: "aethyrforge-local-town",
    key: "town",
    mode: "walk" as const,
    readOnly: false,
    presentationURL: undefined as string | undefined,
  };
}
export const WORLD_PROFILE = selectWorldProfile(
  import.meta.env?.VITE_WORLD_PROFILE,
);

export interface HauntProject {
  sourceSha256: string;
  schema: "aethyrforge.haunt-project.v1" | "aethyrforge.personal-project.v1";
  profile: "haunted-house" | "personal-planet";
  entities: unknown[];
  colliders: unknown[];
  rooms: unknown[];
  removedWalls: string[];
  environment: import("./protocol").EnvironmentSettings;
  sceneURL: string;
  nightSceneURL: string;
}
export async function loadHauntProject(): Promise<HauntProject | undefined> {
  if (
    WORLD_PROFILE.id !== "haunted-house" &&
    WORLD_PROFILE.id !== "personal-planet"
  )
    return undefined;
  const personal = WORLD_PROFILE.id === "personal-planet";
  const prefix = personal ? "planet" : "haunt";
  const read = async (url: string) => {
    if (!/^\/private-assets\/[a-z0-9-]+\.json$/.test(url))
      throw new Error("The world contains an unavailable resource.");
    return readAsset(url, async (response) => {
      if (!response.ok)
        throw new Error("The world could not open. Reload to try again.");
      const text = await response.text();
      if (text.length > 4 * 1024 * 1024)
        throw new Error("The world is too large to open.");
      return JSON.parse(text);
    });
  };
  const profile = await read(`/private-assets/${prefix}-profile.json`);
  const sceneURL = new RegExp(
    `^/private-assets/scene-${prefix}-[a-f0-9]{64}\\.json$`,
  );
  if (
    profile.schema !==
      (personal
        ? "aethyrforge.personal-profile.v1"
        : "aethyrforge.haunt-profile.v1") ||
    !sceneURL.test(profile.scene) ||
    !sceneURL.test(profile.nightScene)
  )
    throw new Error("This world needs a compatible engine release.");
  const project = await read(profile.project);
  if (
    project.schema !==
      (personal
        ? "aethyrforge.personal-project.v1"
        : "aethyrforge.haunt-project.v1") ||
    project.profile !== WORLD_PROFILE.id ||
    !Array.isArray(project.entities) ||
    project.entities.length > 128 ||
    !Array.isArray(project.colliders) ||
    project.colliders.length > 4096 ||
    !Array.isArray(project.rooms) ||
    !Array.isArray(project.removedWalls)
  )
    throw new Error(
      "The world could not open. Its saved work has not changed.",
    );
  return {
    ...project,
    sourceSha256:
      new RegExp(
        `^/private-assets/${prefix}-project-([a-f0-9]{64})\\.json$`,
      ).exec(profile.project)?.[1] ?? "",
    sceneURL: profile.scene,
    nightSceneURL: profile.nightScene,
  };
}

export {
  parseOverviewPresentation,
  type OverviewPresentation,
} from "../runtime/overview.ts";

export function profileSupportsShot(
  profile: ReturnType<typeof selectWorldProfile>,
  shotId: string,
): boolean {
  if (profile.id === "open-ground") return false;
  return profile.readOnly
    ? shotId === "MT-04"
    : /^MT-(?:0[2-9]|1[0-8])$/.test(shotId) && shotId !== "MT-04";
}
