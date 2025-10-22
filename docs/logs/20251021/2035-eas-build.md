```bash
➜  expo git:(main) ✗ bun run build:ios:prod
$ eas build --profile production --platform ios
✔ Generated eas.json. Learn more: https://docs.expo.dev/build-reference/eas-json/
Resolved "production" environment for the build. Learn more: https://docs.expo.dev/eas/environment-variables/#setting-the-environment-for-your-builds
No environment variables with visibility "Plain text" and "Sensitive" found for the "production" environment on EAS.

✔ iOS app only uses standard/exempt encryption? Learn more: https://developer.apple.com/documentation/Security/complying-with-encryption-export-regulations … yes
No remote versions are configured for this project, buildNumber will be initialized based on the value from the local project.
✔ Initialized buildNumber with 1.
✔ Using remote iOS credentials (Expo server)

If you provide your Apple account credentials we will be able to generate all necessary build credentials and fully validate them.
This is optional, but without Apple account access you will need to provide all the missing values manually and we can only run minimal validation on them.
✔ Do you want to log in to your Apple account? … yes

› Log in to your Apple Developer account to continue
✔ Apple ID: … chris@openagents.com
› Restoring session
› Team OpenAgents, Inc.
› Provider OpenAgents, Inc.
✔ Logged in Local session
✔ Bundle identifier registered com.openagents.app
✔ Synced capabilities: No updates
✔ Synced capability identifiers: No updates
✔ Fetched Apple distribution certificates
✔ Generate a new Apple Distribution Certificate? … yes
✔ Created Apple distribution certificate
✔ Created distribution certificate
✔ Generate a new Apple Provisioning Profile? … yes
✔ Created Apple provisioning profile
✔ Created provisioning profile

Project Credentials Configuration

Project                   @openagents-org/openagents
Bundle Identifier         com.openagents.app

App Store Configuration

Distribution Certificate
Serial Number             71CB5F1D71212BFC62904589EF9E29BC
Expiration Date           Wed, 21 Oct 2026 20:25:33 CDT
Apple Team                OpenAgents, Inc. (Company/Organization)
Updated                   4 seconds ago

Provisioning Profile
Status                    active
Expiration                Wed, 21 Oct 2026 20:25:33 CDT
Apple Team                OpenAgents, Inc. (Company/Organization)
Updated                   1 second ago

All credentials are ready to build @openagents-org/openagents (com.openagents.app)


Compressing project files and uploading to EAS Build. Learn more: https://expo.fyi/eas-build-archive
✔ Uploaded to EAS
✔ Computed project fingerprint

See logs: https://expo.dev/accounts/openagents-org/projects/openagents/builds/dc4c7bf2-3dcc-4eff-9ca7-6cac395fcac7

Waiting for build to complete. You can press Ctrl+C to exit.
⠹ Build in progress...
```
