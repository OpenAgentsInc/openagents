# Build UE 5.9 from source (macOS)

## Requirements

- Access to `EpicGames/UnrealEngine` on GitHub. Your GitHub account must be
  linked to an Epic account. `git ls-remote` on the repo succeeds for the
  owner's `AtlantisPleb` login.
- Xcode. This Mac has Xcode 26.6.
- Free disk: about 200 GB on `/System/Volumes/Data`. Measured on 2026-11-11:
  - shallow clone of the `5.9` branch: 3.9 GB
  - `Setup.sh` dependency download: 31.8 GB
  - build output: grows to roughly 100 GB or more

  Free space was 234 GB when the build started and fell to 152 GB at step
  3,928 of 6,121.

## Uninstall 5.7 and 5.8

The Epic Games Launcher keeps installs in `/Users/Shared/Epic Games/UE_<version>`
and lists them in
`~/Library/Application Support/Epic/UnrealEngineLauncher/LauncherInstalled.dat`.

1. Quit any running `UnrealEditor`.
2. Delete the `UE_5.8` folder (49 GB). Use the literal path, never a variable.
3. Remove its entries from `LauncherInstalled.dat`. Restart the Launcher to
   refresh its list.

Done on 2026-11-11 for 5.8. 5.7 was not installed. Only config remains in
`~/Library/Application Support/Epic/UnrealEngine/5.7` and `.../5.8`.

Projects that pointed at 5.8 (Draconia, Mayor, the AethyrForge fixtures) have no
engine to open with until 5.9 is built.

## Steps

```sh
mkdir -p ~/UnrealEngine-5.9
cd ~
git clone --depth 1 --branch 5.9 https://github.com/EpicGames/UnrealEngine.git UnrealEngine-5.9/src
cd UnrealEngine-5.9/src
./Setup.sh --force            # downloads ~31.8 GB of dependencies
./GenerateProjectFiles.sh
./Engine/Build/BatchFiles/RunUBT.sh UnrealEditor Mac Development -Progress
```

Notes:

- `--depth 1` keeps the clone small. Run `git fetch --unshallow` if you need
  history.
- The build has 6,121 steps and takes a few hours.
- Log the build to a file (`> ../build.log 2>&1`) and follow it with
  `tail -f`.
- `GenerateProjectFiles.sh` writes `UE5 (IOS).xcworkspace`; there is no plain
  `UE5.xcworkspace` on this checkout.

## Run the editor

After the build succeeds, launch the editor from
`Engine/Binaries/Mac/UnrealEditor.app`.

## Branches

`5.7`, `5.8`, `5.9`, `dev-5.7`, `dev-5.8` and `dev-5.9` exist on the remote.
The `5.9` branch is the MegaJam Preview. `dev-5.9` is the development branch.
