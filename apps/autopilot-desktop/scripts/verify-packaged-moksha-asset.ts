import { existsSync, statSync } from "node:fs"

const mokshaAssetSuffix =
  "views/autopilot-desktop/assets/moksha/diamond.glb"

const candidateAssetPaths = [
  `build/dev-macos-arm64/Autopilot-dev.app/Contents/Resources/app/${mokshaAssetSuffix}`,
  `build/dev-macos-x64/Autopilot-dev.app/Contents/Resources/app/${mokshaAssetSuffix}`,
  `build/dev-linux-x64/Autopilot-dev/Resources/app/${mokshaAssetSuffix}`,
  `build/dev-linux-arm64/Autopilot-dev/Resources/app/${mokshaAssetSuffix}`,
  `build/dev-windows-x64/Autopilot-dev/resources/app/${mokshaAssetSuffix}`,
  `build/dev-windows-arm64/Autopilot-dev/resources/app/${mokshaAssetSuffix}`,
]

const packagedAssetPath = candidateAssetPaths.find(path => {
  if (!existsSync(path)) {
    return false
  }

  return statSync(path).size > 0
})

if (packagedAssetPath === undefined) {
  console.error(
    [
      "Packaged Moksha diamond asset is missing.",
      "Checked:",
      ...candidateAssetPaths.map(path => `- ${path}`),
    ].join("\n"),
  )
  process.exit(1)
}

console.log(`Packaged Moksha diamond asset verified: ${packagedAssetPath}`)
