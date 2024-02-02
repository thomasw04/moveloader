#!/bin/bash
set -eou pipefail

# The duration in which we expect the bootloader to fix broken metadata pages
SLEEP_DURATION=3

# Ensure st-flash is installed
if ! command -v st-flash &> /dev/null
then
	echo "st-flash could not be found. Please install it."
	exit
fi

# This is a hardware test script that can be run to test if
# the bootloader correctly interacts with the hardware.

SCRIPTS_DIR="$(realpath $(dirname "$0"))"
REPO_DIR="$(realpath $SCRIPTS_DIR/..)"
DUMMY_OS_IMAGE="$REPO_DIR/image-builder/testdata/main_ram.bin"
TEMP_DIR="$(mktemp -d -t hardware_test.XXXXXX)"
echo "Working in $TEMP_DIR"

# Delete on exit
# trap "rm -rf $TEMP_DIR" EXIT SIGINT SIGTERM

echo "Ensuring containers are up to date..."
"$SCRIPTS_DIR/setup_containers.sh"

echo "Building bootloader..."
"$SCRIPTS_DIR/build_bootloader.sh"

mv bootloader.bin "$TEMP_DIR/bootloader.bin"
cd "$TEMP_DIR"
cp -f "$DUMMY_OS_IMAGE" dummy_os_image.bin

echo "Building image..."
"$SCRIPTS_DIR/build_image.sh" write -b bootloader.bin -1 dummy_os_image.bin -o correct_image.bin

# ensure_file_equals expected_file actual_file error_message
ensure_file_equals() {
	if ! cmp -s "$1" "$2"; then
		echo "ERROR: $1 and $2 are not the same file: $3 ($TEMP_DIR)"

		# Print the image builder output
		"$SCRIPTS_DIR/build_image.sh" read -i "$2"

		exit 1
	fi

	echo "File $1 and $2 are the same"
}

# checks that the first argument image is broken, e.g.
# ensure_image_is_broken broken_image_md1.bin
ensure_image_is_broken() {
	if "$SCRIPTS_DIR/build_image.sh" read -i "$1" > /dev/null 2>&1; then
		echo "ERROR: Expected image $1 to be broken, but it is not ($TEMP_DIR)"
		exit 1
	fi

	echo "Image $1 is broken as expected"
}

# checks that the first argument image is valid, e.g.
# ensure_image_is_valid correct_image.bin
ensure_image_is_valid() {
	if ! "$SCRIPTS_DIR/build_image.sh" read -i "$1" > /dev/null 2>&1; then
		echo "ERROR: Expected image $1 to be valid, but it is not ($TEMP_DIR)"
		exit 1
	fi

	echo "Image $1 is valid as expected"
}

output() {
	echo "----------------------------------------"
	echo "$@"
	echo "----------------------------------------"
}

# Do not show eraseflash all the time in the output, keep it to a single line
process_flash_output() {
    while IFS= read -r line; do
        if [[ $line == "EraseFlash"* ]]; then
            printf "\r%s" "$line"
        else
            echo "$line"
        fi
    done
}

# Wrapper around st-flash that adds --reset --flash=0x200000 to force 2MB flash size
stflash() {
	# We process the output, so that the EraseFlash output takes less screen space
	st-flash --reset --flash=0x200000 "$@" 2>&1 | process_flash_output
}

set_single_bank_mode() {
	st-flash --area=option write 0xff8ff8aa
}

set_dual_bank_mode() {
	st-flash --area=option write 0xfbeff8aa
}

check_chip_type() {
	output "Check 1: Chip must be STM32L4Rx"
	OUTPUT="$(st-info --descr)"
	if [ "$OUTPUT" != "STM32L4Rx" ]; then
	echo "Check 1: Incorrect chip description. Expected STM32L4Rx, got $OUTPUT"
	exit 1
	fi
	output "CHECK 1: Chip description as expected"
}


test_single_bank_correct() {
	output "TEST 1: Single bank flash and read back correct image"

	ensure_image_is_valid correct_image.bin

	stflash write correct_image.bin 0x8000000
	# We don't need the chip to do anything in between, so no need to sleep
	stflash read read_expect_correct_image.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image.bin

	ensure_file_equals correct_image.bin read_expect_correct_image.bin \
		"Test 1: Flash and read back did not yield the same file"

	output "PASSED TEST 1"
}

test_single_bank_md1_broken() {
	output "TEST 2: Single bank flash broken metadata page one, wait for bootloader to fix it, and read it back"

	# Copy correct_image.bin, but set 17 bytes at 0x2000 to 0xff
	# This will break the first metadata page, but leaves the second one intact
	cp -f correct_image.bin broken_image_md1.bin
	printf '\xff%.0s' {1..17} | dd of=broken_image_md1.bin bs=1 seek=8192 count=17 conv=notrunc
	ensure_image_is_broken broken_image_md1.bin

	# Bootloader fixup should result in correct_image, except that everything on
	# the first metadata page after the correct metadata is 0xff - the reset value of flash memory
	cp -f correct_image.bin expected_image_md1.bin
	printf '\xff%.0s' {1..8128} | dd of=expected_image_md1.bin bs=1 seek=8256 count=8128 conv=notrunc
	ensure_image_is_valid expected_image_md1.bin

	# Now for the more interesting tests: we flash a broken image, wait for the bootloader to fix it, and then read it back
	# This should yield the correct image
	stflash write broken_image_md1.bin 0x8000000
	# Give it some time to boot and fix the image
	echo "Waiting for bootloader to fix metadata page one ..."
	sleep $SLEEP_DURATION
	stflash read read_expect_correct_image_md1_was_broken.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image_md1_was_broken.bin

	ensure_file_equals expected_image_md1.bin read_expect_correct_image_md1_was_broken.bin \
		"Test 2: The bootloader did not fix broken metadata in first metadata page"
	output "PASSED TEST 2"
}

test_single_bank_md2_broken() {
	output "TEST 3: Single bank flash broken metadata page two, wait for bootloader to fix it, and read it back"

	# Similar for the second metadata page at 0x4000, but different bytes
	cp -f correct_image.bin broken_image_md2.bin
	printf '\xff%.0s' {1..5} | dd of=broken_image_md2.bin bs=1 seek=16384 count=5 conv=notrunc
	ensure_image_is_broken broken_image_md2.bin

	# Bootloader fixup should result in correct_image, except that everything on
	# the second metadata page after the correct metadata is 0xff - the reset value of flash memory
	cp -f correct_image.bin expected_image_md2.bin
	printf '\xff%.0s' {1..8128} | dd of=expected_image_md2.bin bs=1 seek=16448 count=8128 conv=notrunc
	ensure_image_is_valid expected_image_md2.bin

	stflash write broken_image_md2.bin 0x8000000
	# Give it some time to boot and fix the image
	echo "Waiting for bootloader to fix metadata page two ..."
	sleep $SLEEP_DURATION
	stflash read read_expect_correct_image_md2_was_broken.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image_md2_was_broken.bin

	ensure_file_equals expected_image_md2.bin read_expect_correct_image_md2_was_broken.bin \
		"Test 3: The bootloader did not fix broken metadata in second metadata page"

	output "PASSED TEST 3"
}

test_dual_bank_correct() {
	output "TEST 4: Dual bank flash and read back correct image"

	ensure_image_is_valid correct_image.bin

	stflash write correct_image.bin 0x8000000
	# We don't need the chip to do anything in between, so no need to sleep
	stflash read read_expect_correct_image.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image.bin

	ensure_file_equals correct_image.bin read_expect_correct_image.bin \
		"Test 4: Flash and read back did not yield the same file"

	output "PASSED TEST 4"
}


test_dual_bank_md1_broken() {
	output "TEST 5: Dual bank flash broken metadata page one, wait for bootloader to fix it, and read it back"

	# Copy correct_image.bin, but set 17 bytes at 0x2000 to 0xff
	# This will break the first metadata page, but leaves the second one intact
	cp -f correct_image.bin broken_image_md1.bin
	printf '\xff%.0s' {1..17} | dd of=broken_image_md1.bin bs=1 seek=8192 count=17 conv=notrunc
	ensure_image_is_broken broken_image_md1.bin

	# Bootloader fixup should result in correct_image, except that everything on
	# the first metadata page after the correct metadata is 0xff - the reset value of flash memory
	# In this case, the page is 0x1000 bytes in length
	cp -f correct_image.bin expected_image_md1.bin
	printf '\xff%.0s' {1..4032} | dd of=expected_image_md1.bin bs=1 seek=8256 count=4032 conv=notrunc
	ensure_image_is_valid expected_image_md1.bin

	# Now for the more interesting tests: we flash a broken image, wait for the bootloader to fix it, and then read it back
	# This should yield the correct image
	stflash write broken_image_md1.bin 0x8000000
	# Give it some time to boot and fix the image
	echo "Waiting for bootloader to fix metadata page one ..."
	sleep $SLEEP_DURATION
	stflash read read_expect_correct_image_md1_was_broken.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image_md1_was_broken.bin

	ensure_file_equals expected_image_md1.bin read_expect_correct_image_md1_was_broken.bin \
		"Test 5: The bootloader did not fix broken metadata in first metadata page"
	output "PASSED TEST 5"
}


test_dual_bank_md2_broken() {
	output "TEST 6: Single bank flash broken metadata page two, wait for bootloader to fix it, and read it back"

	# Similar for the second metadata page at 0x4000, but different bytes
	cp -f correct_image.bin broken_image_md2.bin
	printf '\xff%.0s' {1..5} | dd of=broken_image_md2.bin bs=1 seek=16384 count=5 conv=notrunc
	ensure_image_is_broken broken_image_md2.bin

	# Bootloader fixup should result in correct_image, except that everything on
	# the second metadata page after the correct metadata is 0xff - the reset value of flash memory
	cp -f correct_image.bin expected_image_md2.bin
	printf '\xff%.0s' {1..4032} | dd of=expected_image_md2.bin bs=1 seek=16448 count=4032 conv=notrunc
	ensure_image_is_valid expected_image_md2.bin

	stflash write broken_image_md2.bin 0x8000000
	# Give it some time to boot and fix the image
	echo "Waiting for bootloader to fix metadata page two ..."
	sleep $SLEEP_DURATION
	stflash read read_expect_correct_image_md2_was_broken.bin 0x8000000 0x200000

	ensure_image_is_valid read_expect_correct_image_md2_was_broken.bin

	ensure_file_equals expected_image_md2.bin read_expect_correct_image_md2_was_broken.bin \
		"Test 6: The bootloader did not fix broken metadata in second metadata page"

	output "PASSED TEST 6"
}

# TODO: we could also do a check for the page size in dual/single bank mode,
# however, st-info returns 0x1000 in both cases - I assume that's an st-info bug,
# because the bootloader can correctly read the dbank bit and erased pages
# are as long as expected (0x2000 in single bank, 0x1000 in dual bank)
check_chip_type

set_single_bank_mode

test_single_bank_correct
test_single_bank_md1_broken
test_single_bank_md2_broken

# In dual-bank mode, we have the same logic, except that pages are now 0x1000 bytes long
set_dual_bank_mode
test_dual_bank_correct
test_dual_bank_md1_broken
test_dual_bank_md2_broken

echo "All tests passed!"
