# Tell make to do as many things as possible at once - e.g. build bootloader and image builder in parallel,
# but only if there isn't a different -j flag set.
ifeq ($(filter -j,$(MAKEFLAGS)),)
	MAKEFLAGS += -j
endif

PROJECT_DIRS += bootloader image-builder interface

.PHONY: test coverage watch-coverage flashable-image.bin flash hello blinky read direct bootloader image-builder clean verify

DEFAULT_OS_IMAGE = image-builder/testdata/main.bin

BOOTLOADER_ELF_RELEASE = bootloader/target/thumbv7em-none-eabi/release/moveloader
BOOTLOADER_RELEASE = $(BOOTLOADER_ELF_RELEASE).bin

# Max size is same as METADATA_ADDRESS in interface/src/lib.rs
MAX_BOOTLOADER_SIZE = 8192

check_file_size = \
	if [ -e "$(BOOTLOADER_RELEASE)" ]; then \
		file_size=$$(stat -c%s "$(BOOTLOADER_RELEASE)"); \
		if [ "$$file_size" -gt "$(MAX_BOOTLOADER_SIZE)" ]; then \
			echo "Bootloader binary is larger than $(MAX_BOOTLOADER_SIZE) bytes. Exiting."; \
			exit 1; \
		else \
			echo "Bootloader binary is $$file_size bytes, is within size limit."; \
		fi; \
	else \
		echo "Bootloader binary not found. Exiting."; \
		exit 1; \
	fi

flashable-image.bin: bootloader image-builder
	image-builder write -b $(BOOTLOADER_RELEASE) -1 $(DEFAULT_OS_IMAGE) -o $@

broken-image.bin: flashable-image.bin
	cp -f $^ $@
	printf '\xff%.0s' {1..17} | dd of=$@ bs=1 seek=8192 count=17 conv=notrunc

flash-broken: broken-image.bin
	st-flash --reset --flash=0x200000 write $^ 0x8000000

image-builder:
	# Build the image generator
	cd image-builder && cargo install --path . && cd ..

bootloader:
	# First of all, build the bootloader
	cd bootloader && cargo build --release --target thumbv7em-none-eabi && cd ..
	arm-none-eabi-objcopy -O binary $(BOOTLOADER_ELF_RELEASE) $(BOOTLOADER_RELEASE)

image: $(DEFAULT_OS_IMAGE)
	st-flash --reset --flash=0x200000 write $^ 0x8000000

read:
	st-flash --flash=0x200000 read read.bin 0x8000000 0x200000

flash: flashable-image.bin
	image-builder read -i $^
	st-flash --reset --flash=0x200000 write $^ 0x8000000

direct:
	cd bootloader && cargo build --release && cd ..
	st-flash --reset --flash=0x200000 write $(BOOTLOADER_RELEASE) 0x8000000

check: bootloader test verify

test:
	# Run cargo test in each sub director
	$(foreach dir,$(PROJECT_DIRS),cd $(dir) && cargo test && cd ..;)

coverage:
	$(foreach dir,$(PROJECT_DIRS),cd $(dir) && cargo tarpaulin --out Lcov --skip-clean && cd ..;)

watch-coverage:
	watchexec --exts rs make coverage

clean:
	$(foreach dir,$(PROJECT_DIRS),cd $(dir) && cargo clean && cd ..;)

verify:
	cd bootloader && cargo kani -Z concrete-playback --concrete-playback=print | tail -n1 && cd ..
	cd interface && cargo kani -Z concrete-playback --concrete-playback=print | tail -n1 && cd ..
