# unusable_eve_tradeworks
Calculates profitable items to buy in one system and sell in another, possibly with an empty market. Includes freight services costs in its calculations.

## How to use
[Install rust](https://www.rust-lang.org/tools/install).

Set nightly as the default toolchain:
```bash
rustup default nightly
```

Clone the repository:
```bash
git clone git@github.com:LokiVKlokeNaAndoke/unusable_eve_tradeworks.git
cd unusable_eve_tradeworks/
```

Use `cargo run --release` to compile and run. Also everything after `--` is passed to the executable.

Example:
```bash
cargo run --release -- -c config.jita-t0dt.json -sr
```

Run this to get a list of all commands:
```bash
cargo run --release -- -h
```
