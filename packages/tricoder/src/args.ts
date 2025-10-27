export function buildTunnelArgs(localPort: number, to: string): string[] {
  return [
    "run",
    "-q",
    "-p",
    "oa-tunnel",
    "--",
    "--to",
    String(to),
    "--local-port",
    String(localPort),
  ];
}

