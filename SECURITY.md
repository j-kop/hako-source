# Security policy

## Supported versions

Only the [latest release](https://github.com/j-kop/hako-source/releases/latest) gets security fixes. Update with `hako upgrade`.

## Reporting a vulnerability

Please **don't open a public issue**. Report it privately instead: go to this repository's **Security** tab and click **Report a vulnerability**. Only the maintainer can see these reports.

Please include:
- the hako version (`hako --version`) and your platform;
- what an attacker could do, and the steps to reproduce it;
- a malicious manifest, archive or registry that triggers it, if you have one.

Examples of what counts as a vulnerability:
- installing a file whose checksum doesn't match;
- writing files outside `~/.hako`, e.g. through crafted archive paths or symlinks;
- running code during install;
- overwriting executables that belong to another package.

This is a hobby project, so there's no guaranteed response time. Reports are read and taken seriously.
