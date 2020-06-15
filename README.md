# repochecker

This repository contains the source and default configuration files for the `repochecker` service. It can read yum/dnf
repositories (primarilty targeted at fedora), analyze broken dependencies, remove false positives, take overrides from a
configuration file into account, and serve the resulting statistics as JSON files and via a simple HTTP API.

It is intended to be the replacement for [fedora-health-check](https://pagure.io/fedora-health-check), which is written
in python, is a bit slow, and is written as a one-shot program instead of a service, where results are committed to git
instead of served via an API.


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

