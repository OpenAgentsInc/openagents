/** Preserve a press/release pair that arrives between two simulation frames. */
export class JumpInput {
  private held = false;
  private pressed = false;
  update(held: boolean) {
    this.pressed ||= held && !this.held;
    this.held = held;
  }
  sample() {
    const jump = this.held || this.pressed;
    this.pressed = false;
    return jump;
  }
  reset() {
    this.held = false;
    this.pressed = false;
  }
}
