#![no_main]
#![no_std]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

pub mod decoder;

use defmt::*;
use defmt_rtt as _; // global logger
use display_interface::AsyncWriteOnlyDataCommand;
use embassy_time::Timer;
use embedded_hal::{digital::OutputPin, pwm::SetDutyCycle};
use panic_probe as _;
use ssd1306::{mode::TerminalModeAsync, prelude::*, Ssd1306Async};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Pulse {
    Dit,
    Dah,
}

impl Pulse {
    pub fn duration(&self, unit: u64) -> u64 {
        match self {
            Pulse::Dit => unit,
            Pulse::Dah => 3 * unit,
        }
    }

    pub fn toggle(&self) -> Self {
        match self {
            Pulse::Dit => Pulse::Dah,
            Pulse::Dah => Pulse::Dit,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum PaddleInput {
    DitOnly,
    DahOnly,
    Both,
}

impl PaddleInput {
    pub fn from_io(dit: bool, dah: bool) -> Option<Self> {
        match (dit, dah) {
            (true, false) => Some(Self::DitOnly),
            (false, true) => Some(Self::DahOnly),
            (true, true) => Some(Self::Both),
            (false, false) => None,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum IambicMode {
    A,
    #[default]
    B,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
enum KeyerState {
    #[default]
    Repeating,
    Alternating,
    Residual,
}

#[derive(Default)]
pub struct Keyer {
    current_pulse: Option<Pulse>,
    state: KeyerState,
    mode: IambicMode,
}

impl Keyer {
    pub fn new(mode: IambicMode) -> Self {
        Keyer {
            current_pulse: None,
            state: KeyerState::Repeating,
            mode,
        }
    }

    pub fn update(&mut self, input: Option<PaddleInput>) -> Option<Pulse> {
        self.current_pulse = match (input, self.current_pulse) {
            (Some(PaddleInput::DitOnly), _) => {
                self.state = KeyerState::Repeating;
                Some(Pulse::Dit)
            }
            (Some(PaddleInput::DahOnly), _) => {
                self.state = KeyerState::Repeating;
                Some(Pulse::Dah)
            }

            // Toggle
            (Some(PaddleInput::Both), None) => {
                self.state = KeyerState::Alternating;
                Some(Pulse::Dah)
            }
            (Some(PaddleInput::Both), Some(p)) => {
                self.state = KeyerState::Alternating;
                Some(p.toggle())
            }

            // Iambic B - add a residual pulse after key up
            (None, Some(p)) if self.mode == IambicMode::B && self.state == KeyerState::Alternating => {
                self.state = KeyerState::Residual;
                Some(p.toggle())
            }
            // Clear residual
            (None, _) if self.state == KeyerState::Residual => {
                self.state = KeyerState::Repeating;
                None
            }
            (None, _) => None,
        };

        self.current_pulse
    }
}

pub struct KeyOutput<L, A, P, R> {
    led: L,
    active: A,
    passive: P,
    radio: R,
}

impl<L: OutputPin, A: OutputPin, P: SetDutyCycle, R: OutputPin> KeyOutput<L, A, P, R> {
    pub fn new(led: L, active: A, passive: P, radio: R) -> Self {
        KeyOutput { led, active, passive, radio }
    }

    pub async fn send(&mut self, pulse: Pulse, unit: u64) {
        let duration = pulse.duration(unit);
        info!("P {}", duration);
        self.led.set_low().ok(); // Key down (active-low)
        self.active.set_high().ok(); // Key down
        self.passive.set_duty_cycle_percent(30).unwrap();
        self.radio.set_low().ok(); // Key down (open-drain: pull to GND)
        Timer::after_millis(duration).await;
        self.led.set_high().ok(); // Key up
        self.active.set_low().ok(); // Key up
        self.passive.set_duty_cycle_fully_off().unwrap();
        self.radio.set_high().ok(); // Key up (open-drain: release/float)
        Timer::after_millis(unit).await; // Inter-element spacing
    }
}

pub struct MorseDisplay<DI> {
    inner: Ssd1306Async<DI, DisplaySize128x32, TerminalModeAsync>,
}

impl<DI: AsyncWriteOnlyDataCommand> MorseDisplay<DI> {
    pub fn new(interface: DI) -> Self {
        MorseDisplay {
            inner: Ssd1306Async::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
                .into_terminal_mode(),
        }
    }

    pub async fn init(&mut self) {
        if self.inner.init().await.is_err() {
            warn!("Display init failed – no display connected?");
        } else {
            let _ = self.inner.clear().await;
            let _ = self.inner.write_str("Hello").await;
        }
    }
}

#[cfg(test)]
fn push_seq(dec: &mut decoder::MorseDecoder, seq: &[Pulse]) {
    for &p in seq {
        dec.push(p);
    }
}

#[cfg(test)]
#[defmt_test::tests]
mod tests {
    use super::{decoder, IambicMode, Keyer, PaddleInput, Pulse};
    use decoder::MorseDecoder;

    // Calling embassy_stm32::init links in the interrupt vector table and the
    // time driver, both of which the test binary needs to link cleanly.
    #[init]
    fn setup() {
        let _ = embassy_stm32::init(Default::default());
    }

    #[test]
    fn pulse_duration_correct() {
        assert_eq!(Pulse::Dit.duration(120), 120);
        assert_eq!(Pulse::Dah.duration(120), 360);
    }

    #[test]
    fn pulse_toggle() {
        assert_eq!(Pulse::Dit.toggle(), Pulse::Dah);
        assert_eq!(Pulse::Dah.toggle(), Pulse::Dit);
    }

    #[test]
    fn keyer_mode_a_dit() {
        let mut keyer = Keyer::new(IambicMode::A);

        // Single
        assert_eq!(keyer.update(Some(PaddleInput::DitOnly)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);

        // Continuous
        assert_eq!(keyer.update(Some(PaddleInput::DitOnly)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::DitOnly)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::DitOnly)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::DitOnly)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);
    }

    #[test]
    fn keyer_mode_a_dah() {
        let mut keyer = Keyer::new(IambicMode::A);

        // Single
        assert_eq!(keyer.update(Some(PaddleInput::DahOnly)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);

        // Continuous
        assert_eq!(keyer.update(Some(PaddleInput::DahOnly)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::DahOnly)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::DahOnly)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::DahOnly)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);
    }

    #[test]
    fn keyer_mode_a_both() {
        let mut keyer = Keyer::new(IambicMode::A);

        // Single
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);

        // Continuous
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);

        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);

        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);
    }

    #[test]
    fn keyer_mode_b_both() {
        let mut keyer = Keyer::new(IambicMode::B);

        // Single
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);

        // Continuous
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);

        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), None);

        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dah));
        assert_eq!(keyer.update(Some(PaddleInput::Both)), Some(Pulse::Dit));
        assert_eq!(keyer.update(None), Some(Pulse::Dah));
        assert_eq!(keyer.update(None), None);
    }

    // ------------------------------------------------------------------ decoder

    #[test]
    fn decode_e() {
        let mut d = MorseDecoder::new();
        super::push_seq(&mut d, &[Pulse::Dit]);
        assert_eq!(d.decode(), Some('E'));
    }

    #[test]
    fn decode_t() {
        let mut d = MorseDecoder::new();
        super::push_seq(&mut d, &[Pulse::Dah]);
        assert_eq!(d.decode(), Some('T'));
    }

    #[test]
    fn decode_common_letters() {
        use Pulse::{Dah, Dit};
        let cases: &[(&[Pulse], char)] = &[
            (&[Dit, Dah],           'A'),
            (&[Dah, Dit, Dit, Dit], 'B'),
            (&[Dah, Dit, Dah, Dit], 'C'),
            (&[Dah, Dit, Dit],      'D'),
            (&[Dit, Dit],           'I'),
            (&[Dit, Dah, Dah, Dah], 'J'),
            (&[Dah, Dit, Dah],      'K'),
            (&[Dit, Dah, Dit, Dit], 'L'),
            (&[Dah, Dah],           'M'),
            (&[Dah, Dit],           'N'),
            (&[Dah, Dah, Dah],      'O'),
            (&[Dit, Dah, Dah, Dit], 'P'),
            (&[Dah, Dah, Dit, Dah], 'Q'),
            (&[Dit, Dah, Dit],      'R'),
            (&[Dit, Dit, Dit],      'S'),
            (&[Dah, Dah, Dit],      'G'),
            (&[Dit, Dit, Dit, Dit], 'H'),
            (&[Dit, Dit, Dah],      'U'),
            (&[Dit, Dit, Dit, Dah], 'V'),
            (&[Dit, Dah, Dah],      'W'),
            (&[Dah, Dit, Dit, Dah], 'X'),
            (&[Dah, Dit, Dah, Dah], 'Y'),
            (&[Dah, Dah, Dit, Dit], 'Z'),
            (&[Dit, Dit, Dah, Dit], 'F'),
        ];
        for (seq, expected) in cases {
            let mut d = MorseDecoder::new();
            super::push_seq(&mut d, seq);
            assert_eq!(d.decode(), Some(*expected));
            d.reset();
        }
    }

    #[test]
    fn decode_digits() {
        use Pulse::{Dah, Dit};
        let cases: &[(&[Pulse], char)] = &[
            (&[Dit, Dit, Dit, Dit, Dit], '5'),
            (&[Dit, Dit, Dit, Dit, Dah], '4'),
            (&[Dit, Dit, Dit, Dah, Dah], '3'),
            (&[Dit, Dit, Dah, Dah, Dah], '2'),
            (&[Dit, Dah, Dah, Dah, Dah], '1'),
            (&[Dah, Dit, Dit, Dit, Dit], '6'),
            (&[Dah, Dah, Dit, Dit, Dit], '7'),
            (&[Dah, Dah, Dah, Dit, Dit], '8'),
            (&[Dah, Dah, Dah, Dah, Dit], '9'),
            (&[Dah, Dah, Dah, Dah, Dah], '0'),
        ];
        for (seq, expected) in cases {
            let mut d = MorseDecoder::new();
            super::push_seq(&mut d, seq);
            assert_eq!(d.decode(), Some(*expected));
            d.reset();
        }
    }

    #[test]
    fn decode_empty_returns_none() {
        let d = MorseDecoder::new();
        assert_eq!(d.decode(), None);
    }

    #[test]
    fn decode_overflow_returns_none() {
        let mut d = MorseDecoder::new();
        super::push_seq(&mut d, &[Pulse::Dit; 6]);
        assert_eq!(d.decode(), None);
    }

    #[test]
    fn decode_reset() {
        let mut d = MorseDecoder::new();
        super::push_seq(&mut d, &[Pulse::Dah, Pulse::Dah, Pulse::Dah]); // O
        assert_eq!(d.decode(), Some('O'));
        d.reset();
        assert_eq!(d.decode(), None);
        super::push_seq(&mut d, &[Pulse::Dit]); // E
        assert_eq!(d.decode(), Some('E'));
    }

    #[test]
    fn decode_word_paris() {
        use Pulse::{Dah, Dit};
        let word: &[(&[Pulse], char)] = &[
            (&[Dit, Dah, Dah, Dit], 'P'),
            (&[Dit, Dah],           'A'),
            (&[Dit, Dah, Dit],      'R'),
            (&[Dit, Dit],           'I'),
            (&[Dit, Dit, Dit],      'S'),
        ];
        let mut d = MorseDecoder::new();
        for (seq, expected) in word {
            super::push_seq(&mut d, seq);
            assert_eq!(d.decode(), Some(*expected));
            d.reset();
        }
    }
}
