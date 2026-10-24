/** Operator-selected loopback ports. World content cannot change this pair. */
export function devOrigins(environment = process.env) {
  const profile = environment.VITE_WORLD_PROFILE;
  if (
    profile !== undefined &&
    ![
      "medieval-town",
      "haunted-house",
      "medieval-overview",
      "starter",
      "open-ground",
    ].includes(profile)
  )
    throw new Error("Choose a supported deployment world profile.");
  const overview = profile === "medieval-overview";
  const ground = profile === "open-ground";
  const port = (name, fallback) => {
    const input = environment[name];
    if (input === undefined) return fallback;
    if (!/^[0-9]{4,5}$/.test(input))
      throw new Error(`${name} must be a loopback port from 1024 to 65535.`);
    const value = Number(input);
    if (value < 1024 || value > 65535)
      throw new Error(`${name} must be a loopback port from 1024 to 65535.`);
    return value;
  };
  const editorPort = port(
    "AETHYRFORGE_EDITOR_PORT",
    ground ? 55316 : overview ? 55306 : 55304,
  );
  const previewPort = port(
    "AETHYRFORGE_PREVIEW_PORT",
    ground ? 55317 : overview ? 55307 : 55305,
  );
  if (editorPort === previewPort)
    throw new Error("The editor and preview need different loopback ports.");
  return {
    editorPort,
    previewPort,
    editorOrigin: `http://127.0.0.1:${editorPort}`,
    previewOrigin: `http://127.0.0.1:${previewPort}`,
  };
}
