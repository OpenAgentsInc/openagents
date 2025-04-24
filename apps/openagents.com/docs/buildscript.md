# Laravel Cloud Monorepo Build Script

This script is adapted for the OpenAgents monorepo structure to deploy the `apps/openagents.com` application to Laravel Cloud.
See: https://cloud.laravel.com/docs/knowledge-base/monorepo-support

```bash
#!/bin/bash
# ---------------------------------
# Deploying the "apps/openagents.com" directory from monorepo
# ---------------------------------

# Step 1: Create a temporary directory
mkdir /tmp/monorepo_tmp

# Step 2: Move the openagents.com app to the temporary directory
if [ -d "apps/openagents.com" ]; then
  cp -Rf apps/openagents.com /tmp/monorepo_tmp/
fi

# Step 3: Clean the current directory
rm -rf * .[^.]*

# Step 4: Move the openagents.com app contents into root
cp -Rf /tmp/monorepo_tmp/openagents.com/{.,}* . 2>/dev/null || :

# Step 5: Remove the temporary directory
rm -rf /tmp/monorepo_tmp

# Step 6: Proceed with build steps
composer install --no-dev
npm install
npm run build
```

Make sure to test this script in a safe environment before using it in production.