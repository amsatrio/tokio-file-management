# dev section
start:
	RUST_BACKTRACE=1 cargo run
build:
	cargo build &> logs/build.log
dev: # auto-reload
	RUST_BACKTRACE=1 cargo watch -q -c -w src/ -x run

clean:
	cargo clean
	rm -rf target