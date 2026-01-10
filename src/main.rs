#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_time::Timer;
use morse_paddle::{Pulse, send_element};
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

    let mut pending_dit = false; // Mode B: extra element to add after release
    let mut pending_dah = false;

    loop {
        let dit_pressed = dit.is_low();
        let dah_pressed = dah.is_low();

        // Decide what to send this cycle
        let send_dit: bool = if dit_pressed && dah_pressed {
            // Squeeze both: alternate – send opposite of the pending one
            !pending_dah // if we were going to add a dah next, send dit instead (and vice versa)
        } else if pending_dit {
            true
        } else if pending_dah {
            false
        } else if dit_pressed {
            true
        } else if dah_pressed {
            false
        } else {
            // Nothing pressed and no pending → idle
            Timer::after_millis(UNIT_MS / 10).await;
            continue;
        };

        // Send the chosen element
        if send_dit {
            info!("dit");
            send_element(&mut led, &mut buzzer, UNIT_MS, Pulse::DIT).await;
            // Mode B: if dah is still held when we finish this dit, queue an extra dah
            pending_dah = dah_pressed;
            pending_dit = false;
        } else {
            info!("dah");
            send_element(&mut led, &mut buzzer, UNIT_MS, Pulse::DAH).await;
            // Mode B: if dit is still held when we finish this dah, queue an extra dit
            pending_dit = dit_pressed;
            pending_dah = false;
        }
    }
}
