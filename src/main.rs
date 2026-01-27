#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::Timer;
use morse_paddle::{IambicMode, Keyer, PaddleInput, send_element};
use {defmt_rtt as _, panic_probe as _};

const WPM: u64 = 15;
const UNIT_MS: u64 = 1200 / WPM;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let dit = Input::new(p.PA0, Pull::Up);
    let dah = Input::new(p.PA1, Pull::Up);
    let mut led = Output::new(p.PC13, Level::High, Speed::Low); // Active-low
    let mut buzzer = Output::new(p.PB8, Level::Low, Speed::Low); // Active-low

    info!("Iambic Mode B keyer ready –  {} WPM", WPM);

    let mut keyer = Keyer::new(IambicMode::B);

    loop {
        let paddle_input = PaddleInput::from_io(dit.is_low(), dah.is_low());

        match keyer.update(paddle_input) {
            Some(p) => {
                send_element(&mut led, &mut buzzer, UNIT_MS, p).await;
            }
            None => {
                // Nothing pressed and no pending → idle
                Timer::after_millis(UNIT_MS / 10).await;
                continue;
            }
        };
    }
}
