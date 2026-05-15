<!--
SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>

SPDX-License-Identifier: Apache-2.0
-->

# morse-paddle

An iambic morse code keyer for the STM32F103C8 (Blue Pill), written in Rust using Embassy.

Supports Iambic Mode A and B. Defaults to Mode B at 15 WPM.

## Hardware

| Peripheral | Pin |
|---|---|
| Dit paddle | PA0 |
| Dah paddle | PA1 |
| LED (active-low) | PC13 |
| Active buzzer | PB8 |
| Passive buzzer (PWM, TIM4 CH4) | PB9 |
| OLED display SDA (I2C1) | PB7 |
| OLED display SCL (I2C1) | PB6 |

The OLED display is an SSD1306 128×32 module. It is optional — the keyer will log a warning and continue without it if the display is not connected.

Paddle inputs are active-low (pulled up internally, closed to GND).

Either an active or passive buzzer can be fitted — populate whichever suits your build:

- **Active buzzer** (PB8) — a self-oscillating buzzer driven by a simple GPIO high/low. Simplest option; no tone or volume control.
- **Passive buzzer** (PB9) — driven by PWM at 3 kHz via TIM4 CH4. Tone can be adjusted by changing the PWM frequency; volume can be adjusted by changing the duty cycle (currently 30%).

Active and passive buzzers are often difficult to tell apart visually. To identify yours, briefly apply 3–5 V DC directly across the pins: an active buzzer will emit a continuous tone; a passive buzzer will only produce a faint click (or nothing). Many modules are also labelled on the underside. Connecting a passive buzzer to PB8 will produce no sound, and connecting an active buzzer to PB9 may produce a distorted tone or no sound at all.

## Development environment

A Nix flake is provided that gives you a complete, reproducible development environment including Rust (stable, with the `thumbv7m-none-eabi` target), `probe-rs`, `flip-link`, `rust-analyzer`, `openocd`, `gcc-arm-embedded`, and a set of useful cargo tools.

With [Nix] and [direnv] installed, entering the project directory is all that's needed:

```bash
direnv allow
```

direnv will automatically activate the flake's dev shell whenever you `cd` into the directory, and deactivate it when you leave. No manual `nix develop` required.

If you prefer not to use direnv, you can enter the shell manually:

```bash
nix develop
```

Without Nix, you will need to install the dependencies manually:

```bash
cargo install flip-link
cargo install probe-rs-tools --locked
```

## Building and flashing

Flash to the connected board:

```bash
cargo run
```

Or using the alias:

```bash
cargo rb morse-paddle
```

For a release build:

```bash
cargo rrb morse-paddle
```

RTT log output is printed inline by `probe-rs` during flashing.

## Running tests

Tests run on-device via `defmt-test`:

```bash
cargo test --lib
```

## Configuration

WPM is set at compile time in `src/main.rs`:

```rust
const WPM: u64 = 15;
```

Iambic mode can be changed by passing `IambicMode::A` or `IambicMode::B` to `Keyer::new`.

## Architecture

- `src/lib.rs` — keyer logic and peripheral drivers
  - `Keyer` — iambic state machine (modes A and B)
  - `KeyOutput<L, A, P>` — drives LED + active buzzer + passive buzzer for a single element
  - `MorseDisplay<DI>` — SSD1306 async terminal mode wrapper
- `src/main.rs` — hardware initialisation and main loop

[`probe-rs`]: https://probe.rs/docs/getting-started/installation/
[`flip-link`]: https://github.com/knurling-rs/flip-link
[Nix]: https://nixos.org/download/
[direnv]: https://direnv.net/
