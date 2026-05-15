#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    gpio::{Input, Level, Output, OutputOpenDrain, OutputType, Pull, Speed},
    i2c::{self},
    peripherals,
    time::khz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_time::Timer;
use morse_paddle::{
    IambicMode, KeyOutput, Keyer, MorseDisplay, PaddleInput, decoder::MorseDecoder,
};
use ssd1306::I2CDisplayInterface;
use {defmt_rtt as _, panic_probe as _};

const WPM: u64 = 20;
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
    let led = Output::new(p.PC13, Level::High, Speed::Low); // Active-low
    let buzzer_act = Output::new(p.PB8, Level::Low, Speed::Low);
    let radio = OutputOpenDrain::new(p.PA4, Level::High, Speed::Low); // Open-drain: floats high, pulls to GND when keying

    let pwm_pin = PwmPin::new(p.PB9, OutputType::PushPull);
    let mut pwm = SimplePwm::new(
        p.TIM4,
        None,
        None,
        None,
        Some(pwm_pin),
        khz(3),
        Default::default(),
    );
    let mut buzzer_pass = pwm.ch4();
    buzzer_pass.enable();
    buzzer_pass.set_duty_cycle_fully_off();

    let mut key_output = KeyOutput::new(led, buzzer_act, buzzer_pass, radio);

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
    let mut display = MorseDisplay::new(interface);
    display.init().await;

    info!("Iambic Mode B keyer ready –  {} WPM", WPM);

    let mut keyer = Keyer::new(IambicMode::B);
    let mut decoder = MorseDecoder::new();
    // Character gap = 3 units total; send() already adds 1 inter-element unit,
    // so we need 2 more units of idle at UNIT_MS/10 resolution = 20 ticks.
    let mut idle_ticks: u32 = 0;
    // send() already adds 1 inter-element unit, so thresholds are:
    //   char gap = 3 units total → 2 more = 20 ticks
    //   word gap = 7 units total → 6 more = 60 ticks
    const CHAR_GAP_TICKS: u32 = 20;
    const WORD_GAP_TICKS: u32 = 60;

    loop {
        let paddle_input = PaddleInput::from_io(dit.is_low(), dah.is_low());

        match keyer.update(paddle_input) {
            Some(p) => {
                idle_ticks = 0;
                decoder.push(p);
                key_output.send(p, UNIT_MS).await;
            }
            None => {
                Timer::after_millis(UNIT_MS / 10).await;
                idle_ticks += 1;
                if idle_ticks == CHAR_GAP_TICKS {
                    if let Some(ch) = decoder.decode() {
                        info!("Char: {}", ch as u8);
                        display.write_char(ch).await;
                    }
                    decoder.reset();
                } else if idle_ticks == WORD_GAP_TICKS {
                    info!("Word space");
                    display.write_char(' ').await;
                }
                continue;
            }
        };
    }
}
