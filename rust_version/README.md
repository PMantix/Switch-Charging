# Switch Charging Controller (Rust)

High-performance MOSFET switching controller for Raspberry Pi 5, rewritten in Rust for improved timing precision at high frequencies.

## Features

- **High-frequency support**: Up to 10kHz (vs ~300Hz limit in Python)
- **Precise timing**: Native code with no garbage collection pauses
- **Same functionality**: Rotary encoder, TM1637 display, sequence selection
- **Graceful shutdown**: Clean MOSFET turn-off on Ctrl+C

## Building

### Prerequisites

Install Rust on your Raspberry Pi 5:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

### Compile

```bash
cd rust_version
cargo build --release
```

The optimized binary will be at `target/release/switch_charging`

### Cross-compile from another machine (optional)

```bash
# On your development machine
rustup target add aarch64-unknown-linux-gnu

# Install cross-compiler
sudo apt install gcc-aarch64-linux-gnu

# Build
cargo build --release --target aarch64-unknown-linux-gnu
```

## Running

```bash
# Must run as root for GPIO access
sudo ./target/release/switch_charging

# With debug logging
sudo RUST_LOG=info ./target/release/switch_charging
```

## GPIO Pinout

| Function | GPIO Pin |
|----------|----------|
| P1 (High-side MOSFET 1) | 17 |
| P2 (High-side MOSFET 2) | 27 |
| N1 (Low-side MOSFET 1) | 22 |
| N2 (Low-side MOSFET 2) | 23 |
| Display CLK | 18 |
| Display DIO | 24 |
| Rotary CLK | 16 |
| Rotary DT | 20 |
| Rotary Button | 21 |

## Controls

- **Rotate**: Adjust frequency (0.1 - 10,000 Hz)
- **Short press**: Toggle step rate (x1 / x10)
- **Hold 0.5s**: Enter/exit sequence selection mode
- **In sequence mode**:
  - Rotate to select sequence (1-8)
  - Short press to toggle detailed view

## Sequences

| # | Pattern | Description |
|---|---------|-------------|
| 1 | [5,5,5,5] | All OFF |
| 2 | [0,1,2,3] | Standard rotation |
| 3 | [0,1,3,2] | Alternate 1 |
| 4 | [0,2,1,3] | Alternate 2 |
| 5 | [0,2,3,1] | Alternate 3 |
| 6 | [0,3,1,2] | Alternate 4 |
| 7 | [0,3,2,1] | Alternate 5 |
| 8 | [4,4,4,4] | All ON |

## Performance Notes

- At 10kHz, step time is 50μs - Rust handles this easily
- The main loop runs without sleep during active switching
- Display updates are decoupled from switching timing
- GPIO operations use direct register access via rppal

## Differences from Python Version

1. **No display caching** - Rust is fast enough that it doesn't matter
2. **Increased max frequency** - 10kHz vs 300Hz
3. **Atomic operations** - Thread-safe rotary encoder handling
4. **Compiled binary** - No interpreter overhead
