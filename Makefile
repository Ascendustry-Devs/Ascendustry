SHELL := /bin/bash
.PHONY: build server-bg client-bg server client clean-logs fmt check run killall kill clean-code

build-bg: fmt
	RUSTFLAGS="-Awarnings" cargo build >/dev/null 2>&1

build: fmt
	RUSTFLAGS="-Awarnings" cargo build


server-bg: build-bg
	RUSTFLAGS="-Awarnings" cargo run -q -p server --bin server | tee logs/server.txt &

client-bg: build-bg
	RUSTFLAGS="-Awarnings" cargo run -q -p client --bin client | tee logs/client.txt

server: build
	RUSTFLAGS="-Awarnings" cargo run -p server --bin server

client: build
	RUSTFLAGS="-Awarnings" cargo run -p client --bin Ascendustry

server-release:
		RUSTFLAGS="-Awarnings" cargo run -r -p server --bin server

client-release:
		RUSTFLAGS="-Awarnings" cargo run -r -p client --bin Ascendustry

launcher: build
	RUSTFLAGS="-Awarnings" cargo run -p launcher --bin launcher
launcher-release:
	RUSTFLAGS="-Awarnings" cargo run -r -p launcher --bin launcher
launcher-profile:
	RUSTFLAGS="-Awarnings" cargo run --profile flamegraph -p launcher --bin launcher

client-profile:
	RUSTFLAGS="-C force-frame-pointers=yes" cargo run --profile flamegraph -p client

doc:
	cargo doc --no-deps --open --document-private-items

clean-doc:
	cargo clean --doc

new-doc: clean-doc doc

clean-logs:
	rm logs/* -rf

fmt:
	cargo fmt

check: fmt
	cargo check

run: clean-logs server-bg client killall

killall:
	pkill -f target/debug/server 2>/dev/null
	pkill -f target/release/server 2>/dev/null
	pkill -f target/debug/client 2>/dev/null
	pkill -f target/release/client 2>/dev/null

kill: killall


clean-code:
	cargo fix --allow-dirty

launch: launcher
