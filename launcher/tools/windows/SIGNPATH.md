# Signing the Windows build with SignPath

The Windows exe is signed through the [SignPath Foundation](https://signpath.org) programme for open source
projects. The rules the project follows are in the README's
[Code signing policy](../../../README.md#code-signing-policy).

## How a build gets signed

- **A release** (a pushed tag `vX.Y.Z`, `launcher/README.md` "Releasing a version") is signed whenever the
  SignPath secret and variable below are set; without them it is published unsigned.
- **A test build** is signed only when the `launcher` workflow is run by hand (Actions → launcher → Run
  workflow) with **sign** ticked; that run fails if SignPath isn't set up.

Either way:

1. The Windows job tests the exe as usual, then uploads it and submits a signing request to SignPath.
2. An approver confirms the request on signpath.io. The job waits up to an hour.
3. The job checks the signature and packs the signed exe into the zip and into the installer
   (`installer.iss`).
4. The installer goes through a second request (the `installer` artifact configuration), which an approver
   confirms the same way, so a signed build asks for **two approvals**. The job checks that signature too.
5. The publish job puts both in the release (or replaces the `launcher-latest` assets). The release notes and
   `VERSION.txt` say whether the build is signed.

Pushes to `launcher` still build unsigned test zips, so a push after a signed run replaces the signed test zip
with an unsigned one. Releases keep theirs.

## Setup, once

- **On signpath.io**, after the project is accepted:
  - Link the GitHub repository through the GitHub Actions trusted build system.
  - Create the project with the slug `speedrunning-launcher`.
  - Create its artifact configuration from
    [`signpath-artifact-configuration.xml`](signpath-artifact-configuration.xml) (slug `initial`).
  - Create a second artifact configuration, for the installer, from
    [`signpath-installer-artifact-configuration.xml`](signpath-installer-artifact-configuration.xml) (slug
    `installer`). Without it every signed run fails at the installer's signing request.
  - Use the `release-signing` signing policy. Every request needs an approver.
  - Create a CI user with submitter rights on that policy and generate its API token.
- **In the GitHub repository**, under Settings → Secrets and variables → Actions:
  - Secret `SIGNPATH_API_TOKEN`: the CI user's API token.
  - Variable `SIGNPATH_ORGANIZATION_ID`: the organization ID from signpath.io.
  - If the slugs differ from the defaults above, set the variables `SIGNPATH_PROJECT_SLUG`,
    `SIGNPATH_POLICY_SLUG`, `SIGNPATH_ARTIFACT_CONFIGURATION_SLUG` and
    `SIGNPATH_INSTALLER_ARTIFACT_CONFIGURATION_SLUG`.

Every team member uses multi-factor authentication on GitHub and on SignPath.
