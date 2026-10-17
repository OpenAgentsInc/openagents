# Accept the previous release's environment names for one release.
# Source this file from Bash before reading app settings.
while IFS= read -r openagents_legacy_name; do
  openagents_new_name="OPENAGENTS_${openagents_legacy_name#ZERON_}"
  if ! declare -p "$openagents_new_name" >/dev/null 2>&1; then
    export "$openagents_new_name=${!openagents_legacy_name}"
  fi
done < <(compgen -e | LC_ALL=C sed -n '/^ZERON_[A-Za-z0-9_]*$/p')
unset openagents_legacy_name openagents_new_name
