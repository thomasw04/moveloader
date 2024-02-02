FROM ubuntu:22.04

# The packages we need, as documented in the README
RUN DEBIAN_FRONTEND=noninteractive apt-get update && apt-get install -y \
	build-essential cmake libusb-1.0 libusb-1.0-0-dev gdb-multiarch gcc-arm-none-eabi openocd \
	qemu-system-arm curl wget git \
	pkg-config libssl-dev # required for the cargo-tarpaulin install

RUN ln -s /usr/bin/gdb-multiarch /usr/bin/arm-none-eabi-gdb

# Install Rust
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y && \
	echo 'source $HOME/.cargo/env' >> $HOME/.bashrc && \
	echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> $HOME/.bashrc
ENV PATH="${HOME}/.cargo/bin:${PATH}"

RUN $HOME/.cargo/bin/rustup default nightly
RUN $HOME/.cargo/bin/rustup target add thumbv7em-none-eabi
RUN $HOME/.cargo/bin/rustup component add llvm-tools-preview

# Install tools for:
# - binary (size) inspection
# - test coverage generation
# - file watching
RUN $HOME/.cargo/bin/cargo install cargo-binutils cargo-tarpaulin watchexec-cli

# Install specific version of stlink tools
RUN git clone --single-branch -b fix-stm32l4r5 https://github.com/bauen1/stlink.git /tmp/stlink && \
	cd /tmp/stlink && \
	make clean && \
	make release && \
	make install && \
	ldconfig && \
	rm -r /tmp/stlink

# Tools for formal verification
RUN apt-get install -y \
	python3 python3-pip python3-setuptools python3-wheel

RUN $HOME/.cargo/bin/cargo install --locked kani-verifier && \
	$HOME/.cargo/bin/cargo kani setup
