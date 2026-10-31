# Signing the Windows build with SignPath

The Windows exe is signed through the [SignPath Foundation](https://signpath.org) programme for open source
projects. The rules the project follows are in the README's
[Code signing policy](../../../README.md#code-signing-policy).

## How a build gets signed

1. Run the `launcher` workflow by hand (Actions → launcher → Run workflow) with **sign** ticked.
2. The Windows job tests the exe as usual, then uploads it and submits a signing request to SignPath.
3. An approver confirms the request on signpath.io. The job waits up to an hour.
4. The job checks the signature, packs the signed exe into the zip and the publish job replaces the
   `launcher-latest` asset. The release notes and `VERSION.txt` say whether the build is signed.

Pushes to `launcher` still build unsigned test zips, so a push after a signed run replaces the signed zip with
an unsigned one.

## Setup, once

- **On signpath.io**, after the project is accepted:
  - Link the GitHub repository through the GitHub Actions trusted build system.
  - Create the project with the slug `speedrunning-launcher`.
  - Create its artifact configuration from
    [`signpath-artifact-configuration.xml`](signpath-artifact-configuration.xml) (slug `initial`).
  - Use the `release-signing` signing policy. Every request needs an approver.
  - Create a CI user with submitter rights on that policy and generate its API token.
- **In the GitHub repository**, under Settings → Secrets and variables → Actions:
  - Secret `SIGNPATH_API_TOKEN`: the CI user's API token.
  - Variable `SIGNPATH_ORGANIZATION_ID`: the organization ID from signpath.io.
  - If the slugs differ from the defaults above, set the variables `SIGNPATH_PROJECT_SLUG`,
    `SIGNPATH_POLICY_SLUG` and `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG`.

Every team member uses multi-factor authentication on GitHub and on SignPath.
