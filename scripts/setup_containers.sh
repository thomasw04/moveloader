#!/bin/bash
set -eou pipefail

source "$(dirname "$0")/env"

"$CONTAINER_RUNTIME" build --pull -t "$IMAGE_BUILDER_IMAGE_NAME" -f "$IMAGE_BUILDER_DOCKERFILE" .
"$CONTAINER_RUNTIME" build --pull -t "$BOOTLOADER_BUILDER_IMAGE_NAME" -f "$BOOTLOADER_BUILDER_DOCKERFILE" .
