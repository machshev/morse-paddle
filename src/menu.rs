// SPDX-FileCopyrightText: 2026 David James McCorrie <djmccorrie@gmail.com>
//
// SPDX-License-Identifier: Apache-2.0

use crate::IambicMode;

pub const VOLUME_LEVELS: [u8; 5] = [1, 5, 10, 20, 40];
const VOLUME_LABELS: [&str; 5] = ["1", "2", "3", "4", "5"];

pub const WPM_LEVELS: [u64; 6] = [5, 10, 15, 20, 25, 30];
const WPM_LABELS: [&str; 6] = ["5", "10", "15", "20", "25", "30"];

pub const PITCH_LEVELS: [u32; 6] = [600, 700, 800, 900, 1000, 1200];
const PITCH_LABELS: [&str; 6] = ["600", "700", "800", "900", "1K", "1.2K"];

// At WPM=20: tick = UNIT_MS/10 = 6 ms → 833 ticks ≈ 5 seconds.
// Scales naturally with WPM since tick duration is always UNIT_MS/10.
const MENU_TIMEOUT_TICKS: u32 = 833;

#[derive(Clone, Copy)]
pub enum MenuItem {
    Tutor,
    Volume,
    KeyerMode,
    Wpm,
    Pitch,
}

impl MenuItem {
    fn next(self) -> Self {
        match self {
            MenuItem::Tutor => MenuItem::Volume,
            MenuItem::Volume => MenuItem::KeyerMode,
            MenuItem::KeyerMode => MenuItem::Wpm,
            MenuItem::Wpm => MenuItem::Pitch,
            MenuItem::Pitch => MenuItem::Tutor,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            MenuItem::Tutor => "TUTOR",
            MenuItem::Volume => "VOLUME",
            MenuItem::KeyerMode => "KEYER",
            MenuItem::Wpm => "WPM",
            MenuItem::Pitch => "PITCH",
        }
    }
}

pub struct Settings {
    pub volume_idx: usize,
    pub keyer_mode: IambicMode,
    pub wpm_idx: usize,
    pub pitch_idx: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self::new()
    }
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            volume_idx: 1,  // VOLUME_LEVELS[1] = 5% duty cycle (hardware default)
            keyer_mode: IambicMode::B,
            wpm_idx: 3,     // WPM_LEVELS[3] = 20 WPM
            pitch_idx: 4,   // PITCH_LEVELS[4] = 1000 Hz
        }
    }

    pub fn volume_percent(&self) -> u8 {
        VOLUME_LEVELS[self.volume_idx]
    }

    pub fn unit_ms(&self) -> u64 {
        1200 / WPM_LEVELS[self.wpm_idx]
    }

    pub fn pitch_hz(&self) -> u32 {
        PITCH_LEVELS[self.pitch_idx]
    }
}

pub struct Menu {
    pub item: MenuItem,
    pub tutor_on: bool,
    pub display_dirty: bool,
    /// Set when volume or pitch changes so the caller can play a preview tone.
    pub sound_preview: bool,
    last_dit: bool,
    last_dah: bool,
    idle_ticks: u32,
}

impl Menu {
    /// `tutor_currently_active` pre-selects TUTOR=ON so the user sees the
    /// current state immediately upon entering the menu from tutor mode.
    pub fn new(tutor_currently_active: bool) -> Self {
        Menu {
            item: MenuItem::Tutor,
            tutor_on: tutor_currently_active,
            display_dirty: true,
            sound_preview: false,
            last_dit: false,
            last_dah: false,
            idle_ticks: 0,
        }
    }

    /// Process raw paddle state for one tick.
    /// Returns `true` when the idle timeout fires and the menu should close.
    pub fn update(&mut self, dit_pressed: bool, dah_pressed: bool, settings: &mut Settings) -> bool {
        let dit_edge = dit_pressed && !self.last_dit;
        let dah_edge = dah_pressed && !self.last_dah;

        if dit_edge {
            self.item = self.item.next();
            self.display_dirty = true;
            self.sound_preview = false;
        }
        if dah_edge {
            self.adjust(settings);
            self.display_dirty = true;
        }

        self.last_dit = dit_pressed;
        self.last_dah = dah_pressed;

        if dit_pressed || dah_pressed {
            self.idle_ticks = 0;
        } else {
            self.idle_ticks += 1;
        }

        self.idle_ticks >= MENU_TIMEOUT_TICKS
    }

    fn adjust(&mut self, settings: &mut Settings) {
        match self.item {
            MenuItem::Tutor => {
                self.tutor_on = !self.tutor_on;
            }
            MenuItem::Volume => {
                settings.volume_idx = (settings.volume_idx + 1) % VOLUME_LEVELS.len();
                self.sound_preview = true;
            }
            MenuItem::KeyerMode => {
                settings.keyer_mode = match settings.keyer_mode {
                    IambicMode::A => IambicMode::B,
                    IambicMode::B => IambicMode::A,
                };
            }
            MenuItem::Wpm => {
                settings.wpm_idx = (settings.wpm_idx + 1) % WPM_LEVELS.len();
            }
            MenuItem::Pitch => {
                settings.pitch_idx = (settings.pitch_idx + 1) % PITCH_LEVELS.len();
                self.sound_preview = true;
            }
        }
    }

    pub fn value_label(&self, settings: &Settings) -> &'static str {
        match self.item {
            MenuItem::Tutor => {
                if self.tutor_on { "ON" } else { "OFF" }
            }
            MenuItem::Volume => VOLUME_LABELS[settings.volume_idx],
            MenuItem::KeyerMode => match settings.keyer_mode {
                IambicMode::A => "A",
                IambicMode::B => "B",
            },
            MenuItem::Wpm => WPM_LABELS[settings.wpm_idx],
            MenuItem::Pitch => PITCH_LABELS[settings.pitch_idx],
        }
    }
}
