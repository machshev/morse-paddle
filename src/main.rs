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
    time::hz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_time::{Instant, Timer};
use morse_paddle::{
    IambicMode, KeyOutput, Keyer, MorseDisplay, PaddleInput, Pulse, decoder::MorseDecoder,
    tutor::Tutor,
};
use ssd1306::I2CDisplayInterface;
use {defmt_rtt as _, panic_probe as _};

const WPM: u64 = 20;
const UNIT_MS: u64 = 1200 / WPM;
const KEYER_MODE: IambicMode = IambicMode::B;

bind_interrupts!(struct Irqs {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

enum AppMode {
    Normal,
    Tutor(Tutor),
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let dit = Input::new(p.PA0, Pull::Up);
    let dah = Input::new(p.PA1, Pull::Up);
    let led = Output::new(p.PC13, Level::High, Speed::Low); // Active-low
    let buzzer_act = Output::new(p.PB8, Level::Low, Speed::Low);
    let radio = OutputOpenDrain::new(p.PA4, Level::High, Speed::Low);

    let pwm_pin = PwmPin::new(p.PB9, OutputType::PushPull);
    let mut pwm = SimplePwm::new(
        p.TIM4,
        None,
        None,
        None,
        Some(pwm_pin),
        hz(800),
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
    Timer::after_millis(100).await; // Allow SSD1306 to complete power-on reset
    display.init().await;

    info!("Iambic Mode B keyer ready –  {} WPM", WPM);

    let mut keyer = Keyer::new(KEYER_MODE);
    let mut decoder = MorseDecoder::new();
    // send() already adds 1 inter-element unit, so gap thresholds are:
    //   char gap = 3 units total → 2 more at UNIT_MS/10 resolution = 20 ticks
    //   word gap = 7 units total → 6 more = 60 ticks
    let mut idle_ticks: u32 = 0;
    const CHAR_GAP_TICKS: u32 = 20;
    const WORD_GAP_TICKS: u32 = 60;

    let mut app_mode = AppMode::Normal;
    let mut dit_count: u8 = 0;

    loop {
        let dit_pressed = dit.is_low();
        let dah_pressed = dah.is_low();
        let paddle_input = PaddleInput::from_io(dit_pressed, dah_pressed);

        // ── Keyer ────────────────────────────────────────────────────────────
        match keyer.update(paddle_input) {
            Some(p) => {
                idle_ticks = 0;
                // 10 consecutive dits toggles tutor mode on/off.
                match p {
                    Pulse::Dit => dit_count += 1,
                    Pulse::Dah => dit_count = 0,
                }
                if dit_count >= 10 {
                    dit_count = 0;
                    decoder.reset();
                    idle_ticks = 0;
                    keyer = Keyer::new(KEYER_MODE);

                    match app_mode {
                        AppMode::Normal => {
                            let seed =
                                (Instant::now() - Instant::from_ticks(0)).as_millis() as u32 | 1;
                            let tutor = Tutor::new(seed);

                            info!("Entering tutor mode – target: {}", tutor.target as u8);

                            display.show_tutor_char(tutor.target).await;
                            app_mode = AppMode::Tutor(tutor);
                        }
                        AppMode::Tutor(_) => {
                            info!("Exiting tutor mode");

                            display.clear_for_normal().await;
                            app_mode = AppMode::Normal;
                        }
                    }
                    continue;
                }
                decoder.push(p);
                key_output.send(p, UNIT_MS).await;
            }
            None => {
                Timer::after_millis(UNIT_MS / 10).await;
                // Only advance the gap timer when no paddle is physically
                // held.  This prevents the Both-suppression in tutor mode
                // (or any other None-producing input) from counting toward
                // the char/word gap and causing premature decodes.
                if dit_pressed || dah_pressed {
                    idle_ticks = 0;
                } else {
                    idle_ticks += 1;
                }

                if idle_ticks == CHAR_GAP_TICKS {
                    dit_count = 0;
                    if let Some(ch) = decoder.decode() {
                        match &mut app_mode {
                            AppMode::Normal => {
                                info!("Char: {}", ch as u8);
                                display.write_char(ch).await;
                            }
                            AppMode::Tutor(t) => {
                                t.check(ch, &mut key_output, &mut display, UNIT_MS).await;
                            }
                        }
                    }
                    decoder.reset();
                } else if idle_ticks == WORD_GAP_TICKS && matches!(app_mode, AppMode::Normal) {
                    info!("Word space");
                    display.write_char(' ').await;
                }
                continue;
            }
        };
    }
}
