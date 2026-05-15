#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    flash::Flash,
    gpio::{Input, Level, Output, OutputOpenDrain, OutputType, Pull, Speed},
    i2c::{self},
    peripherals,
    time::hz,
    timer::simple_pwm::{PwmPin, SimplePwm},
};
use embassy_time::{Instant, Timer};
use morse_paddle::{
    KeyOutput, Keyer, MorseDisplay, PaddleInput, PassiveBuzzer, Pulse, decoder::MorseDecoder,
    menu::Menu, storage, tutor::Tutor,
};
use ssd1306::I2CDisplayInterface;
use {defmt_rtt as _, panic_probe as _};

bind_interrupts!(struct Irqs {
    I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
    I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
});

/// Wraps `SimplePwm` so we can get temporary channel borrows for duty-cycle
/// control while still being able to call `set_frequency` for pitch changes.
struct PwmBuzzer<'d> {
    pwm: SimplePwm<'d, peripherals::TIM4>,
}

impl<'d> PwmBuzzer<'d> {
    fn new(mut pwm: SimplePwm<'d, peripherals::TIM4>) -> Self {
        // Enable ch4 once; duty-cycle is controlled from here on.
        let mut ch = pwm.ch4();
        ch.enable();
        ch.set_duty_cycle_fully_off();
        PwmBuzzer { pwm }
    }
}

impl<'d> PassiveBuzzer for PwmBuzzer<'d> {
    fn buzzer_on(&mut self, volume_percent: u8) {
        // Temporary borrow; hardware register stays set after drop.
        self.pwm.ch4().set_duty_cycle_percent(volume_percent);
    }
    fn buzzer_off(&mut self) {
        self.pwm.ch4().set_duty_cycle_fully_off();
    }
    fn set_pitch_hz(&mut self, pitch: u32) {
        self.pwm.set_frequency(hz(pitch));
    }
}

enum AppMode {
    Normal,
    Menu(Menu),
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
    let pwm = SimplePwm::new(
        p.TIM4,
        None,
        None,
        None,
        Some(pwm_pin),
        hz(1000),
        Default::default(),
    );
    let buzzer_pwm = PwmBuzzer::new(pwm);

    let mut key_output = KeyOutput::new(led, buzzer_act, buzzer_pwm, radio);

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
    Timer::after_millis(200).await; // Allow SSD1306 to complete power-on reset
    display.init().await;

    let mut flash = Flash::new_blocking(p.FLASH);
    let mut settings = storage::load(&mut flash).unwrap_or_default();
    let mut keyer = Keyer::new(settings.keyer_mode);
    let mut decoder = MorseDecoder::new();
    // send() already adds 1 inter-element unit, so gap thresholds are:
    //   char gap = 3 units total → 2 more at UNIT_MS/10 resolution = 20 ticks
    //   word gap = 7 units total → 6 more = 60 ticks
    // These are WPM-independent because tick duration is always UNIT_MS/10.
    let mut idle_ticks: u32 = 0;
    const CHAR_GAP_TICKS: u32 = 20;
    const WORD_GAP_TICKS: u32 = 60;

    info!(
        "Keyer ready – {} WPM, {} Hz",
        settings.unit_ms(),
        settings.pitch_hz()
    );

    let mut app_mode = AppMode::Normal;
    let mut dit_count: u8 = 0;

    loop {
        let dit_pressed = dit.is_low();
        let dah_pressed = dah.is_low();
        let paddle_input = PaddleInput::from_io(dit_pressed, dah_pressed);

        // ── Menu mode (bypasses keyer – raw paddle edge detection) ───────────
        if matches!(app_mode, AppMode::Menu(_)) {
            Timer::after_millis(settings.unit_ms() / 10).await;

            // Update state; extract all needed values before dropping borrow.
            let (exiting, dirty, sound_preview, tutor_on) = {
                let AppMode::Menu(ref mut menu) = app_mode else {
                    core::unreachable!()
                };
                let exiting = menu.update(dit_pressed, dah_pressed, &mut settings);
                let dirty = core::mem::replace(&mut menu.display_dirty, false);
                let preview = core::mem::replace(&mut menu.sound_preview, false);
                (exiting, dirty, preview, menu.tutor_on)
            };

            if sound_preview {
                // Apply new pitch/volume immediately so the tone reflects the change.
                key_output.set_volume(settings.volume_percent());
                key_output.set_pitch(settings.pitch_hz());
                key_output.send(Pulse::Dit, settings.unit_ms()).await;
            }

            if dirty {
                let (item_label, value_label) = {
                    let AppMode::Menu(ref menu) = app_mode else {
                        core::unreachable!()
                    };
                    (menu.item.label(), menu.value_label(&settings))
                };
                display.show_menu(item_label, value_label).await;
            }

            if exiting {
                storage::save(&mut flash, &settings);
                key_output.set_volume(settings.volume_percent());
                key_output.set_pitch(settings.pitch_hz());
                keyer = Keyer::new(settings.keyer_mode);
                dit_count = 0;
                idle_ticks = 0;
                decoder.reset();

                if tutor_on {
                    let seed = (Instant::now() - Instant::from_ticks(0)).as_millis() as u32 | 1;
                    let tutor = Tutor::new(seed);
                    info!("Menu exit: entering tutor – target: {}", tutor.target as u8);
                    display.show_tutor_char(tutor.target).await;
                    app_mode = AppMode::Tutor(tutor);
                } else {
                    info!("Menu exit: normal mode");
                    display.clear_for_normal().await;
                    app_mode = AppMode::Normal;
                }
            }

            continue;
        }

        // ── Keyer ────────────────────────────────────────────────────────────
        match keyer.update(paddle_input) {
            Some(p) => {
                idle_ticks = 0;
                // 10 consecutive dits opens the settings menu.
                match p {
                    Pulse::Dit => dit_count += 1,
                    Pulse::Dah => dit_count = 0,
                }
                if dit_count >= 10 {
                    dit_count = 0;
                    decoder.reset();
                    idle_ticks = 0;
                    keyer = Keyer::new(settings.keyer_mode);

                    let tutor_active = matches!(app_mode, AppMode::Tutor(_));
                    let menu = Menu::new(tutor_active);
                    display
                        .show_menu(menu.item.label(), menu.value_label(&settings))
                        .await;
                    app_mode = AppMode::Menu(menu);
                    continue;
                }
                decoder.push(p);
                key_output.send(p, settings.unit_ms()).await;
            }
            None => {
                Timer::after_millis(settings.unit_ms() / 10).await;
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
                                t.check(ch, &mut key_output, &mut display, settings.unit_ms())
                                    .await;
                            }
                            AppMode::Menu(_) => {} // handled above
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
