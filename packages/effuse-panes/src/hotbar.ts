// Port of the Hotbar data model from `crates/wgpui/src/components/hud/hotbar.rs`.

export type HotbarSlot = Readonly<{
  slot: number;
  icon: string;
  title: string;
  active: boolean;
  ghost: boolean;
}>;

export const hotbarSlot = (slot: number, icon: string, title: string): HotbarSlot => ({
  slot,
  icon,
  title,
  active: false,
  ghost: false,
});

export class HotbarModel {
  private itemsInternal: HotbarSlot[] = [];
  private flashIndex: number | undefined;
  private flashStartedAt: number | undefined;
  private readonly flashDurationMs = 90;
  private pendingClicks: number[] = [];

  items(): ReadonlyArray<HotbarSlot> {
    return this.itemsInternal;
  }

  setItems(items: ReadonlyArray<HotbarSlot>): void {
    this.itemsInternal = [...items];
  }

  takeClickedSlots(): number[] {
    const out = this.pendingClicks;
    this.pendingClicks = [];
    return out;
  }

  clickSlot(slot: number): void {
    this.pendingClicks.push(slot);
  }

  flashSlot(slot: number): void {
    const index = this.itemsInternal.findIndex((i) => i.slot === slot);
    if (index < 0) return;
    this.flashIndex = index;
    this.flashStartedAt = Date.now();
  }

  isFlashing(): boolean {
    this.clearExpiredFlash();
    return this.flashIndex !== undefined;
  }

  isSlotFlashing(slot: number): boolean {
    this.clearExpiredFlash();
    const index = this.itemsInternal.findIndex((i) => i.slot === slot);
    return index >= 0 && this.flashIndex === index;
  }

  private clearExpiredFlash(): void {
    if (this.flashStartedAt === undefined) return;
    if (Date.now() - this.flashStartedAt >= this.flashDurationMs) {
      this.flashIndex = undefined;
      this.flashStartedAt = undefined;
    }
  }
}

