cargo build --features "drill" --release && scp target/armv5te-unknown-linux-musleabi/release/bricksorter ev3:bricksorter-drill
cargo build --features "debug drill" --release && scp target/armv5te-unknown-linux-musleabi/release/bricksorter ev3:bricksorter-debug-drill
cargo build --features "" --release && scp target/armv5te-unknown-linux-musleabi/release/bricksorter ev3:bricksorter-vanilla
cargo build --all-features --release && scp target/armv5te-unknown-linux-musleabi/release/bricksorter ev3:bricksorter-all-features
