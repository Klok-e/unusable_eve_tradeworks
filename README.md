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

## Configs
There may be a config for each trade route.

You specify them like this:
```bash
cargo run --release -- -c config.jita-t0dt.json
```

Sample config is in the file `unusable_eve_tradeworks/example.config.json`.
