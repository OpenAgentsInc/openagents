export const MAX_BUNDLED_SOURCE_DRAWS = 131072;
const WORDS = 5;
const MAX_BUNDLES = 8;
export const SOURCE_BUNDLE_PAGE_DRAWS = 256;

export interface BundledSourceDraw {
  sourceDrawIndex: number;
  indexCount: number;
  firstIndex: number;
  baseVertex: number;
  firstInstance: number;
  count: number;
  material: number;
  wind?: boolean;
  rasterOrientation?: "positive" | "negative" | "degenerate";
}
export interface SourceBundleBindings {
  key: string;
  descriptor: GPURenderBundleEncoderDescriptor;
  frameGroup: GPUBindGroup;
  shader: object;
  materials: object;
  pipelineVersion: number;
  vertices: GPUBuffer;
  instances: GPUBuffer;
  colors: GPUBuffer;
  uv1: GPUBuffer;
  wind: GPUBuffer;
  zeroWind: GPUBuffer;
  indices: GPUBuffer;
  include(draw: BundledSourceDraw): boolean;
  material(draw: BundledSourceDraw): GPUBindGroup;
  pipeline(draw: BundledSourceDraw): GPURenderPipeline;
}

interface SourceBundleCommands {
  draws: number;
  pipelines: number;
  materials: number;
  windBindings: number;
}

interface SourceBundlePage {
  revision: number;
  bundle: GPURenderBundle | undefined;
  commands: SourceBundleCommands;
}

/** Fixed source offsets; each pass records bounded pages in original source order. */
export class SourceRenderBundles {
  readonly indirect: GPUBuffer;
  private readonly words: Uint32Array;
  private closed = false;
  private readonly bundles = new Map<
    string,
    {
      resources: unknown[];
      revision: number;
      pages: (SourceBundlePage | undefined)[];
      bundles: readonly GPURenderBundle[];
      commands: SourceBundleCommands;
    }
  >();
  private uploads = 0;
  private builds = 0;
  private slotUpdates = 0;
  private pageReuses = 0;
  private reuses = 0;
  private uploadBytes = 0;
  private uploadPending = false;
  private readonly instanceCapacity: number;
  private readonly admitted: Uint8Array;
  private readonly pendingIds: Uint32Array;
  private readonly pageRevisions: Uint32Array;
  private admittedCount = 0;
  private membershipRevision = 0;
  private lastCommands: SourceBundleCommands = {
    draws: 0,
    pipelines: 0,
    materials: 0,
    windBindings: 0,
  };

  constructor(
    private readonly device: GPUDevice,
    readonly draws: readonly BundledSourceDraw[],
    private readonly account: (delta: number) => void = () => {},
    private readonly pruneNeverVisible = true,
  ) {
    if (draws.length > MAX_BUNDLED_SOURCE_DRAWS)
      throw new Error("The source draw table exceeds its render bundle limit.");
    this.instanceCapacity = draws.reduce(
      (maximum, draw) => Math.max(maximum, draw.firstInstance + draw.count),
      0,
    );
    this.words = new Uint32Array(draws.length * WORDS);
    this.admitted = new Uint8Array(draws.length);
    this.pendingIds = new Uint32Array(draws.length);
    this.pageRevisions = new Uint32Array(
      Math.ceil(draws.length / SOURCE_BUNDLE_PAGE_DRAWS),
    );
    draws.forEach((draw, index) => {
      if (
        draw.sourceDrawIndex !== index ||
        !this.uint(draw.indexCount) ||
        !this.uint(draw.firstIndex) ||
        !Number.isInteger(draw.baseVertex) ||
        draw.baseVertex < -0x80000000 ||
        draw.baseVertex > 0x7fffffff ||
        !this.uint(draw.firstInstance) ||
        !this.uint(draw.count)
      )
        throw new Error(
          "Use a stable, complete source draw table before recording bundles.",
        );
      this.words[index * WORDS] = draw.indexCount;
      this.words[index * WORDS + 2] = draw.firstIndex;
      this.words[index * WORDS + 3] = draw.baseVertex >>> 0;
    });
    this.indirect = device.createBuffer({
      label: "Source indexed indirect commands",
      size: Math.max(20, this.words.byteLength),
      usage: GPUBufferUsage.INDIRECT | GPUBufferUsage.COPY_DST,
    });
    this.account(this.indirect.size);
    if (this.words.byteLength)
      try {
        this.upload();
      } catch (error) {
        this.dispose();
        throw error;
      }
  }

  private uint(value: number) {
    return Number.isInteger(value) && value >= 0 && value <= 0xffffffff;
  }
  private open() {
    if (this.closed) throw new Error("The source render bundles have closed.");
  }
  private upload() {
    this.uploadPending = true;
    this.device.queue.writeBuffer(this.indirect, 0, this.words);
    this.uploadPending = false;
    this.uploads++;
    this.uploadBytes += this.words.byteLength;
  }

  private publishMembership(count: number): void {
    if (!count) return;
    let previousPage = -1;
    for (let i = 0; i < count; i++) {
      const id = this.pendingIds[i];
      this.admitted[id] = 1;
      const page = Math.floor(id / SOURCE_BUNDLE_PAGE_DRAWS);
      if (page !== previousPage) this.pageRevisions[page]++;
      previousPage = page;
    }
    this.admittedCount += count;
    this.membershipRevision++;
  }

  /** CPU culling keeps geometry words fixed and updates only count/firstInstance lanes. */
  update(visible: readonly BundledSourceDraw[]): void {
    this.open();
    let previous = -1;
    // Validate before changing any command; repeated/split IDs cannot silently overwrite a slot.
    for (const draw of visible) {
      const index = draw.sourceDrawIndex,
        base = this.draws[index];
      if (
        !Number.isInteger(index) ||
        index <= previous ||
        !base ||
        !this.uint(draw.count) ||
        !this.uint(draw.firstInstance) ||
        draw.count > base.count ||
        draw.firstInstance + draw.count > this.instanceCapacity ||
        draw.indexCount !== base.indexCount ||
        draw.firstIndex !== base.firstIndex ||
        draw.baseVertex !== base.baseVertex ||
        draw.material !== base.material ||
        Boolean(draw.wind) !== Boolean(base.wind) ||
        draw.rasterOrientation !== base.rasterOrientation
      )
        throw new Error(
          "Visible source draws must retain their geometry, order, material, and wind identity.",
        );
      previous = index;
    }
    let rank = 0,
      changed = false,
      pendingCount = 0;
    for (let index = 0; index < this.draws.length; index++) {
      const draw =
        visible[rank]?.sourceDrawIndex === index ? visible[rank++] : undefined;
      const count = draw?.count ?? 0,
        first = draw?.firstInstance ?? 0;
      const offset = index * WORDS;
      if (count && !this.admitted[index])
        this.pendingIds[pendingCount++] = index;
      changed ||=
        this.words[offset + 1] !== count || this.words[offset + 4] !== first;
      this.words[offset + 1] = count;
      this.words[offset + 4] = first;
    }
    if ((changed || this.uploadPending) && this.words.byteLength) this.upload();
    // A failed upload cannot publish membership for commands the GPU has not received.
    this.publishMembership(pendingCount);
  }

  get(bindings: SourceBundleBindings): readonly GPURenderBundle[] {
    this.open();
    if (this.uploadPending)
      throw new Error(
        "Upload the complete source draw table before recording bundles.",
      );
    if (
      typeof bindings.key !== "string" ||
      !bindings.key ||
      bindings.key.length > 64
    )
      throw new Error("Choose a bounded source render bundle slot.");
    const resources: unknown[] = [
      bindings.frameGroup,
      bindings.shader,
      bindings.pipelineVersion,
      bindings.materials,
      bindings.vertices,
      bindings.instances,
      bindings.colors,
      bindings.uv1,
      bindings.wind,
      bindings.zeroWind,
      bindings.indices,
      Array.from(bindings.descriptor.colorFormats).join(","),
      bindings.descriptor.depthStencilFormat,
      bindings.descriptor.sampleCount ?? 1,
      bindings.descriptor.depthReadOnly ?? false,
      bindings.descriptor.stencilReadOnly ?? false,
    ];
    const revision = this.pruneNeverVisible ? this.membershipRevision : 0;
    const previous = this.bundles.get(bindings.key);
    const sameResources =
      previous &&
      resources.every((value, index) => value === previous.resources[index]);
    if (sameResources && previous.revision === revision) {
      this.reuses++;
      this.lastCommands = previous.commands;
      return previous.bundles;
    }
    if (!previous && this.bundles.size >= MAX_BUNDLES)
      throw new Error("The source view has too many render bundle slots.");
    // Publish the complete ordered result only after every changed page succeeds.
    const pages: (SourceBundlePage | undefined)[] = sameResources
      ? previous.pages.slice()
      : new Array(this.pageRevisions.length);
    const bundles: GPURenderBundle[] = [];
    const commands: SourceBundleCommands = {
      draws: 0,
      pipelines: 0,
      materials: 0,
      windBindings: 0,
    };
    for (let index = 0; index < pages.length; index++) {
      const pageRevision = this.pruneNeverVisible
        ? this.pageRevisions[index]
        : 0;
      if (this.pruneNeverVisible && !pageRevision) continue;
      let page = pages[index];
      if (!page || page.revision !== pageRevision)
        page = pages[index] = this.recordPage(bindings, index, pageRevision);
      else this.pageReuses++;
      if (page.bundle) bundles.push(page.bundle);
      commands.draws += page.commands.draws;
      commands.pipelines += page.commands.pipelines;
      commands.materials += page.commands.materials;
      commands.windBindings += page.commands.windBindings;
    }
    const result = Object.freeze(bundles);
    this.bundles.set(bindings.key, {
      resources,
      revision,
      pages,
      bundles: result,
      commands,
    });
    this.lastCommands = commands;
    this.slotUpdates++;
    return result;
  }

  private recordPage(
    bindings: SourceBundleBindings,
    page: number,
    revision: number,
  ): SourceBundlePage {
    const encoder = this.device.createRenderBundleEncoder(bindings.descriptor);
    encoder.setBindGroup(0, bindings.frameGroup);
    encoder.setVertexBuffer(0, bindings.vertices);
    encoder.setVertexBuffer(1, bindings.instances);
    encoder.setVertexBuffer(2, bindings.colors);
    encoder.setVertexBuffer(3, bindings.uv1);
    encoder.setIndexBuffer(bindings.indices, "uint32");
    const commands: SourceBundleCommands = {
      draws: 0,
      pipelines: 0,
      materials: 0,
      windBindings: 0,
    };
    let previousMaterial: GPUBindGroup | undefined;
    let previousPipeline: GPURenderPipeline | undefined,
      previousWind: GPUBuffer | undefined;
    const end = Math.min(
      this.draws.length,
      (page + 1) * SOURCE_BUNDLE_PAGE_DRAWS,
    );
    for (let index = page * SOURCE_BUNDLE_PAGE_DRAWS; index < end; index++) {
      const draw = this.draws[index];
      // Each page establishes all bindings before its first retained source draw.
      if (
        (this.pruneNeverVisible && !this.admitted[index]) ||
        !bindings.include(draw)
      )
        continue;
      const material = bindings.material(draw),
        pipeline = bindings.pipeline(draw);
      const wind = draw.wind ? bindings.wind : bindings.zeroWind;
      if (pipeline !== previousPipeline) {
        encoder.setPipeline(pipeline);
        previousPipeline = pipeline;
        commands.pipelines++;
      }
      if (material !== previousMaterial) {
        encoder.setBindGroup(1, material);
        previousMaterial = material;
        commands.materials++;
      }
      if (wind !== previousWind) {
        encoder.setVertexBuffer(4, wind);
        previousWind = wind;
        commands.windBindings++;
      }
      encoder.drawIndexedIndirect(this.indirect, index * WORDS * 4);
      commands.draws++;
    }
    const finished = encoder.finish({
      label: "Source bundle: " + bindings.key + ":" + page,
    });
    this.builds++;
    return {
      revision,
      bundle: commands.draws ? finished : undefined,
      commands,
    };
  }

  snapshot() {
    return {
      draws: this.draws.length,
      admittedDraws: this.admittedCount,
      membershipRevision: this.membershipRevision,
      policy: this.pruneNeverVisible ? "admitted-id-union" : "all-source-ids",
      cpuMembershipBytes:
        this.admitted.byteLength +
        this.pendingIds.byteLength +
        this.pageRevisions.byteLength,
      pageSize: SOURCE_BUNDLE_PAGE_DRAWS,
      pageCount: this.pageRevisions.length,
      pageRevisions: Array.from(this.pageRevisions),
      pageBundles: Array.from(this.bundles.values()).reduce(
        (sum, entry) => sum + entry.bundles.length,
        0,
      ),
      slots: Array.from(this.bundles, ([key, entry]) => ({
        key,
        revision: entry.revision,
        commands: { ...entry.commands },
        pages: entry.pages.filter(Boolean).length,
        bundles: entry.bundles.length,
      })),
      bytes: this.indirect.size,
      bundles: this.bundles.size,
      uploads: this.uploads,
      uploadBytes: this.uploadBytes,
      builds: this.builds,
      slotUpdates: this.slotUpdates,
      pageReuses: this.pageReuses,
      reuses: this.reuses,
    };
  }
  commandCounts(): Readonly<SourceBundleCommands> {
    return this.lastCommands;
  }
  commandSnapshot(): Uint32Array {
    this.open();
    return this.words.slice();
  }
  dispose(): void {
    if (this.closed) return;
    this.closed = true;
    this.bundles.clear();
    this.indirect.destroy();
    this.account(-this.indirect.size);
  }
}
