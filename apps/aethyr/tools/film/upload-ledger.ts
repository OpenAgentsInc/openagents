/** An acknowledged film frame can be retried without encoding it twice. */
export class FilmUploadLedger {
  next = 0;
  private held?: { index: number; repeat: boolean };
  private lastHash?: string;
  get busy() {
    return this.held !== undefined;
  }
  reserve(index: number) {
    if (this.busy) throw Error("The previous frame is still arriving.");
    if (
      !Number.isSafeInteger(index) ||
      index < 0 ||
      (index !== this.next && index !== this.next - 1)
    )
      throw Error("Upload the next film frame in order.");
    this.held = { index, repeat: index < this.next };
    return this.held.repeat;
  }
  verify(hash: string) {
    if (!this.held || !/^[a-f0-9]{64}$/.test(hash))
      throw Error("Upload a complete film frame.");
    if (this.held.repeat && hash !== this.lastHash)
      throw Error("A repeated film frame has different pixels.");
  }
  commit(hash: string) {
    this.verify(hash);
    if (!this.held!.repeat) {
      this.lastHash = hash;
      this.next++;
    }
  }
  release() {
    this.held = undefined;
  }
}
