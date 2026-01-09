//! TM1637 7-segment display driver

use rppal::gpio::{Gpio, OutputPin};
use std::thread::sleep;
use std::time::Duration;

const DELAY_US: u64 = 2;

// Segment patterns for digits 0-9, blank, and dash
const SEGMENTS: [u8; 12] = [
    0x3F, // 0
    0x06, // 1
    0x5B, // 2
    0x4F, // 3
    0x66, // 4
    0x6D, // 5
    0x7D, // 6
    0x07, // 7
    0x7F, // 8
    0x6F, // 9
    0x00, // 10: blank
    0x40, // 11: dash
];

pub struct TM1637 {
    clk: OutputPin,
    dio: OutputPin,
    brightness: u8,
}

impl TM1637 {
    pub fn new(gpio: &Gpio, clk_pin: u8, dio_pin: u8) -> Result<Self, rppal::gpio::Error> {
        let clk = gpio.get(clk_pin)?.into_output_high();
        let dio = gpio.get(dio_pin)?.into_output_high();
        
        Ok(Self {
            clk,
            dio,
            brightness: 3,
        })
    }

    pub fn set_brightness(&mut self, level: u8) {
        self.brightness = level.min(7);
    }

    fn delay(&self) {
        sleep(Duration::from_micros(DELAY_US));
    }

    fn start(&mut self) {
        self.dio.set_low();
        self.delay();
    }

    fn stop(&mut self) {
        self.dio.set_low();
        self.delay();
        self.clk.set_high();
        self.delay();
        self.dio.set_high();
        self.delay();
    }

    fn write_byte(&mut self, data: u8) -> bool {
        let mut byte = data;
        
        for _ in 0..8 {
            self.clk.set_low();
            self.delay();
            
            if byte & 0x01 != 0 {
                self.dio.set_high();
            } else {
                self.dio.set_low();
            }
            
            self.delay();
            self.clk.set_high();
            self.delay();
            byte >>= 1;
        }
        
        // ACK
        self.clk.set_low();
        self.dio.set_high();
        self.delay();
        self.clk.set_high();
        self.delay();
        self.clk.set_low();
        
        true
    }

    /// Display 4 digits. Each value should be 0-9, 10 for blank, 11 for dash.
    /// Set decimal_pos to Some(0-3) to show decimal point at that position.
    pub fn display(&mut self, digits: [u8; 4], decimal_pos: Option<usize>) {
        self.start();
        self.write_byte(0x40); // Data command: write data, auto-increment
        self.stop();

        self.start();
        self.write_byte(0xC0); // Address command: start at 0

        for (i, &digit) in digits.iter().enumerate() {
            let mut segment = if digit < SEGMENTS.len() as u8 {
                SEGMENTS[digit as usize]
            } else {
                SEGMENTS[10] // blank for invalid
            };
            
            // Add decimal point if requested
            if decimal_pos == Some(i) {
                segment |= 0x80;
            }
            
            self.write_byte(segment);
        }
        self.stop();

        self.start();
        self.write_byte(0x88 | self.brightness); // Display control: on + brightness
        self.stop();
    }

    /// Display a frequency value (e.g., 1.5 shows as " 1.5" or "15" with decimal)
    pub fn display_frequency(&mut self, freq: f64) {
        // Display as freq * 10, with decimal after first digit from right
        // e.g., 1.5 Hz -> "0015" with decimal after position 2 -> "001.5"
        let freq_int = (freq * 10.0).round() as u32;
        let freq_clamped = freq_int.min(9999);
        
        let digits = [
            ((freq_clamped / 1000) % 10) as u8,
            ((freq_clamped / 100) % 10) as u8,
            ((freq_clamped / 10) % 10) as u8,
            (freq_clamped % 10) as u8,
        ];
        
        self.display(digits, Some(2)); // Decimal after 3rd digit
    }

    /// Display sequence number (1-8)
    pub fn display_sequence(&mut self, seq: usize, detailed: bool, sequence: &[usize; 4]) {
        if !detailed {
            // Show just the sequence number with decimal point
            let digit = (seq % 10) as u8;
            self.display([10, 10, 10, digit], Some(3));
        } else {
            // Show the actual sequence pattern (1-indexed for display)
            if seq == 0 {
                self.display([0, 0, 0, 0], None);
            } else if seq == 7 {
                self.display([1, 1, 1, 1], None);
            } else {
                let digits = [
                    (sequence[0] + 1) as u8,
                    (sequence[1] + 1) as u8,
                    (sequence[2] + 1) as u8,
                    (sequence[3] + 1) as u8,
                ];
                self.display(digits, None);
            }
        }
    }

    pub fn clear(&mut self) {
        self.display([10, 10, 10, 10], None);
    }
}
