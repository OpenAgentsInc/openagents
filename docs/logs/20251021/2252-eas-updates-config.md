```bash
➜  expo git:(main) ✗ eas update:configure
💡 The following process will configure your project to use EAS Update. These changes only apply to your local project files and you can safely revert them at any time.
> npx expo install expo-updates
[expo-cli] › Installing 1 SDK 54.0.0 compatible native module using bun
[expo-cli] > bun add expo-updates@~29.0.12
[expo-cli] bun add v1.3.0 (b0a6feca)
[expo-cli] Resolving dependencies
[expo-cli] Resolved, downloaded and extracted [35]
[expo-cli] Saved lockfile
[expo-cli] installed expo-updates@29.0.12 with binaries:
[expo-cli]  - expo-updates
[expo-cli]
[expo-cli] 15 packages installed [936.00ms]
✔ Installed expo-updates
✔ Configured updates.url to "https://u.expo.dev/39df2bde-2c42-49ec-9cef-1567231dfe38"
✔ Configured runtimeVersion for Android and iOS with "{"policy":"appVersion"}"

All builds of your app going forward will be eligible to receive updates published with EAS Update.

✔ Configured eas.json.

🎉 Your app is configured to use EAS Update!

- Learn more about other capabilities of EAS Update: https://docs.expo.dev/eas-update/introduction/
```
