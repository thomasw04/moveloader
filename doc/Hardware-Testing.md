# Hardware testing

A lot of the testing and verification that happens for the bootloader is either via normal Rust tests, formal verification or "verifying by debugging on chip" to make sure the bootloader acts as expected.

In addition, there are hardware testing utilities that are outlined in this document.

## Setup

To run the hardware tests, grab a STM32L4Rx chip with exactly 2MB of storage (e.g. a [NUCLEO-L4R5ZI](https://www.st.com/en/product/nucleo-l4r5zi)). You also need to have Docker installed for building the bootloader and related images.

## Chip configuration

We must configure the chip with the following settings:

- Single-bank mode for the flash (by default, it uses dual-bank mode)
  - This means that we want a page size of `0x2000` bytes

This can be done by setting the option bits of the flash. Plug in your chip and run the setup script:

```sh
./scripts/setup_chip.sh
```

## Testing

Now that we've set up the chip as expected, we can run tests that the bootloader works as expected:

```sh
./scripts/test_hardware.sh
```

If everything worked, it should output "All tests passed!".
