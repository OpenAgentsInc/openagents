/** A loaded player never acquires authoring, source export, or a second release. */
export class PlayerMode {
  active = false;
  loaded: string | undefined;
  loading = false;
  enter(player: boolean) {
    this.active ||= player;
  }
  allows(type: string) {
    return (
      !this.active ||
      ![
        "author",
        "command",
        "mmorpgEditing",
        "battle",
        "setShader",
        "setEnvironment",
        "setSceneEffects",
        "previewImpact",
        "exportProject",
        "materialView",
      ].includes(type)
    );
  }
  begin(id: string) {
    if (!this.active || !id || this.loading)
      throw new Error("This player cannot load another version.");
    if (this.loaded) {
      if (this.loaded === id) return false;
      throw new Error("This player stays on its opened version.");
    }
    this.loading = true;
    return true;
  }
  finish(id: string) {
    this.loaded = id;
    this.loading = false;
  }
  fail() {
    this.loading = false;
  }
}
