cargo build --all-features --release && scp target/armv5te-unknown-linux-musleabi/release/bricksorter ev3:bricksorter && ssh -t ev3 RUST_BACKTRACE=full ./run.sh
