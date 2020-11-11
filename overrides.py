#!/usr/bin/python3

import argparse
import json
import os

from typing import List

VALID_ARCHES = [
    "x86_64",
    "i686",
    "aarch64",
    "armv7hl",
    "ppc64le",
    "s390x",
]

VALID_RELEASES = [
    "rawhide",
    "33",
    "32",
    "31",
]


def main():
    parser = argparse.ArgumentParser()
    parser.set_defaults(action=None)

    path_parser = argparse.ArgumentParser(add_help=False)
    path_parser.add_argument(
        "--path",
        "-p",
        required=False,
        nargs="?",
        default="./overrides.json",
        help="override path to the 'overrides.json' file",
    )

    parsers = parser.add_subparsers()

    insert_parser = parsers.add_parser(
        "insert",
        parents=[path_parser],
        help="insert new value into the overrides file (may need resorting afterwards)",
    )
    insert_parser.set_defaults(action="insert")

    insert_parser.add_argument(
        "release",
        choices=(VALID_RELEASES + ["all"]),
        help="release to add override for",
    )
    insert_parser.add_argument(
        "arch",
        choices=(VALID_ARCHES + ["all"]),
        help="architecture to add override for",
    )
    insert_parser.add_argument(
        "dependency",
        help="dependency to ignore for specified packages",
    )
    insert_parser.add_argument(
        "packages",
        nargs="+",
        default=[],
        help="packages to override missing dependencies for (singleton all has the special meaning)",
    )

    sortit_parser = parsers.add_parser(
        "sortit",
        parents=[path_parser],
        help="sort overrides file for reproducible contents",
    )
    sortit_parser.set_defaults(action="sortit")

    validate_parser = parsers.add_parser(
        "validate",
        parents=[path_parser],
        help="validate contents of the overrides file for correct syntax",
    )
    validate_parser.set_defaults(action="validate")

    cli_args = vars(parser.parse_args())

    action = cli_args["action"]

    if action is None:
        print("No action specified.")
        return 1

    cli_path = cli_args["path"]

    if os.path.isabs(cli_path):
        path = cli_path
    else:
        path = os.path.abspath(cli_path)

    if action == "insert":
        release = cli_args["release"]
        arch = cli_args["arch"]
        dep = cli_args["dependency"]
        value = cli_args["packages"]

        return insert(path, release, arch, dep, value)

    if action == "sortit":
        return sortit(path)

    if action == "validate":
        return validate(path)


def insert(path: str, release: str, arch: str, dep: str, values: List[str]) -> int:
    with open(path) as file:
        overrides = json.loads(file.read())

    inserted = False

    current = overrides[release][arch]
    if dep in current.keys():
        if current[dep] == "all":
            print(" → 'all' override subsumes individual overrides, this has no effect")
        elif values == ["all"]:
            print(f" → upgrading to 'all' override for /{release}/{arch}/{dep}")
            current[dep] = "all"
            inserted = True
        else:
            print(f" → adding new values for /{release}/{arch}/{dep}")
            current[dep].extend(values)
            inserted = True
    else:
        if values == ["all"]:
            print(f" → adding 'all' override for /{release}/{arch}/{dep}")
            current[dep] = "all"
            inserted = True
        else:
            print(f" → adding {len(values)} overrides for /{release}/{arch}/{dep}")
            current[dep] = values
            inserted = True

    with open(path, "w") as file:
        file.write(json.dumps(overrides, indent=2, sort_keys=True))

    if inserted:
        sortit(path)

    return 0


def sortit(path: str):
    with open(path) as file:
        overrides = json.loads(file.read())

    for release, release_item in overrides.items():
        if not isinstance(release_item, dict):
            print(f" - /{release} does not contain a map of architectures.")
            print(type(release_item))
            break

        for arch, arch_item in release_item.items():
            if not isinstance(arch_item, dict):
                print(f" - /{release}/{arch} does not contain a map of dependencies.")
                break

            for dep, dep_item in arch_item.items():
                if isinstance(dep_item, list):
                    dep_item.sort()

    with open(path, "w") as file:
        file.write(json.dumps(overrides, indent=2, sort_keys=True))

    return 0


def validate(path: str):
    try:
        with open(path) as file:
            overrides = json.loads(file.read())
    except json.JSONDecodeError as e:
        print(" - File is not valid JSON")
        print(e)
        return 1

    if not isinstance(overrides, dict):
        print(" - Root element of JSON is not a map of releases.")
        return 1

    valid = True
    broad = []

    for release, release_item in overrides.items():
        if release != "all" and release not in VALID_RELEASES:
            print(f" - /{release} is not a valid value for a release.")
            valid = False

        if not isinstance(release_item, dict):
            print(f" - /{release} does not contain a map of architectures.")
            print(type(release_item))
            valid = False
            break

        for arch, arch_item in release_item.items():
            if arch != "all" and arch not in VALID_ARCHES:
                print(f" - /{release}/{arch} is not a valid value for an architecture.")
                valid = False

            if not isinstance(arch_item, dict):
                print(f" - /{release}/{arch} does not contain a map of dependencies.")
                valid = False
                break

            for dep, dep_item in arch_item.items():
                if isinstance(dep_item, str):
                    if dep_item == "all":
                        broad.append(f"/{release}/{arch}/{dep}")
                    else:
                        print(f" - /{release}/{arch}/{dep} contains invalid string '{dep_item}'.")
                        valid = False

                elif isinstance(dep_item, list):
                    for package in dep_item:
                        if not isinstance(package, str):
                            print(f" - /{release}/{arch}/{dep} contains invalid element '{package}'.")
                            valid = False

                else:
                    print(f" - /{release}/{arch}/{dep} has invalid type '{type(dep_item)}'.")

    if broad:
        print("Overrides file contains broad exceptions.")
        print("Verify these manually and add individual overrides, if possible.")
        for item in broad:
            print(" -", item)

    if valid:
        print("File valid.")
        return 0
    else:
        return 1


if __name__ == "__main__":
    try:
        exit(main())
    except KeyboardInterrupt:
        print("Cancelled by SIGINT.")
        exit(0)
