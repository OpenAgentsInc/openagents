export type LiveState = { assistant: string; thought?: string };

export class LiveAggregator {
  private state: LiveState = { assistant: '' };

  clear() {
    this.state = { assistant: '' };
  }

  update(kind: 'assistant' | 'reason', text: string) {
    if (kind === 'assistant') this.state.assistant = text;
    else if (kind === 'reason') this.state.thought = text;
  }

  snapshot(): LiveState {
    return { ...this.state };
  }
}
