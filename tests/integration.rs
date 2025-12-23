#![no_std]
#![no_main]

// SPDX-FileCopyrightText: 2025 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use stm32_rust as _; // memory layout + panic handler

// See https://crates.io/crates/defmt-test/0.3.0 for more documentation (e.g. about the 'state'
// feature)
#[defmt_test::tests]
mod tests {
    use defmt::assert;

    #[test]
    fn it_works() {
        assert!(true)
    }
}
