SHELL := /bin/bash

# ====== Default target ======

all: launcher

# ====== Build configuration ======

RUSTFLAGS_DEBUG   := -Awarnings
RUSTFLAGS_PROFILE := -C force-frame-pointers=yes
RUSTFLAGS_RELEASE := -Awarnings

# ====== Utility targets ======

.PHONY: all fmt check test clean kill

fmt:
	cargo fmt

check: fmt
	cargo check

test:
	cargo test

clean:
	cargo clean

clean-all: clean clean-doc clean-profiler clean-flame

kill:
	-pkill -f "target/debug/" 2>/dev/null
	-pkill -f "target/release/" 2>/dev/null
	-pkill -f "target/flamegraph/" 2>/dev/null

# ====== Documentation targets ======

.PHONY: doc clean-doc new-doc

doc:
	cargo doc --no-deps --open --document-private-items

clean-doc:
	cargo clean --doc

new-doc: clean-doc doc

# ====== Binary template ======

define BINARY_template
.PHONY: $(1)-build $(1) $(1)-profile-build $(1)-profile $(1)-release-build $(1)-release

$(1)-build: fmt
	RUSTFLAGS="$$(RUSTFLAGS_DEBUG)" cargo build -p $(1)

$(1): $(1)-build
	RUSTFLAGS="$$(RUSTFLAGS_DEBUG)" cargo run -p $(1)

$(1)-profile-build: fmt
	RUSTFLAGS="$$(RUSTFLAGS_PROFILE)" cargo build --profile flamegraph -p $(1)

$(1)-profile: $(1)-profile-build
	RUSTFLAGS="$$(RUSTFLAGS_PROFILE)" cargo run --profile flamegraph -p $(1)

$(1)-release-build: fmt
	RUSTFLAGS="$$(RUSTFLAGS_RELEASE)" cargo build -r -p $(1)

$(1)-release: $(1)-release-build
	RUSTFLAGS="$$(RUSTFLAGS_RELEASE)" cargo run -r -p $(1)
endef

$(eval $(call BINARY_template,client))
$(eval $(call BINARY_template,server))

# ====== Launcher (special: depends on client + server builds) ======

.PHONY: launcher-build launcher launcher-profile-build launcher-profile
.PHONY: launcher-release-build launcher-release

launcher-build: fmt client-build server-build
	RUSTFLAGS="$(RUSTFLAGS_DEBUG)" cargo build -p launcher

launcher: launcher-build
	RUSTFLAGS="$(RUSTFLAGS_DEBUG)" cargo run -p launcher

launcher-profile-build: fmt client-profile-build server-profile-build
	RUSTFLAGS="$(RUSTFLAGS_PROFILE)" cargo build --profile flamegraph -p launcher

launcher-profile: launcher-profile-build
	RUSTFLAGS="$(RUSTFLAGS_PROFILE)" cargo run --profile flamegraph -p launcher

launcher-release-build: fmt client-release-build server-release-build
	RUSTFLAGS="$(RUSTFLAGS_RELEASE)" cargo build -r -p launcher --bin launcher

launcher-release: launcher-release-build
	RUSTFLAGS="$(RUSTFLAGS_RELEASE)" cargo run -r -p launcher --bin launcher

# ====== Profiler ======

PROFILE_NAME ?= Ascendustry

.PHONY: profile-main profile-all clean-profiler

profile-main: PROFILE_TITLE = MAIN THREAD
profile-main: PROFILE_FLAG = -t
profile-main: _profile

profile-all: PROFILE_TITLE = ALL
profile-all: PROFILE_FLAG = -p
profile-all: _profile

clean-profiler:
	rm -f out.perf perf.data perf.data.old folded.txt

clean-flame:
	@rm -f flamegraph.svg

.PHONY: _profile _flame

_profile:
	@PID=$$(pgrep -n $(PROFILE_NAME)) ; \
	echo "==== Profiler ====" ; \
	echo "Profiling: $(PROFILE_TITLE)" ; \
	echo "Ascendustry PID: $$PID" ; \
	perf record -F 99 $(PROFILE_FLAG) $$PID -g -- sleep 20 ; \
	sudo chown $$USER:$$USER perf.data
	perf script > out.perf ; \
	$(MAKE) _flame

_flame:
	$(MAKE) clean-flame
	Flamegraph/stackcollapse-perf.pl out.perf > folded.txt ; \
	Flamegraph/flamegraph.pl folded.txt > flamegraph.svg ; \
	$(MAKE) clean-profiler
