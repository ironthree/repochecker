# repochecker

This repository contains the source and default configuration files for the `repochecker` service. It can read yum/dnf
repositories (primarily targeted at fedora), analyze broken dependencies, remove false positives, take overrides from a
configuration file into account, and serve the resulting statistics as JSON files and via a simple HTTP API.

It is the successor of [fedora-health-check](https://pagure.io/fedora-health-check), which was written in Python, was
slow, and was written as a one-shot program instead of a service, with results committed to a git repository instead of
served via an API.

## dependencies

The service relies on dnf/yum repositories that are available on the system (though they need not be enabled by
default), so by default, it requires `dnf`, `dnf-utils`, `fedora-repos`, and `fedora-repos-rawhide`.

## overrides

It's possible to provide overrides / an "allowlist" for filtering out false positives that are not really broken
dependencies. The current configuration is shipped as structured JSON data in the `overrides.json` file in the project
root. It's possible to filter out broken dependencies per release (or for all releases), per architecture (or for all
architectures), and either for all packages with a specific false positive, or only for a specified list of packages.

The `overrides.py` script serves as a utility for editing, validating, and consistently sorting and formatting the JSON
overrides file.  

## configuration

The default configuration is shipped in the `repochecker.toml` file in the project root. This is where releases are
added after the branch point, old releases are removed after they reach their EOL, and where release type can be
switched from `prerelease` to `stable` after a fedora release reaches GA. The refresh interval for repository data and
package maintainers can also be configured (in number of hours).

## deployment

An example systemd unit file is provided in the `etc` directory. By default, `repochecker` will check the following
locations for an existing configuration file and overrides, in this order, and will use the first one it finds:

- current working directory (as returned by `std::env::current_dir()`)
- `/etc/repochecker/repochecker.{toml,json}`
- `/usr/share/repochecker/repochecker.{toml,json}`

## limitations

Data served via HTTP endpoints by `repochecker` is provided on a best-effort basis. Limitations of the underlying data
sources and/or technologies make providing 100% correct data almost impossible.

- architecture-dependent RPM source packages (SRPMs)

While it is forbidden by policy to have a package produce RPM source package **contents** that differ depending on the
environment (including the architecture that the SRPM is generated on), the RPM **headers** (source package metadata)
can - and will - be different. For example, BuildRequires that are only present on specific architectures might or
might not be present in RPM headers, depending on the host system the SRPM is built on.

- architecture-specific source packages discarded by koji

For "normal" builds in the Fedora koji instance, the SRPM for the build is built from dist-git, and then distributed to
builders of the different architectures. There, they are built in mock, which also produces a rebuilt SRPM file that
has RPM headers that are specific to that architecture. However, only the initial SRPM file is collected, and those
built on different builder architectures are discarded. The machine that runs the `buildSRPMfromSCM` task for generating
the initial SRPM file is selected by the koji scheduler, so its architecture is, essentially, [randomized].

[randomized]: https://pagure.io/koji/issue/2726

- Fedora infrastructure treats SRPM packages as architecture-independent

Since only one SRPM file (which might or might not have architecture-dependent RPM headers) is collected by koji, there
is also only one `-source` repository produced for all architectures. Querying this repository hence yields unreliable
results - in particular, `BuildRequires` might be present on architectures even if they should explicitly not be present
for that architecture according to the `.spec` file contained in the SRPM file.

It is possible to add overrides for "false positives" that are caused by these architectural limitations, as described
above.

- compose process has issues with architecture-specific `noarch` packages

Packages that are contain no architecture-specific files (`noarch`) but have dependencies that are not available on all
architectures get [erroneously] included in repositories that are explicitly excluded by the package's `.spec` file.

These are not really "false positives" in the same sense, because the packages really do have broken dependencies on
some architectures, but since this is due to infrastructure bugs, they can be added to the overrides as well. 

[erroneously]: https://pagure.io/koji/issue/1843

- incomplete package maintainer information from pagure

Pagure provides a "special" API endpoint for getting all packages and their maintainers in one request, but this only
includes the package's main admin ("owner") and any BugZilla assignee overrides that might be present for each
package. However, co-maintainers are not returned by this endpoint at all. Querying the pagure API for almost 30000
individual packages just to get information for all co-maintainers seems unreasonable, so `repochecker` can only
associate packages with their "main admin".

## license

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>) or
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>),

at your option.

### contributions

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as
defined in the Apache-2.0 license, shall be dual licensed as stated above, without any additional terms or conditions.

