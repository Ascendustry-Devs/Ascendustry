SHELL := /bin/bash
.PHONY: run

clean:
	cargo clean

test:
	cargo test

doc:
	cargo doc --no-deps --open --document-private-items

clean-doc:
	cargo clean --doc

new-doc: clean-doc doc

run: launcher

killall:
	pkill -f target/debug/server 2>/dev/null
	pkill -f target/release/server 2>/dev/null
	pkill -f target/debug/client 2>/dev/null
	pkill -f target/release/client 2>/dev/null

kill: killall

fmt:
	cargo fmt

check: fmt
	cargo check

clean-code: fmt
	cargo fix --allow-dirty

client-build: fmt
	RUSTFLAGS="-Awarnings" cargo build -p client --bin Ascendustry
server-build: fmt
	RUSTFLAGS="-Awarnings" cargo build -p server --bin server

launch: launcher
launcher: fmt client-build server-build
	RUSTFLAGS="-Awarnings" cargo run -p launcher --bin launcher

launcher-profile: client-profile server-profile fmt
	RUSTFLAGS="-Awarnings" cargo run --profile flamegraph -p launcher --bin launcher

client-profile: fmt
	RUSTFLAGS="-C force-frame-pointers=yes" cargo run --profile flamegraph -p client

launcher-release: fmt client-release-build server-release-build
	RUSTFLAGS="-Awarnings" cargo run -r -p launcher --bin launcher

launcher-release-build: fmt
	cargo build -r -p launcher --bin launcher

server-release: fmt
	RUSTFLAGS="-Awarnings" cargo run -r -p server --bin server

client-release-build: fmt
	cargo build -r -p client --bin Ascendustry
server-release-build: fmt
	cargo build -r -p server --bin server

full-release: launcher-release-build server-release-build
