// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use crate::Pulse;

/// ITU-R Morse code lookup table indexed by the accumulated bit pattern.
///
/// Encoding: start with accumulator = 1, then for each element shift left
/// and OR in 0 (dit) or 1 (dah).  The final accumulator value is the table
/// index.  0 means "not assigned".
///
/// Examples:
///   E (.)   → 1 → 2
///   T (-)   → 1 → 3
///   A (.-)  → 1 → 2 → 5
///   K (-.-) → 1 → 3 → 6 → 13
const MORSE_TABLE: [u8; 64] = [
    0,    // 0   invalid
    0,    // 1   empty (start bit only)
    b'E', // 2   .
    b'T', // 3   -
    b'I', // 4   ..
    b'A', // 5   .-
    b'N', // 6   -.
    b'M', // 7   --
    b'S', // 8   ...
    b'U', // 9   ..-
    b'R', // 10  .-.
    b'W', // 11  .--
    b'D', // 12  -..
    b'K', // 13  -.-
    b'G', // 14  --.
    b'O', // 15  ---
    b'H', // 16  ....
    b'V', // 17  ...-
    b'F', // 18  ..-.
    0,    // 19  ..--
    b'L', // 20  .-..
    0,    // 21  .-.-
    b'P', // 22  .--.
    b'J', // 23  .---
    b'B', // 24  -...
    b'X', // 25  -..-
    b'C', // 26  -.-.
    b'Y', // 27  -.--
    b'Z', // 28  --..
    b'Q', // 29  --.-
    0,    // 30  ---.
    0,    // 31  ----
    b'5', // 32  .....
    b'4', // 33  ....-
    0,    // 34  ...-. (not standard)
    b'3', // 35  ...--
    0,    // 36  ..-.. (not standard)
    0,    // 37  ..-.- (not standard)
    0,    // 38  ..--. (not standard)
    b'2', // 39  ..---
    0,    // 40  .-... (not standard)
    0,    // 41  .-..- (not standard)
    0,    // 42  .-.-. (not standard)
    0,    // 43  .-.-- (not standard)
    0,    // 44  .--.. (not standard)
    0,    // 45  .--.- (not standard)
    0,    // 46  .---. (not standard)
    b'1', // 47  .----
    b'6', // 48  -....
    0,    // 49  -...- (not standard)
    0,    // 50  -..-. (not standard)
    0,    // 51  -..-- (not standard)
    0,    // 52  -.-.. (not standard)
    0,    // 53  -.-.- (not standard)
    0,    // 54  -.--. (not standard)
    0,    // 55  -.--- (not standard)
    b'7', // 56  --...
    0,    // 57  --..- (not standard)
    0,    // 58  --.-. (not standard)
    0,    // 59  --.-- (not standard)
    b'8', // 60  ---..
    0,    // 61  ---.- (not standard)
    b'9', // 62  ----.
    b'0', // 63  -----
];

/// Accumulates [`Pulse`] elements and decodes them to an ASCII character.
///
/// Reset between characters with [`MorseDecoder::reset`].  Up to 5 elements
/// (covering the full ITU-R alphabet and digit set) are supported; additional
/// elements are silently ignored and [`MorseDecoder::decode`] will return
/// `None`.
pub struct MorseDecoder {
    accumulator: u8,
}

impl MorseDecoder {
    pub fn new() -> Self {
        MorseDecoder { accumulator: 1 }
    }

    /// Push the next element of the current character.
    pub fn push(&mut self, pulse: Pulse) {
        if self.accumulator >= 64 {
            // Already marked as overflowed; ignore further input.
            return;
        }
        if self.accumulator >= 32 {
            // A 6th element would exceed the 64-entry table; mark as overflow.
            self.accumulator = 0xFF;
            return;
        }
        self.accumulator = (self.accumulator << 1)
            | match pulse {
                Pulse::Dit => 0,
                Pulse::Dah => 1,
            };
    }

    /// Decode the accumulated elements to a character, or `None` if the
    /// sequence is empty, too long, or not in the ITU-R table.
    pub fn decode(&self) -> Option<char> {
        if self.accumulator < 2 || self.accumulator >= 64 {
            return None;
        }
        let c = MORSE_TABLE[self.accumulator as usize];
        if c == 0 { None } else { Some(c as char) }
    }

    /// Clear the accumulator, ready for the next character.
    pub fn reset(&mut self) {
        self.accumulator = 1;
    }
}
