//! Rotary encoder driver with button support

use rppal::gpio::{Gpio, InputPin, Level, Trigger};
use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct RotaryEncoder {
    _clk: InputPin,
    _dt: InputPin,
    steps: Arc<AtomicI32>,
}

impl RotaryEncoder {
    pub fn new(gpio: &Gpio, clk_pin: u8, dt_pin: u8) -> Result<Self, rppal::gpio::Error> {
        let steps = Arc::new(AtomicI32::new(0));
        let steps_clone = Arc::clone(&steps);
        
        let mut clk = gpio.get(clk_pin)?.into_input_pullup();
        let dt = gpio.get(dt_pin)?.into_input_pullup();
        
        let dt_pin_ref = gpio.get(dt_pin)?.into_input_pullup();
        
        // Set up interrupt on CLK falling edge (None = no reset timeout)
        clk.set_async_interrupt(Trigger::FallingEdge, None, move |_event| {
            // Read DT to determine direction
            if dt_pin_ref.read() == Level::High {
                steps_clone.fetch_add(1, Ordering::SeqCst);
            } else {
                steps_clone.fetch_sub(1, Ordering::SeqCst);
            }
        })?;

        Ok(Self {
            _clk: clk,
            _dt: dt,
            steps,
        })
    }

    pub fn steps(&self) -> i32 {
        self.steps.load(Ordering::SeqCst)
    }
}

pub struct Button {
    _pin: InputPin,
    pressed: Arc<AtomicBool>,
    press_time: Arc<std::sync::Mutex<Option<Instant>>>,
}

impl Button {
    pub fn new(gpio: &Gpio, pin: u8) -> Result<Self, rppal::gpio::Error> {
        let pressed = Arc::new(AtomicBool::new(false));
        let press_time = Arc::new(std::sync::Mutex::new(None::<Instant>));
        
        let pressed_clone = Arc::clone(&pressed);
        let press_time_clone = Arc::clone(&press_time);
        
        let mut input = gpio.get(pin)?.into_input_pullup();
        
        // None = no reset timeout
        // The callback receives an Event with a trigger field
        input.set_async_interrupt(Trigger::Both, None, move |event| {
            match event.trigger {
                Trigger::FallingEdge => {
                    // Button pressed (active low - falling edge)
                    pressed_clone.store(true, Ordering::SeqCst);
                    *press_time_clone.lock().unwrap() = Some(Instant::now());
                }
                Trigger::RisingEdge => {
                    // Button released (rising edge)
                    pressed_clone.store(false, Ordering::SeqCst);
                }
                _ => {}
            }
        })?;

        Ok(Self {
            _pin: input,
            pressed,
            press_time,
        })
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed.load(Ordering::SeqCst)
    }

    /// Returns how long the button has been held (if currently pressed)
    pub fn hold_duration(&self) -> Option<Duration> {
        if self.is_pressed() {
            self.press_time.lock().unwrap().map(|t| t.elapsed())
        } else {
            None
        }
    }

    /// Check if button was just released and return press duration
    pub fn get_press_duration(&self) -> Option<Duration> {
        if !self.is_pressed() {
            if let Some(time) = self.press_time.lock().unwrap().take() {
                return Some(time.elapsed());
            }
        }
        None
    }
}
