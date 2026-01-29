#![no_main]
#![no_std]

// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use defmt::*;
use defmt_rtt as _; // global logger
use embassy_stm32::{gpio::Output, peripherals::TIM3, timer::simple_pwm::SimplePwmChannel};
use embassy_time::Timer;
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
pub enum PulseMode {
    #[default]
    Repeating,
    Alternating,
}

#[derive(Debug, Default, PartialEq, Clone, Copy)]
pub enum PulseType {
    #[default]
    Normal,
    Residual,
}

#[derive(Default)]
pub struct Keyer {
    current_pulse: Option<Pulse>,
    pulse_type: PulseType,
    pulse_mode: PulseMode,
    mode: IambicMode,
}

impl Keyer {
    pub fn new(mode: IambicMode) -> Self {
        Keyer {
            current_pulse: None,
            pulse_type: PulseType::Normal,
            pulse_mode: PulseMode::Repeating,
            mode,
        }
    }

    pub fn update(&mut self, input: Option<PaddleInput>) -> Option<Pulse> {
        self.current_pulse = match (input, self.current_pulse) {
            (Some(PaddleInput::DitOnly), _) => {
                self.pulse_mode = PulseMode::Repeating;
                Some(Pulse::Dit)
            }
            (Some(PaddleInput::DahOnly), _) => {
                self.pulse_mode = PulseMode::Repeating;
                Some(Pulse::Dah)
            }

            // Toggle
            (Some(PaddleInput::Both), None) => {
                self.pulse_mode = PulseMode::Alternating;
                Some(Pulse::Dah)
            }
            (Some(PaddleInput::Both), Some(p)) => {
                self.pulse_mode = PulseMode::Alternating;
                Some(p.toggle())
            }

            // Iambic B - add a residual pulse after key up
            (None, Some(p))
                if self.mode == IambicMode::B && self.pulse_mode == PulseMode::Alternating =>
            {
                match self.pulse_type {
                    // Residual pulse
                    PulseType::Normal => {
                        self.pulse_type = PulseType::Residual;
                        Some(p.toggle())
                    }
                    // Clear residual
                    PulseType::Residual => {
                        self.pulse_type = PulseType::Normal;
                        None
                    }
                }
            }
            (None, _) => None,
        };

        self.current_pulse
    }
}

pub async fn send_element(
    led: &mut Output<'_>,
    buzzer_act: &mut Output<'_>,
    buzzer_pass: &mut SimplePwmChannel<'_, TIM3>,
    unit: u64,
    pulse: Pulse,
) {
    let duration = pulse.duration(unit);

    info!("P {}", duration);

    led.set_low(); // Key down
    buzzer_act.set_high(); // Key down
    buzzer_pass.set_duty_cycle_percent(2);

    Timer::after_millis(duration).await;

    led.set_high(); // Key up
    buzzer_act.set_low(); // Key up
    buzzer_pass.set_duty_cycle_fully_off();

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
