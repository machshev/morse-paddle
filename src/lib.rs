#![no_main]
#![no_std]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use defmt_rtt as _; // global logger
use embassy_stm32::gpio::Output;
use embassy_time::Timer;
use embedded_hal::pwm::SetDutyCycle;
use panic_probe as _;

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

pub async fn send_element<P: SetDutyCycle>(
    led: &mut Output<'_>,
    buzzer_act: &mut Output<'_>,
    buzzer_pass: &mut P,
    unit: u64,
    pulse: Pulse,
) {
    let duration = pulse.duration(unit);

    info!("P {}", duration);

    led.set_low(); // Key down
    buzzer_act.set_high(); // Key down
    buzzer_pass.set_duty_cycle_percent(30).unwrap();

    Timer::after_millis(duration).await;

    led.set_high(); // Key up
    buzzer_act.set_low(); // Key up
    buzzer_pass.set_duty_cycle_fully_off().unwrap();

    Timer::after_millis(unit).await; // Inter-element spacing
}

#[cfg(test)]
#[defmt_test::tests]
mod tests {
    use super::*;

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
}
