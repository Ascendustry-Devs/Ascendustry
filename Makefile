SHELL := /bin/bash
.PHONY: build server-bg client-bg server client clean-logs fmt check run killall kill clean-code

build-bg: fmt
	RUSTFLAGS="-Awarnings" cargo build >/dev/null 2>&1

build: fmt
	RUSTFLAGS="-Awarnings" cargo build

server-bg: build-bg
	RUSTFLAGS="-Awarnings" cargo run -q -p server --bin server | tee logs/server.txt

client-bg: build-bg
	RUSTFLAGS="-Awarnings" cargo run -q -p client --bin client | tee logs/client.txt

server: build
	RUSTFLAGS="-Awarnings" cargo run -p server --bin server

client: build
	RUSTFLAGS="-Awarnings" cargo run -p client --bin client

profile:
	cargo flamegraph --profile flamegraph -p client --bin client

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
	cargo fix -p server --bin "server"
	cargo fix -p client --bin "client"
