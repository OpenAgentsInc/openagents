# Releasing

1.  Make sure you have `make`, `git`, [`git-extras`](https://github.com/tj/git-extras), and [`gh`](https://cli.github.com/) (GitHub CLI) installed.
2.  Make sure you're authenticated with GitHub CLI: `gh auth login`
3.  Run `VERSION=X.Y.Z make release` (where X.Y.Z is the new version).

That's it! The release process will:

- Update the version in `lib/PostHog.php` and `composer.json`
- Create a changelog entry in `History.md` with a list of commits since last release
- Create and push a git tag
- Create a GitHub release with the changelog notes

Composer will pick up the new tag and you can see the latest version at https://packagist.org/packages/posthog/posthog-php.
