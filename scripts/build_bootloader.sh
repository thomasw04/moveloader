#!/bin/bash
set -eou pipefail

source "$(dirname "$0")/env"

# Make sure bootloader and interface dir exist in pwd
if [[ ! -d bootloader ]]; then
    echo "Error: bootloader dir not found. Please make sure you run this script from the root of the project directory."
    exit 1
fi
if [[ ! -d interface ]]; then
    echo "Error: interface dir not found. Please make sure you run this script from the root of the project directory."
    exit 1
fi

# Could think about adding -v "$(pwd)/.cache/cargo-registry:/root/.cargo/registry" to cache build dependencies
"$CONTAINER_RUNTIME" run \
    -v "$(pwd)":/build \
    -v "$(pwd)":/output \
	-it "$BOOTLOADER_BUILDER_IMAGE_NAME"
