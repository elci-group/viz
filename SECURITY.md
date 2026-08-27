# Security policy

## Trust boundaries

`viz inspect` and `viz generate` only read project metadata and write the requested
tape. They never execute project code. This matters because build files, package
scripts, tests, and binaries in an untrusted checkout are executable content.

`viz check` invokes `vhs validate`, which parses but does not intentionally execute
the tape. Treat parser vulnerabilities as part of the installed VHS threat model.

`viz render` executes the tape through VHS. Every `Type` and `Enter` sequence can
run with the caller's permissions. Read the tape first and use normal sandboxing
when rendering an untrusted project.

Viz starts VHS with a direct process argument vector. It does not interpolate tape
paths into a shell command. Generated `Type` strings escape quotes and backslashes,
and multiline commands are refused.

The `vhs` executable is resolved through the caller's `PATH`, following normal CLI
conventions. Use a trusted PATH and trusted VHS installation. Viz validates a tape
with VHS before starting the separate rendering process; callers should still avoid
concurrent modification of a tape between those operations.

## Reporting

Report vulnerabilities privately to the repository maintainers. Include the Viz
version, operating system, reproduction tape or project fixture, and the observed
impact. Do not include secrets or private repository contents.

Supported security fixes target the latest release candidate and subsequent
stable releases.
