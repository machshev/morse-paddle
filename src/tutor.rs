// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use crate::{KeyOutput, MorseDisplay, decoder::char_to_pulses};
use defmt::info;
use display_interface::AsyncWriteOnlyDataCommand;
use embassy_time::Timer;
use embedded_hal::{digital::OutputPin, pwm::SetDutyCycle};


const TUTOR_LETTERS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const MAX_ATTEMPTS: u8 = 3;

fn lcg_next(state: &mut u32) -> u32 {
    *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    *state
}

fn pick(rng: &mut u32) -> char {
    TUTOR_LETTERS[(lcg_next(rng) as usize) % TUTOR_LETTERS.len()] as char
}

/// Encapsulates tutor mode state: the current target letter, attempt counter,
/// and the PRNG used to pick the next random letter.
pub struct Tutor {
    pub target: char,
    attempts: u8,
    rng: u32,
}

impl Tutor {
    /// Create a new tutor session.  `rng_seed` should be varied (e.g. derived
    /// from uptime) so successive sessions aren't identical.
    pub fn new(rng_seed: u32) -> Self {
        let mut rng = rng_seed;
        let target = pick(&mut rng);
        Tutor {
            target,
            attempts: 0,
            rng,
        }
    }

    /// Process a character keyed by the user.
    ///
    /// * Correct → advance to the next random letter and show it.
    /// * Wrong (< MAX_ATTEMPTS) → re-display the same letter.
    /// * Wrong (= MAX_ATTEMPTS) → play the correct Morse sequence as a hint,
    ///   reset the attempt counter, then re-display the same letter.
    pub async fn check<L, A, P, R, DI>(
        &mut self,
        ch: char,
        key_output: &mut KeyOutput<L, A, P, R>,
        display: &mut MorseDisplay<DI>,
        unit_ms: u64,
    ) where
        L: OutputPin,
        A: OutputPin,
        P: SetDutyCycle,
        R: OutputPin,
        DI: AsyncWriteOnlyDataCommand,
    {
        if ch == self.target {
            info!("Tutor: correct!");
            self.target = pick(&mut self.rng);
            self.attempts = 0;
        } else {
            self.attempts += 1;
            info!("Tutor: wrong ({}/{})", self.attempts, MAX_ATTEMPTS);
            if self.attempts >= MAX_ATTEMPTS {
                info!("Tutor: playing hint for {}", self.target as u8);
                if let Some((pulses, len)) = char_to_pulses(self.target) {
                    display.show_tutor_message("listen...").await;
                    Timer::after_millis(1_000).await;
                    for pulse in pulses.iter().take(len) {
                        key_output.send(*pulse, unit_ms).await;
                    }
                    Timer::after_millis(unit_ms * 2).await;
                }
                self.attempts = 0;
            }
        }
        // Always show current target (either the new letter or the same one again).
        display.show_tutor_char(self.target).await;
    }
}
