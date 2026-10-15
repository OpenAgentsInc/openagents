#!/bin/sh
# Runs a Python script inside Unreal Editor headless, for Draconia's
# MetaHuman authoring (docs/verse/draconia/metahuman-study.md).
#
#   run_ue.sh SCRIPT.py [ARGS...]
#
# The engine is the newest finished install under /Users/Shared/Epic Games/
# (UE_5.8 once its editor binary exists, else UE_5.7), or $UE_ROOT. The
# project is private and lives outside the repository ($DRACONIA_UE_PROJECT,
# default ~/work/film-out/draconia/ue/DraconiaMH/DraconiaMH.uproject); it is
# created on first use. Everything it makes is a MetaHuman asset: private,
# never committed, and never used to train AI models.
set -eu
script=$(cd "$(dirname "$1")" && pwd)/$(basename "$1"); shift
if [ -z "${UE_ROOT:-}" ]; then
  # Newest minor version whose editor binary exists (a finished install).
  UE_ROOT=$(ls -d "/Users/Shared/Epic Games"/UE_5.* 2>/dev/null | sort -t. -k2 -n -r | while IFS= read -r v; do
    # Skip an install the launcher is still writing (files changed in the
    # last 15 minutes).
    if [ -x "$v/Engine/Binaries/Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor" ] \
      && [ -z "$(find "$v/Engine/Binaries/Mac" -maxdepth 1 -mmin -15 2>/dev/null | head -1)" ]; then echo "$v"; break; fi
  done)
fi
[ -n "$UE_ROOT" ] || { echo "No Unreal Engine 5 install found" >&2; exit 1; }
editor="$UE_ROOT/Engine/Binaries/Mac/UnrealEditor.app/Contents/MacOS/UnrealEditor"
version=$(basename "$UE_ROOT" | sed 's/UE_//')
project=${DRACONIA_UE_PROJECT:-$HOME/work/film-out/draconia/ue/DraconiaMH/DraconiaMH.uproject}
if [ ! -f "$project" ]; then
  mkdir -p "$(dirname "$project")/Content"
  cat > "$project" <<JSON
{
  "FileVersion": 3,
  "EngineAssociation": "$version",
  "Description": "Private: Draconia MetaHuman authoring. Never committed; never used to train AI models.",
  "Plugins": [
    {"Name": "PythonScriptPlugin", "Enabled": true},
    {"Name": "EditorScriptingUtilities", "Enabled": true},
    {"Name": "MetaHumanCharacter", "Enabled": true},
    {"Name": "MetaHumanSDK", "Enabled": true},
    {"Name": "MetaHumanCoreTech", "Enabled": true},
    {"Name": "MetaHuman", "Enabled": true},
    {"Name": "HairStrands", "Enabled": true},
    {"Name": "MovieRenderPipeline", "Enabled": true}
  ]
}
JSON
fi
echo "UE $version: $editor" >&2
# Headless runs never launch CrashReportClient: on macOS an unattended
# editor only skips it when the project opts out of unattended bug reports.
# Each crash or ensure otherwise starts a reporter that compresses,
# symbolicates and uploads the report (Epic's servers), and a run of
# crashes leaves dozens of them burning CPU.
settings="$(dirname "$project")/Config/DefaultEditorSettings.ini"
mkdir -p "$(dirname "$settings")"
if ! grep -q "bSendUnattendedBugReports=False" "$settings" 2>/dev/null; then
  printf '\n[/Script/UnrealEd.CrashReportsPrivacySettings]\nbSendUnattendedBugReports=False\n\n[/Script/UnrealEd.AnalyticsPrivacySettings]\nbSendUsageData=False\n' >> "$settings"
fi
if [ -n "${DRACONIA_UE_KEEP:-}" ]; then
  # A script that keeps working on later ticks (render_reference.py) and
  # quits the editor itself: -ExecutePythonScript closes the editor as soon
  # as the script returns, so run it as the project's startup script for
  # this one launch instead.
  ini="$(dirname "$project")/Config/DefaultEngine.ini"
  mkdir -p "$(dirname "$ini")"
  touch "$ini"
  cp "$ini" "$ini.draconia-bak"
  printf '\n[/Script/PythonScriptPlugin.PythonScriptPluginSettings]\n+StartupScripts=%s\n' "$script" >> "$ini"
  status=0
  DRACONIA_ARGS="$*" "$editor" "$project" -unattended -nocrashreports -nosplash -stdout -AllowStdOutLogVerbosity || status=$?
  mv "$ini.draconia-bak" "$ini"
  exit $status
fi
DRACONIA_ARGS="$*" exec "$editor" "$project" -ExecutePythonScript="$script" -unattended -nocrashreports -nosplash -stdout -AllowStdOutLogVerbosity
