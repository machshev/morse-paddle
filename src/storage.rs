// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

//! Persistent settings storage in the last flash page (0x0800_FC00).
//!
//! STM32F103C8: 64 KB flash, 1 KB pages.  The linker script reserves the
//! last page so code is never placed there.
//!
//! Layout (8 bytes, 2-byte aligned for half-word writes):
//!   [0..2]  magic  = 0xAB42 (u16 LE) – marks page as valid
//!   [2]     volume_idx
//!   [3]     keyer_mode  (0 = A, 1 = B)
//!   [4]     wpm_idx
//!   [5]     pitch_idx
//!   [6..8]  0xFF padding

use embassy_stm32::flash::{Blocking, Flash};

use crate::IambicMode;
use crate::menu::{Settings, PITCH_LEVELS, VOLUME_LEVELS, WPM_LEVELS};

const PAGE_SIZE: u32 = 1024;
const SETTINGS_OFFSET: u32 = 64 * 1024 - PAGE_SIZE; // 0xFC00
const MAGIC: u16 = 0xAB42;

pub fn load(flash: &mut Flash<'_, Blocking>) -> Option<Settings> {
    let mut buf = [0u8; 8];
    flash.blocking_read(SETTINGS_OFFSET, &mut buf).ok()?;

    if u16::from_le_bytes([buf[0], buf[1]]) != MAGIC {
        return None;
    }

    let volume_idx = buf[2] as usize;
    let keyer_mode = if buf[3] == 0 { IambicMode::A } else { IambicMode::B };
    let wpm_idx = buf[4] as usize;
    let pitch_idx = buf[5] as usize;

    if volume_idx >= VOLUME_LEVELS.len()
        || wpm_idx >= WPM_LEVELS.len()
        || pitch_idx >= PITCH_LEVELS.len()
    {
        return None;
    }

    Some(Settings { volume_idx, keyer_mode, wpm_idx, pitch_idx })
}

pub fn save(flash: &mut Flash<'_, Blocking>, settings: &Settings) {
    let magic = MAGIC.to_le_bytes();
    let data = [
        magic[0],
        magic[1],
        settings.volume_idx as u8,
        match settings.keyer_mode {
            IambicMode::A => 0,
            IambicMode::B => 1,
        },
        settings.wpm_idx as u8,
        settings.pitch_idx as u8,
        0xFF,
        0xFF,
    ];
    let _ = flash.blocking_erase(SETTINGS_OFFSET, SETTINGS_OFFSET + PAGE_SIZE);
    let _ = flash.blocking_write(SETTINGS_OFFSET, &data);
}
