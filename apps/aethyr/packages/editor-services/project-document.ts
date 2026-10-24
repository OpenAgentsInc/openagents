/** Creator content only. The public Rust SDK checks the wire document. */
export type ProjectDocument = {
  schema: "openagents.verse.project.v1";
  namespace: "haunted-house" | "personal-planet";
  template: {
    world: string;
    engine_release: string;
    asset_set_sha256: string;
    scene_sha256: string;
  };
  source_revision: number;
  entities: Record<
    string,
    {
      asset: string;
      position: [number, number, number];
      yaw: number;
      scale: number;
    }
  >;
  rooms: Record<
    string,
    {
      center: [number, number];
      size: [number, number];
      height: number;
      floor_y: number;
    }
  >;
  removed_walls: string[];
  materials: Record<string, Record<string, number>>;
  environment: Record<string, number>;
  source_draft: string;
  active_source: string;
  layers: unknown[];
  originals: { name: string; sha256: string; text: string }[];
};

export function matchesTemplate(
  document: ProjectDocument,
  template: ProjectDocument["template"],
  materialLayers = false,
  namespace: ProjectDocument["namespace"] = "haunted-house",
) {
  if (
    document.schema !== "openagents.verse.project.v1" ||
    document.namespace !== namespace ||
    document.template.world !== template.world ||
    document.template.asset_set_sha256 !== template.asset_set_sha256 ||
    document.template.scene_sha256 !== template.scene_sha256
  )
    throw new Error("Open this save with its matching world and asset set.");
  // Renderer revisions may change; source and asset identities cannot silently change.
  if (document.layers.length && !materialLayers)
    throw new Error(
      "This editor cannot open these material layers yet. Keep the export.",
    );
}
