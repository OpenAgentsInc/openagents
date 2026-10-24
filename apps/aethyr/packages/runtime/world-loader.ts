import type {
  CompiledScene,
  CompiledEnvironments,
  SceneLook,
} from "../assets/src/scene.ts";
import { loadEnemyPatrols } from "./enemies.ts";
import {
  loadMmorpgDocument,
  loadMmorpgWorld,
  MMORPG_CLASSES,
  type MmorpgClass,
} from "../assets/src/mmorpg.ts";
import type { CharacterManifest } from "../renderer/character.ts";
import {
  validMmoIconsManifest,
  type MmoIconsManifest,
} from "./mmorpg-protocol.ts";
import { validateGrassManifest } from "../renderer/grass.ts";
import { readAsset } from "../assets/src/asset-download.ts";
import { mergeEnvironmentScenario } from "../assets/src/environment.ts";
import { Renderer, type RendererOptions } from "../renderer/renderer.ts";
import { selectedRendererFeatures } from "../renderer/features.ts";
import type { IblManifest } from "../renderer/ibl.ts";
import type { WaterManifest } from "../renderer/water-pass.ts";
import type { SurfaceDecalManifest } from "../renderer/surface-decals.ts";
import type { AmbientParticleManifest } from "../renderer/ambient-particles.ts";
import type { SourceWaterStreamsManifest } from "../renderer/source-water-streams.ts";
import type { SourceWaterSplashesManifest } from "../renderer/source-water-splashes.ts";
import type { CreaturesManifest } from "../renderer/creatures.ts";
import type { LeavesManifest } from "../renderer/leaves.ts";
import type { SourceFogCardsManifest } from "../renderer/source-fog-cards.ts";
import type { SourceWindManifest } from "../renderer/wind-source.ts";
import {
  validateSurfaceSkySources,
  type SurfaceSkyManifest,
} from "../renderer/surface-sky.ts";
import {
  giSceneIdentityPayload,
  giEnvironmentIdentityPayload,
  type GiProbeManifest,
} from "../renderer/gi-probes.ts";
import { probeSha256 } from "../renderer/probe-textures.ts";
import {
  selectProbeProfile,
  type GiProbeProfiles,
} from "../renderer/probe-profiles.ts";
import {
  validateBounceManifest,
  type GiBounceManifests,
} from "../renderer/bounce-textures.ts";
import {
  giBounceCoverageIdentityPayload,
  giBounceMaterialIdentityPayload,
  type GiBounceManifest,
} from "../renderer/gi-bounce.ts";
import { sourceBloomStrength } from "../renderer/source-bloom.ts";
import { sourceFilmicControls } from "../renderer/filmic.ts";
import {
  loadSourceDisplay,
  type SourceDisplayProfiles,
} from "../renderer/source-display.ts";
import { sunAngles } from "../renderer/lighting.ts";
import {
  DEFAULT_ENVIRONMENT,
  validateEnvironment,
  EnvironmentProfiles,
} from "./environment.ts";
import {
  parseOverviewPresentation,
  type OverviewPresentation,
} from "./overview.ts";
import { loadStaticCollision } from "./static-collision.ts";
import { loadDestructibleScene } from "./destructible-scene.ts";

export interface WorldLoaderOptions {
  /** Selected by the trusted Open Ground host, never by scene content. */
  allowQuickCombat?: boolean;
  report: (message: string) => void;
  presentationURL?: string;
  rendererOptions?: RendererOptions;
}

/** Prepare one compatible world before either player or editor adopts it. */
export async function prepareWorld(
  scene: CompiledScene,
  nightScene: CompiledScene,
  targetCanvas: OffscreenCanvas,
  options: WorldLoaderOptions,
) {
  let environment = { ...DEFAULT_ENVIRONMENT };
  let look: SceneLook | undefined;
  let overview: OverviewPresentation | undefined;
  const profiles = new EnvironmentProfiles();
  if (scene.mmorpgURL !== nightScene.mmorpgURL)
    throw new Error("Prepare the same game for day and night.");
  if (scene.mmorpgURL && !options.allowQuickCombat)
    throw new Error("Open this game in the Open Ground player.");
  const mmorpg = scene.mmorpgURL
    ? await loadMmorpgWorld(scene.mmorpgURL)
    : undefined;
  const mmorpgCharactersLoading = mmorpg
    ? Promise.all(
        MMORPG_CLASSES.map(
          async (kind) =>
            [
              kind,
              await (
                options.rendererOptions?.features ?? selectedRendererFeatures
              ).character!.load(mmorpg.characters[kind]),
            ] as const,
        ),
      ).then(
        (entries) =>
          Object.fromEntries(entries) as Record<MmorpgClass, CharacterManifest>,
      )
    : Promise.resolve(undefined);
  void mmorpgCharactersLoading.catch(() => {});
  const mmorpgIconsLoading = mmorpg
    ? loadMmorpgDocument(mmorpg.uiURL, "ui", 64 * 1024).then((value) => {
        if (!validMmoIconsManifest(value))
          throw Error("Prepare the complete game pictures.");
        return value as MmoIconsManifest;
      })
    : Promise.resolve(undefined);
  void mmorpgIconsLoading.catch(() => {});
  const configureEnvironmentLighting = (ibl?: IblManifest) => {
    profiles.configureSourceIntensity({
      day: ibl?.scenarios.day?.gain?.sourceSkyLightIntensity,
      night: ibl?.scenarios.night?.gain?.sourceSkyLightIntensity,
    });
    profiles.configureSourceBloom({
      day: sourceBloomStrength(look, "day", DEFAULT_ENVIRONMENT.bloomStrength),
      night: sourceBloomStrength(
        look,
        "night",
        DEFAULT_ENVIRONMENT.bloomStrength,
      ),
    });
    environment = profiles.initial(environment);
  };
  const collisionLoading = Promise.all([
    loadStaticCollision(scene),
    loadStaticCollision(nightScene),
  ]);
  const gameplayLoading = Promise.all([
    loadDestructibleScene(scene),
    loadDestructibleScene(nightScene),
  ]);
  // Observe concurrent failures now; the original promises still reject before adoption below.
  void collisionLoading.catch(() => {});
  void gameplayLoading.catch(() => {});
  const features =
    options.rendererOptions?.features ?? selectedRendererFeatures;
  if (scene.characterURL && !features.character)
    throw new Error(
      "Open this world with a player that supports its character.",
    );
  if (
    scene.enemyCharacterURL !== nightScene.enemyCharacterURL ||
    scene.enemyPatrolsURL !== nightScene.enemyPatrolsURL ||
    (!scene.enemyCharacterURL && !!scene.enemyPatrolsURL) ||
    (scene.enemyCharacterURL &&
      !scene.enemyPatrolsURL &&
      !options.allowQuickCombat)
  )
    throw new Error(
      "Prepare matching knight models and routes for day and night.",
    );
  if (scene.enemyCharacterURL && !features.character)
    throw new Error("Open this town with a player that supports its knights.");
  const enemiesLoading =
    scene.enemyCharacterURL && !mmorpg
      ? Promise.all([
          features.character!.load(scene.enemyCharacterURL),
          scene.enemyPatrolsURL
            ? loadEnemyPatrols(scene.enemyPatrolsURL)
            : Promise.resolve(undefined),
        ])
      : Promise.resolve(undefined);
  void enemiesLoading.catch(() => {});
  const characterLoading =
    scene.characterURL && !mmorpg
      ? features.character!.load(scene.characterURL)
      : Promise.resolve(undefined);
  void characterLoading.catch(() => {});
  if (
    nightScene.vertices !== scene.vertices ||
    nightScene.indices !== scene.indices ||
    nightScene.materials.length !== scene.materials.length
  )
    throw new Error(
      "Prepare day and night assets together before opening the town.",
    );
  if (scene.lookURL) {
    look = await readAsset(scene.lookURL, async (response) => {
      if (!response.ok)
        throw new Error(
          "The source camera and lighting settings could not open.",
        );
      return (await response.json()) as SceneLook;
    });
    const sun = look.scenarios.day.sun;
    environment = validateEnvironment({
      ...environment,
      sunIntensity: sun?.intensity ?? DEFAULT_ENVIRONMENT.sunIntensity,
      moonIntensity:
        look.scenarios.night.sun?.intensity ??
        DEFAULT_ENVIRONMENT.moonIntensity,
      ...(sun ? sunAngles(sun) : {}),
    });
  }
  const documents = new Map<string, Promise<unknown>>();
  const document = async <T>(url: string, failure: string): Promise<T> => {
    let pending = documents.get(url);
    if (!pending) {
      pending = readAsset(url, (response) => {
        if (!response.ok) throw new Error(failure);
        return response.json();
      });
      documents.set(url, pending);
    }
    return (await pending) as T;
  };
  let environments: CompiledEnvironments | undefined;
  let lightingEnvironments: CompiledEnvironments | undefined;
  const lightingSources = new Map<"day" | "night", CompiledEnvironments>();
  for (const source of [scene, nightScene]) {
    const mode = source.scenario;
    const visibleURL = source.environmentURL ?? scene.environmentURL;
    const lightingURL = source.lightingEnvironmentURL ?? visibleURL;
    for (const [kind, url] of [
      ["visible", visibleURL],
      ["lighting", lightingURL],
    ] as const) {
      if (!url) continue;
      const description = await document<CompiledEnvironments>(
        url,
        "The prepared HDR skies could not open.",
      );
      if (
        description.schema !== "aethyrforge.private-environments.v1" ||
        !description.scenarios[mode]
      )
        throw new Error(`Prepare HDR skies for the ${mode} town scene.`);
      if (kind === "visible") {
        environments ??= {
          schema: "aethyrforge.private-environments.v1",
          scenarios: {},
        };
        environments.scenarios[mode] = description.scenarios[mode];
      } else {
        lightingEnvironments = mergeEnvironmentScenario(
          lightingEnvironments,
          description,
          mode,
        );
        lightingSources.set(mode, description);
      }
    }
  }
  if (environments) {
    environment = { ...environment, skySource: "kit" };
  }
  let ibl: IblManifest | undefined;
  for (const [mode, source] of lightingSources) {
    if (!source.lightingURL) continue;
    const candidate = await document<IblManifest>(
      source.lightingURL,
      "The prepared sky lighting could not open.",
    );
    if (
      candidate.schema !== "aethyrforge.private-environment-lighting.v1" ||
      candidate.profile !== "sh9-ggx-split-sum-v1" ||
      !candidate.scenarios[mode]
    )
      throw new Error(`Prepare sky lighting for the ${mode} town scene.`);
    if (
      ibl &&
      (["url", "width", "height", "format"] as const).some(
        (key) => ibl!.brdfLut[key] !== candidate.brdfLut[key],
      )
    )
      throw new Error(
        "Prepare day and night lighting with the same reflection profile.",
      );
    ibl ??= { ...candidate, scenarios: {} };
    ibl.scenarios[mode] = candidate.scenarios[mode];
  }
  configureEnvironmentLighting(ibl);
  if (options.presentationURL) {
    overview = parseOverviewPresentation(
      await document<unknown>(
        options.presentationURL,
        "The overview camera and lighting could not open.",
      ),
    );
    environment = overview.environment;
  }
  let water: WaterManifest | undefined;
  if (scene.waterURL) {
    water = await readAsset(scene.waterURL, async (response) => {
      if (!response.ok)
        throw new Error("The prepared fountain water could not open.");
      return (await response.json()) as WaterManifest;
    });
  }
  const surfaceDecals: Partial<Record<"day" | "night", SurfaceDecalManifest>> =
    {};
  for (const source of [scene, nightScene]) {
    if (!source.surfaceDecalsURL) continue;
    surfaceDecals[source.scenario] = await readAsset(
      source.surfaceDecalsURL,
      async (response) => {
        if (!response.ok)
          throw new Error("The prepared surface details could not open.");
        return (await response.json()) as SurfaceDecalManifest;
      },
    );
  }
  const probes: GiProbeProfiles = {};
  for (const source of [scene, nightScene]) {
    if (source.probesURL) {
      const candidate = await document<GiProbeManifest>(
        source.probesURL,
        "The prepared sky lighting could not open.",
      );
      const selected = selectProbeProfile(candidate, source.scenario);
      const record = selected.scenarios[source.scenario]!;
      const environmentMap = lightingEnvironments?.scenarios[source.scenario];
      const encoder = new TextEncoder();
      if (
        !environmentMap ||
        (await probeSha256(encoder.encode(giSceneIdentityPayload(source)))) !==
          (record.sourceGeometrySha256 ?? selected.sourceGeometrySha256) ||
        (await probeSha256(
          encoder.encode(giEnvironmentIdentityPayload(environmentMap)),
        )) !== record.sourceEnvironmentSha256
      )
        throw new Error("Rebuild sky lighting for the current town assets.");
      // Preserve the published base identity for linked bounce fields; GPU upload still selects one mode.
      probes[source.scenario] = candidate;
    }
  }
  const bounceManifests: GiBounceManifests = {};
  for (const source of [scene, nightScene]) {
    const base = probes[source.scenario];
    if (!base?.bounceURL) continue;
    const bounce = await document<GiBounceManifest>(
      base.bounceURL,
      "The prepared bounce lighting could not open.",
    );
    await validateBounceManifest(base, bounce, source.scenario);
    const scenario = bounce.scenarios[source.scenario]!,
      encoder = new TextEncoder();
    if (
      (await probeSha256(
        encoder.encode(giBounceMaterialIdentityPayload(source)),
      )) !== scenario.sourceMaterialSha256 ||
      (await probeSha256(
        encoder.encode(giBounceCoverageIdentityPayload(source)),
      )) !== scenario.sourceCoverageSha256
    )
      throw new Error("Rebuild bounce lighting for the current town surfaces.");
    bounceManifests[source.scenario] = bounce;
  }
  let particles: AmbientParticleManifest | undefined;
  const particlesURL = scene.particlesURL ?? nightScene.particlesURL;
  if (
    scene.particlesURL &&
    nightScene.particlesURL &&
    scene.particlesURL !== nightScene.particlesURL
  )
    throw new Error(
      "Prepare the town effects together before opening this world.",
    );
  if (particlesURL)
    particles = await document<AmbientParticleManifest>(
      particlesURL,
      "The prepared town effects could not open.",
    );
  const streamURL =
    scene.sourceWaterStreamsURL ?? nightScene.sourceWaterStreamsURL;
  if (
    scene.sourceWaterStreamsURL &&
    nightScene.sourceWaterStreamsURL &&
    scene.sourceWaterStreamsURL !== nightScene.sourceWaterStreamsURL
  )
    throw new Error("Prepare both town water scenarios together.");
  const sourceWaterStreams = streamURL
    ? await document<SourceWaterStreamsManifest>(
        streamURL,
        "The prepared fountain streams could not open.",
      )
    : undefined;
  const pairedEffect = async <T>(
    key:
      | "sourceWaterSplashesURL"
      | "creaturesURL"
      | "leavesURL"
      | "surfaceSkyURL"
      | "fogCardsURL",
    label: string,
  ): Promise<T | undefined> => {
    if (scene[key] && nightScene[key] && scene[key] !== nightScene[key])
      throw new Error(`Prepare both town ${label} scenarios together.`);
    const url = scene[key] ?? nightScene[key];
    return url
      ? document<T>(url, `The prepared ${label} could not open.`)
      : undefined;
  };
  const splashes = await pairedEffect<SourceWaterSplashesManifest>(
    "sourceWaterSplashesURL",
    "fountain foam",
  );
  const creatures = await pairedEffect<CreaturesManifest>(
    "creaturesURL",
    "birds and bats",
  );
  const leaves = await pairedEffect<LeavesManifest>(
    "leavesURL",
    "falling leaves",
  );
  const fogCards = await pairedEffect<SourceFogCardsManifest>(
    "fogCardsURL",
    "background mist",
  );
  const surfaceSky = await pairedEffect<SurfaceSkyManifest>(
    "surfaceSkyURL",
    "surface lighting",
  );
  if (surfaceSky)
    await validateSurfaceSkySources(
      surfaceSky,
      { day: scene, night: nightScene },
      lightingEnvironments?.scenarios ?? {},
    );
  const wind: Partial<Record<"day" | "night", SourceWindManifest>> = {};
  for (const source of [scene, nightScene])
    if (source.windURL)
      wind[source.scenario] = await document<SourceWindManifest>(
        source.windURL,
        "The prepared fabric motion could not open.",
      );
  const displayProfiles: SourceDisplayProfiles = {};
  for (const source of [scene, nightScene])
    if (source.displayLutURL)
      displayProfiles[source.scenario] = await loadSourceDisplay(
        source.displayLutURL,
        source.scenario,
        sourceFilmicControls(look, source.scenario).controls,
      );
  const character = await characterLoading;
  const enemies = await enemiesLoading;
  if (
    enemies &&
    !enemies[1] &&
    enemies[0].animationProfile !== "quick-mmorpg-zombie-v1"
  )
    throw new Error("Prepare the zombie's original model and animations.");
  const [dayCollision, nightCollision] = await collisionLoading;
  const [dayGameplay, nightGameplay] = await gameplayLoading;
  if (scene.grassURL !== nightScene.grassURL)
    throw new Error("Prepare matching grass for day and night.");
  const grass = scene.grassURL
    ? validateGrassManifest(
        await document(scene.grassURL, "The grass settings could not open."),
      )
    : undefined;
  const built = await Renderer.create(
    targetCanvas,
    scene,
    options.report,
    nightScene,
    look,
    environments,
    ibl,
    water,
    probes,
    surfaceDecals,
    lightingEnvironments,
    bounceManifests,
    particles,
    sourceWaterStreams,
    displayProfiles,
    splashes,
    creatures,
    wind,
    leaves,
    surfaceSky,
    fogCards,
    character,
    {
      ...options.rendererOptions,
      ...(enemies ? { enemyCharacter: enemies[0] } : {}),
      ...(grass ? { grass } : {}),
    },
  );
  const mmorpgCharacters = await mmorpgCharactersLoading;
  const mmorpgIcons = await mmorpgIconsLoading;
  if (mmorpg && mmorpgCharacters) {
    options.report("Preparing the game characters…");
    await built.loadMmorpgCharacters(mmorpgCharacters);
    await built.loadMmorpgEffects(mmorpg);
    await built.loadMmorpgScenery(mmorpg);
    if (mmorpg.terrain) await built.loadMmorpgTerrain(mmorpg.terrain);
  }
  return {
    renderer: built,
    scene,
    nightScene,
    environment,
    look,
    overview,
    profiles,
    mmorpg,
    mmorpgCharacters,
    mmorpgIcons,
    character,
    enemyPatrols: enemies?.[1],
    enemyCharacter: enemies?.[0],
    dayCollision,
    nightCollision,
    dayGameplay,
    nightGameplay,
  };
}
