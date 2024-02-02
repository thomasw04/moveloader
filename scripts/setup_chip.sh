#!/bin/bash
set -eoux pipefail

if ! command -v st-flash &> /dev/null
then
	echo "st-flash could not be found. Please install it."
	exit
fi

# This script sets up the chip option bytes to be exactly as expected
# First of all, we back up the current option bytes
st-flash --flash=0x200000 read option_bytes_backup.bin 0x1FF00000 8

echo "Current option bytes:"
st-flash --area=option read

# When reading a newly produced chip with
#    st-flash --debug read option_bytes_dump.bin 0x1FF00000 8
# it outputs
#    aa f8 ef ff 55 07 10 00
# Which is equivalent to the "ST production value" of 0xFFEFF8AA (once normal, once bitwise NOT)
#
# So to reset to the default value, just use this command:
#    st-flash --area=option write 0xFFEFF8AA

# This sets the chip to dual-bank mode
st-flash --area=option write 0xfbeff8aa
# Single bank would look like this:
#    st-flash --area=option write 0xFF8FF8AA


# Reset the chip
st-flash --flash=0x200000 reset

# Make sure the new page size has been applied
# See test_hardware.sh to see why this check currently doesn't work as expected
# OUTPUT="$(st-info --pagesize)"
# if [ "$OUTPUT" != "0x2000" ]; then
#   echo "Error: Page size is incorrect. Expected 0x2000, got $OUTPUT"
#   exit 1
# fi
