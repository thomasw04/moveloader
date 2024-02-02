#!/bin/bash
set -eou pipefail

source "$(dirname "$0")/env"

"$CONTAINER_RUNTIME" run \
    -v "$(pwd)":/mount \
	-it "$IMAGE_BUILDER_IMAGE_NAME" "$@"
