import { checkPlanetContent, personalSettingsOnly } from "./planet-source.ts";
import { PlanetSources, PLANET_SOURCE } from "./planet-source.ts";
import {
  MaterialImages,
  materialKey,
  materialSource,
  materialHash,
  sameMaterialDocument,
  sameProjectMaterialLayers,
  exportMaterialSources,
  type MaterialBundle,
  type MaterialTarget,
} from "./creator-material.ts";
import {
  DEFAULT_EFFECT,
  MATERIAL_PARAMETERS,
  ENVIRONMENT_PARAMETERS,
  checkedValue,
  materialEffect,
  type AuthorAction,
  type Authoring,
  type AuthorView,
} from "./authoring.ts";
import { matchesTemplate, type ProjectDocument } from "./project-document.ts";
/** Coordinates real kernel prepare/commit and GPU activation without replacing the device. */
export class ProjectAuthoring {
  draft = DEFAULT_EFFECT;
  diagnostics: string[] = [];
  #desired = 0;
  readonly materialImages = new MaterialImages();
  readonly planetSources = new PlanetSources();
  originals: ProjectDocument["originals"] = [];
  constructor(
    private readonly port: {
      snapshot(): any;
      kernel(request: unknown): any;
      materialIds(): { id: string; objects: string[] }[];
      hasAsset(id: string): boolean;
      rendererSource: string;
      shader(source: string, current: () => boolean): Promise<boolean>;
      activate(snapshot: any, prepared?: boolean): Promise<void>;
      prepare?(
        snapshot: any,
        shader: string | undefined,
        current: () => boolean,
      ): Promise<{ ready(): boolean; activate(): void; discard(): void }>;
      template?(): ProjectDocument["template"];
      namespace?(): ProjectDocument["namespace"];
      restoreCamera?(checkpoint: unknown): unknown;
      fallbackCamera?(): unknown;
      restoreEntity?(
        id: string,
        entity: ProjectDocument["entities"][string],
      ): unknown;
    },
  ) {}
  supersede() {
    return ++this.#desired;
  }
  view(): AuthorView {
    const s = this.port.snapshot();
    return {
      schema: "openagents.verse.authoring.v1",
      effect_scope: "world",
      revision: s.revision,
      can_undo: !!s.canUndo,
      can_redo: !!s.canRedo,
      draft: this.draft,
      active_source: s.authoring?.effect ?? DEFAULT_EFFECT,
      diagnostics: this.diagnostics.slice(0, 8),
      materials: this.port.materialIds(),
      environment: s.authoring?.environment ?? {},
    };
  }
  export(): ProjectDocument {
    if (!this.port.template)
      throw new Error("This world cannot export an editable project.");
    const s = this.port.snapshot();
    const document: ProjectDocument = {
      schema: "openagents.verse.project.v1",
      namespace: this.port.namespace?.() ?? "haunted-house",
      template: this.port.template(),
      source_revision: s.revision,
      entities: Object.fromEntries(
        (s.entities ?? []).map((e: any) => [
          e.id,
          { asset: e.kind, position: e.position, yaw: e.yaw, scale: e.scale },
        ]),
      ),
      rooms: Object.fromEntries(
        (s.rooms ?? []).map((r: any) => [
          r.id,
          {
            center: r.center,
            size: r.size,
            height: r.height,
            floor_y: r.floorY,
          },
        ]),
      ),
      removed_walls: [...(s.removedWalls ?? [])].sort(),
      materials: structuredClone(s.authoring?.materials ?? {}),
      environment: structuredClone(s.authoring?.environment ?? {}),
      source_draft: this.draft,
      active_source: s.authoring?.effect ?? DEFAULT_EFFECT,
      layers: [],
      originals: structuredClone(this.originals).filter(
        (o) =>
          this.port.namespace?.() !== "personal-planet" ||
          o.name !== PLANET_SOURCE,
      ),
    };
    if (s.authoring?.planetSource)
      document.originals.push(structuredClone(s.authoring.planetSource));
    exportMaterialSources(document, s);
    if (new TextEncoder().encode(JSON.stringify(document)).length > 48 * 1024)
      throw new Error("This project is too large to export here.");
    return document;
  }
  async execute(
    action: AuthorAction,
    base: number,
    id: string,
    ticket: number,
  ) {
    const old = this.port.snapshot();
    if (base !== old.revision)
      throw new Error(
        "The world changed. Review the current edit before trying again.",
      );
    if (action.type === "draft") {
      if (
        action.asset !== "world-effect" ||
        new TextEncoder().encode(action.source).length > 4096
      )
        throw new Error(
          "Choose the world effect and keep its source below 4 KiB.",
        );
      this.draft = action.source;
      this.diagnostics = [];
      return;
    }
    const values: Authoring = structuredClone(
      old.authoring ?? { environment: {}, materials: {}, effect: null },
    );
    const operations: any[] = [];
    if (
      action.type === "restore_project" ||
      action.type === "restore_material_project"
    ) {
      if (!this.port.template || !this.port.restoreEntity)
        throw new Error("This world cannot open editable projects.");
      const d = action.document;
      matchesTemplate(
        d,
        this.port.template(),
        action.type === "restore_material_project",
        this.port.namespace?.() ?? "haunted-house",
      );
      const planetSource = await this.planetSources.accept(d);
      const materialSources: NonNullable<Authoring["materialSources"]> = {};
      if (action.type === "restore_material_project") {
        if (action.materials.length > 8)
          throw new Error("Open at most eight face materials in this editor.");
        const restored = {
          entities: Object.entries(d.entities).map(([id, e]) => ({
            id,
            kind: e.asset,
          })),
        };
        for (const bundle of action.materials) {
          await this.materialImages.accept(bundle, restored);
          const key = await materialKey(bundle.document.target);
          const source = d.originals.find(
            (o) => o.name === `material-v1-${key}.json`,
          );
          if (
            materialSources[key] ||
            !source ||
            (await materialHash(new TextEncoder().encode(source.text))) !==
              source.sha256 ||
            !sameMaterialDocument(JSON.parse(source.text), bundle.document)
          )
            throw new Error(
              "The saved face sources do not match their images.",
            );
          materialSources[key] = source;
        }
        const sourceCount = d.originals.filter((o) =>
          o.name.startsWith("material-v1-"),
        ).length;
        if (sourceCount !== action.materials.length)
          throw new Error(
            "Reopen this project with all its saved face images.",
          );
        const expected = { ...structuredClone(d), layers: [], originals: [] };
        exportMaterialSources(expected, {
          ...restored,
          authoring: { materialSources },
        });
        if (!sameProjectMaterialLayers(expected.layers, d.layers))
          throw new Error("The saved face layers do not match their sources.");
      }
      const knownMaterials = new Set(this.port.materialIds().map((m) => m.id));
      for (const [id, parameters] of Object.entries(d.materials)) {
        if (!knownMaterials.has(id))
          throw new Error("This save refers to an unavailable material.");
        for (const [parameter, value] of Object.entries(parameters))
          checkedValue(parameter, value, MATERIAL_PARAMETERS);
      }
      for (const [parameter, value] of Object.entries(d.environment))
        checkedValue(parameter, value, ENVIRONMENT_PARAMETERS);
      const nextAuthoring = {
        environment: d.environment,
        materials: d.materials,
        effect: d.active_source,
        materialSources,
        ...(planetSource ? { planetSource } : {}),
      };
      if (
        planetSource &&
        old.authoring?.planetSource &&
        personalSettingsOnly(this.export(), d)
      ) {
        operations.push({ op: "set_authoring", state: nextAuthoring });
      } else
        operations.push({
          op: "replace_content",
          checkpoint: action.checkpoint
            ? this.port.restoreCamera?.(action.checkpoint)
            : null,
          fallback: this.port.fallbackCamera?.() ?? null,
          entities: Object.entries(d.entities).map(([id, e]) =>
            this.port.restoreEntity!(id, e),
          ),
          rooms: Object.entries(d.rooms).map(([id, r]) => ({
            id,
            center: r.center,
            size: r.size,
            height: r.height,
            floorY: r.floor_y,
          })),
          removedWalls: d.removed_walls,
          authoring: {
            environment: d.environment,
            materials: d.materials,
            effect: d.active_source,
            materialSources,
            ...(planetSource ? { planetSource } : {}),
          },
        });
    } else if (action.type === "apply_material") {
      await this.materialImages.accept(action.material, old);
      const key = await materialKey(action.material.document.target);
      (values.materialSources ??= {})[key] = await materialSource(
        action.material.document,
      );
      if (
        this.originals.filter((o) => !o.name.startsWith("material-v1-"))
          .length +
          Object.keys(values.materialSources).length >
        8
      )
        throw new Error(
          "This project has room for eight source files. Export your work before adding another face material.",
        );
      operations.push({ op: "set_authoring", state: values });
    } else if (action.type === "propose") {
      if (!action.operations.length || action.operations.length > 8)
        throw new Error("Choose one to eight changes.");
      const materialIds = new Set(this.port.materialIds().map((x) => x.id));
      for (const edit of action.operations) {
        if (edit.type === "material") {
          const env = edit.target === "environment";
          checkedValue(
            edit.parameter,
            edit.value,
            env ? ENVIRONMENT_PARAMETERS : MATERIAL_PARAMETERS,
          );
          if (env) values.environment[edit.parameter] = edit.value;
          else {
            if (!materialIds.has(edit.target))
              throw new Error("Choose a material in this world.");
            (values.materials[edit.target] ??= {})[edit.parameter] = edit.value;
          }
        } else if (edit.type === "move") {
          operations.push({
            op: "move_entity",
            id: edit.target,
            position: edit.position,
          });
        } else if (edit.type === "remove") {
          for (const [key, source] of Object.entries(
            values.materialSources ?? {},
          ))
            if (JSON.parse(source.text).target.entity === edit.target)
              delete values.materialSources![key];
          operations.push({ op: "remove_entity", id: edit.target });
        } else if (edit.type === "place") {
          if (!this.port.hasAsset(edit.asset))
            throw new Error("Choose an asset in this world.");
          operations.push({
            op: "add_entity",
            entity: {
              id: crypto.randomUUID(),
              kind: edit.asset,
              position: edit.position,
            },
          });
        }
      }
      if (JSON.stringify(values) !== JSON.stringify(old.authoring))
        operations.push({ op: "set_authoring", state: values });
      if (operations.length > 8)
        throw new Error("Keep this group to eight changes.");
    } else if (action.type === "compile") {
      if (action.asset !== "world-effect")
        throw new Error("Choose the world effect to compile.");
      const hash = [
        ...new Uint8Array(
          await crypto.subtle.digest(
            "SHA-256",
            new TextEncoder().encode(this.draft),
          ),
        ),
      ]
        .map((v) => v.toString(16).padStart(2, "0"))
        .join("");
      if (action.source_sha256 !== hash)
        throw new Error(
          "The source draft changed. Review it before compiling again.",
        );
      values.effect = this.draft;
      operations.push({ op: "set_authoring", state: values });
    } else operations.push({ op: action.type });
    const command = { commandId: id, baseRevision: base, operations };
    let prepared:
      Awaited<ReturnType<NonNullable<typeof this.port.prepare>>> | undefined;
    try {
      const candidate = this.port.kernel({ type: "prepare", ...command });
      if (!candidate.ok) throw new Error(candidate.error.message);
      if (this.port.namespace?.() === "personal-planet")
        checkPlanetContent(candidate);
      const current = () =>
        ticket === this.#desired && base === this.port.snapshot().revision;
      const effect = candidate.authoring?.effect ?? null;
      const source =
        effect !== (old.authoring?.effect ?? null)
          ? effect === null
            ? this.port.rendererSource
            : materialEffect(effect, this.port.rendererSource)
          : undefined;
      if (this.port.prepare) {
        prepared = await this.port.prepare(candidate, source, current);
      } else if (source !== undefined) {
        if (!(await this.port.shader(source, current)))
          throw new Error("A newer edit replaced this compilation.");
      }
      if (!current() || (prepared && !prepared.ready()))
        throw new Error("A newer edit replaced this compilation.");
      const accepted = this.port.kernel({ type: "command", ...command });
      if (!accepted.ok) throw new Error(accepted.error.message);
      prepared?.activate();
      await this.port.activate(accepted, !!prepared);
      if (
        this.port.namespace?.() === "personal-planet" &&
        (action.type === "undo" || action.type === "redo") &&
        this.draft === (old.authoring?.effect ?? DEFAULT_EFFECT)
      )
        this.draft = accepted.authoring?.effect ?? DEFAULT_EFFECT;
      if (
        action.type === "restore_project" ||
        action.type === "restore_material_project"
      ) {
        this.draft = action.document.source_draft;
        this.originals = structuredClone(action.document.originals);
      }
      this.diagnostics = [];
    } catch (error) {
      this.diagnostics = [
        String(error instanceof Error ? error.message : error).slice(0, 2048),
      ];
      throw error;
    } finally {
      prepared?.discard();
    }
  }
}
