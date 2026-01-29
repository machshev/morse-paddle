#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputType, Pull, Speed},
    i2c::{self},
    peripherals,
    time::khz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_time::Timer;
use morse_paddle::{IambicMode, Keyer, PaddleInput, send_element};
use ssd1306::{I2CDisplayInterface, Ssd1306Async, prelude::*};
use {defmt_rtt as _, panic_probe as _};

const WPM: u64 = 15;
const UNIT_MS: u64 = 1200 / WPM;

bind_interrupts!(struct Irqs {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let dit = Input::new(p.PA0, Pull::Up);
    let dah = Input::new(p.PA1, Pull::Up);
    let mut led = Output::new(p.PC13, Level::High, Speed::Low); // Active-low

    let mut buzzer_act = Output::new(p.PB8, Level::Low, Speed::Low); // Active-low

    let pwm_pin = PwmPin::new(p.PA6, OutputType::PushPull);
    let mut pwm = SimplePwm::new(
        p.TIM3,
        Some(pwm_pin),
        None,
        None,
        None,
        khz(3),
        Default::default(),
    );
    let mut buzzer_pass = pwm.ch1();
    buzzer_pass.enable();
    buzzer_pass.set_duty_cycle_fully_off();

    let i2c = i2c::I2c::new(
        p.I2C1,
        p.PB6,
        p.PB7,
        Irqs,
        p.DMA1_CH6,
        p.DMA1_CH7,
        Default::default(),
    );

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306Async::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_terminal_mode();
    display.init().await.unwrap();
    let _ = display.clear().await;

    let _ = display.write_str("Hello").await;

    info!("Iambic Mode B keyer ready –  {} WPM", WPM);

    let mut keyer = Keyer::new(IambicMode::B);

    loop {
        let paddle_input = PaddleInput::from_io(dit.is_low(), dah.is_low());

        match keyer.update(paddle_input) {
            Some(p) => {
                send_element(&mut led, &mut buzzer_act, &mut buzzer_pass, UNIT_MS, p).await;
            }
            None => {
                // Nothing pressed and no pending → idle
                Timer::after_millis(UNIT_MS / 10).await;
                continue;
            }
        };
    }
}
