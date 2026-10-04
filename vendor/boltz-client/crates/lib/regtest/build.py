#!/usr/bin/env python3
"""Script for building and pushing the Boltz Docker images."""

import os
import sys
from argparse import ArgumentParser
from dataclasses import dataclass
from os import chdir, system
from pathlib import Path


@dataclass
class BuildArgument:
    """Argument of the "docker build" command."""

    name: str
    value: str


@dataclass
class Image:
    """The tags and build arguments of a Docker image."""

    tag: str
    arguments: list[BuildArgument]


def load_env_file(env_path: Path) -> dict[str, str]:
    """Load KEY=VALUE pairs from a .env file, ignoring comments and blanks."""
    env: dict[str, str] = {}
    if not env_path.exists():
        return env

    try:
        with env_path.open("r") as f:
            for raw_line in f:
                line = raw_line.strip()
                if not line or line.startswith("#"):
                    continue
                if "=" not in line:
                    continue
                key, value = line.split("=", 1)
                key = key.strip()
                value = value.strip()
                # Strip optional surrounding quotes
                if (value.startswith('"') and value.endswith('"')) or (
                    value.startswith("'") and value.endswith("'")
                ):
                    value = value[1:-1]
                env[key] = value
    except Exception:
        # Fail silently and return what we have to avoid breaking builds
        pass

    return env


UBUNTU_VERSION = BuildArgument(
    name="UBUNTU_VERSION",
    value="24.04",
)

NODE_VERSION = BuildArgument(
    name="NODE_VERSION",
    value="lts-trixie",
)

NGINX_VERSION = BuildArgument(
    name="NGINX_VERSION",
    value="alpine",
)

CLN_VERSION = "26.06.1"
ELECTRS_VERSION = "new-index-6d182d"

IMAGES: dict[str, Image] = {
    "electrs": Image(
        tag=ELECTRS_VERSION,
        arguments=[
            UBUNTU_VERSION,
        ],
    ),
    "esplora": Image(
        tag="latest",
        arguments=[
            NODE_VERSION,
            NGINX_VERSION,
        ],
    ),
    "foundry": Image(
        tag="latest",
        arguments=[
            UBUNTU_VERSION,
        ],
    ),
    "scripts": Image(
        tag="latest",
        arguments=[
            UBUNTU_VERSION,
            BuildArgument(
                name="ARKD_IMAGE",
                value=load_env_file(Path(__file__).parent / ".env").get("ARKD_IMAGE"),
            ),
        ],
    ),
    "c-lightning-plugins": Image(
        tag=CLN_VERSION,
        arguments=[
            UBUNTU_VERSION,
            BuildArgument(
                name="HOLD_VERSION",
                value="v0.3.3",
            ),
            BuildArgument(
                name="CLNURL_VERSION",
                value="v0.1.0",
            ),
        ],
    ),
}


def print_step(message: str) -> None:
    """Print green text and is used to log the step of a process."""
    print(f"\033[0;32m{message}\033[0;0m")


def print_error(message: str) -> None:
    """Print red text and is used to report errors."""
    print(f"\033[1;31m{message}\033[0;0m")


def change_working_directory() -> None:
    """Change the working directory to the one this script is located in."""
    directory = Path(__file__).parent
    chdir(directory)


def get_build_details(image: str) -> Image:
    """Get the build details of an image or exits the script if they can't be found."""
    build_details = IMAGES.get(image)

    if not build_details:
        print_error(f"Could not find image {image}")
        sys.exit(1)

    return build_details


def list_images(to_list: list[str]) -> None:
    """List the version and build arguments of either one or all images."""
    print("Images:")

    for image in to_list:
        build_details = get_build_details(image)

        print(f"  - {image}")
        print(f"    Tag: {build_details.tag}")

        print()

        if build_details.arguments:
            print("    Build arguments:")
        for argument in build_details.arguments:
            print(f"      - {argument.name}={argument.value}")

        print()


def build_images(
    to_build: list[str],
    organisation: str,
    no_cache: bool,
    no_latest: bool,
    branch: str,
    buildx: bool,
    platform: str = "",
) -> None:
    """Build one or more images."""

    for image in to_build:
        change_working_directory()
        build_details = get_build_details(image)
        tag = build_details.tag

        if branch in ["master", "main"]:
            tag = "latest"
        elif branch != "":
            tag = branch

        build_args = [f"{arg.name}={arg.value}" for arg in build_details.arguments]

        # Add the prefix "--build-arg " to every entry and
        # join the array to a string
        args = " ".join(["--build-arg " + entry for entry in build_args])

        name = f"{organisation}/{image}"
        dockerfile = f"images/{image}/Dockerfile"

        if buildx:
            extra_tag = "" if no_latest else f"--tag {name}:latest"
            command = (
                f"docker buildx build --push {args} --platform {platform} "
                f"--tag {name}:{tag} {extra_tag} ."
            )
        else:
            extra_tag = "" if no_latest else f"-t {name}:latest"
            command = f"docker build --load -t {name}:{tag} {extra_tag} {args} ."

        if no_cache:
            command = command + " --no-cache"

        print()
        print_step(f"Building {image}:{tag}")

        print(command)
        print()

        os.chdir(os.path.dirname(dockerfile))
        if system(command) != 0:
            print_error(f"Could not build image {image}")
            sys.exit(1)

    print()
    print_step("Built images: {}".format(", ".join(to_build)))


def parse_images(to_parse: list[str]) -> list[str]:
    """Return all available images if none was specified."""
    if not to_parse:
        return list(IMAGES.keys())

    return to_parse


if __name__ == "__main__":
    PARSER = ArgumentParser(description="Build or push Docker images")

    # CLI commands
    SUB_PARSERS = PARSER.add_subparsers(dest="command")
    SUB_PARSERS.required = True

    LIST_PARSER = SUB_PARSERS.add_parser("list")
    BUILD_PARSER = SUB_PARSERS.add_parser("build")
    BUILDX_PARSER = SUB_PARSERS.add_parser("buildx")

    # CLI arguments
    LIST_PARSER.add_argument("images", type=str, nargs="*")

    BUILD_PARSER.add_argument("images", type=str, nargs="*")
    BUILD_PARSER.add_argument("--no-cache", dest="no_cache", action="store_true")
    BUILD_PARSER.add_argument("--no-latest", dest="no_latest", action="store_true")
    BUILD_PARSER.add_argument("--branch", default="", help="Branch to build")
    BUILD_PARSER.add_argument(
        "--organisation",
        default="boltz",
        help="The organisation to use for the image names",
    )

    BUILDX_PARSER.add_argument("images", type=str, nargs="*")
    BUILDX_PARSER.add_argument("--no-cache", dest="no_cache", action="store_true")
    BUILDX_PARSER.add_argument("--no-latest", dest="no_latest", action="store_true")
    BUILDX_PARSER.add_argument("--branch", default="", help="Branch to build")
    BUILDX_PARSER.add_argument(
        "--platform",
        default="linux/amd64,linux/arm64",
        help="The platforms to build for",
    )
    BUILDX_PARSER.add_argument(
        "--organisation",
        default="boltz",
        help="The organisation to use for the image names",
    )

    ARGS = PARSER.parse_args()

    PARSED_IMAGES = parse_images(ARGS.images)

    if ARGS.command == "list":
        list_images(PARSED_IMAGES)
    elif ARGS.command == "build":
        build_images(
            PARSED_IMAGES,
            ARGS.organisation,
            ARGS.no_cache,
            ARGS.no_latest,
            ARGS.branch,
            False,
        )
    elif ARGS.command == "buildx":
        build_images(
            PARSED_IMAGES,
            ARGS.organisation,
            ARGS.no_cache,
            ARGS.no_latest,
            ARGS.branch,
            True,
            ARGS.platform,
        )
