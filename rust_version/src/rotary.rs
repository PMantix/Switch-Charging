//! Rotary encoder driver with button support

use rppal::gpio::{Gpio, InputPin, Level, Trigger};
use std::sync::atomic::{AtomicI32, AtomicBool, Ordering};
use std::sync::Arc;

pub struct RotaryEncoder {
    _clk: InputPin,
    steps: Arc<AtomicI32>,
}

impl RotaryEncoder {
    pub fn new(gpio: &Gpio, clk_pin: u8, dt_pin: u8) -> Result<Self, rppal::gpio::Error> {
        let steps = Arc::new(AtomicI32::new(0));
        let steps_clone = Arc::clone(&steps);
        
        let mut clk = gpio.get(clk_pin)?.into_input_pullup();
        let dt = gpio.get(dt_pin)?.into_input_pullup();
        
        // Set up interrupt on CLK falling edge (None = no reset timeout)
        clk.set_async_interrupt(Trigger::FallingEdge, None, move |_event| {
            // Read DT to determine direction
            if dt.read() == Level::High {
                steps_clone.fetch_add(1, Ordering::SeqCst);
            } else {
                steps_clone.fetch_sub(1, Ordering::SeqCst);
            }
        })?;

        Ok(Self {
            _clk: clk,
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
}

impl Button {
    pub fn new(gpio: &Gpio, pin: u8) -> Result<Self, rppal::gpio::Error> {
        let pressed = Arc::new(AtomicBool::new(false));
        let pressed_clone = Arc::clone(&pressed);
        
        let mut input = gpio.get(pin)?.into_input_pullup();
        
        // None = no reset timeout
        // The callback receives an Event with a trigger field
        input.set_async_interrupt(Trigger::Both, None, move |event| {
            match event.trigger {
                Trigger::FallingEdge => {
                    // Button pressed (active low - falling edge)
                    pressed_clone.store(true, Ordering::SeqCst);
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
        })
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed.load(Ordering::SeqCst)
    }
}
