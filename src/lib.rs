#![no_main]
#![no_std]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt_rtt as _; // global logger
use panic_probe as _;

use embassy_stm32::gpio::Output;
use embassy_time::Timer;

pub enum Pulse {
    DIT,
    DAH,
}

impl Pulse {
    pub fn duration(&self, unit: u64) -> u64 {
        match self {
            Pulse::DIT => unit,
            Pulse::DAH => 3 * unit,
        }
    }
}

pub async fn send_element(led: &mut Output<'_>, buzzer: &mut Output<'_>, unit: u64, pulse: Pulse) {
    let duration = pulse.duration(unit);

    led.set_low(); // Key down
    buzzer.set_high(); // Key down

    Timer::after_millis(duration).await;

    led.set_high(); // Key up
    buzzer.set_low(); // Key up

    Timer::after_millis(unit).await; // Inter-element spacing
}

#[cfg(test)]
#[defmt_test::tests]
mod tests {
    use super::*;

    #[test]
    fn pulse_duration_correct() {
        assert_eq!(Pulse::DIT.duration(120), 120);
        assert_eq!(Pulse::DAH.duration(120), 360);
    }
}
