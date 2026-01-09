//! Switch Charging Sequence Controller
//! 
//! High-performance MOSFET switching controller for Raspberry Pi 5
//! Supports frequencies up to several kHz with precise timing

mod tm1637;
mod rotary;

use rppal::gpio::{Gpio, OutputPin, Level};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::thread;

use tm1637::TM1637;
use rotary::{RotaryEncoder, Button};

// ============================================================================
// Configuration
// ============================================================================

// Frequency limits (Hz)
const MAX_FREQ: f64 = 10_000.0;  // Can now go to 10kHz!
const MIN_FREQ: f64 = 0.1;
const DEFAULT_FREQ: f64 = 1.0;

// GPIO Pin assignments
const PIN_P1: u8 = 17;  // High-side MOSFET 1
const PIN_P2: u8 = 27;  // High-side MOSFET 2
const PIN_N1: u8 = 22;  // Low-side MOSFET 1
const PIN_N2: u8 = 23;  // Low-side MOSFET 2

const PIN_DISPLAY_CLK: u8 = 18;
const PIN_DISPLAY_DIO: u8 = 24;

const PIN_ROTARY_CLK: u8 = 16;
const PIN_ROTARY_DT: u8 = 20;
const PIN_ROTARY_BTN: u8 = 21;

// Timing
const HOLD_TIME: Duration = Duration::from_millis(500);
const FREQ_TOGGLE_HYSTERESIS: Duration = Duration::from_millis(1500);

// ============================================================================
// MOSFET State Definitions
// ============================================================================

/// MOSFET state: (P1, P2, N1, N2) - true = ON
#[derive(Clone, Copy, Debug)]
struct MosfetState {
    p1: bool,
    p2: bool,
    n1: bool,
    n2: bool,
}

impl MosfetState {
    const fn new(p1: bool, p2: bool, n1: bool, n2: bool) -> Self {
        Self { p1, p2, n1, n2 }
    }
}

const STATE_DEFINITIONS: [MosfetState; 6] = [
    MosfetState::new(true,  false, true,  false), // 0: P1-N1
    MosfetState::new(true,  false, false, true),  // 1: P1-N2
    MosfetState::new(false, true,  true,  false), // 2: P2-N1
    MosfetState::new(false, true,  false, true),  // 3: P2-N2
    MosfetState::new(true,  true,  true,  true),  // 4: All ON
    MosfetState::new(false, false, false, false), // 5: All OFF
];

/// Allowed switching sequences (indices into STATE_DEFINITIONS)
const SEQUENCES: [[usize; 4]; 8] = [
    [5, 5, 5, 5], // All OFF
    [0, 1, 2, 3], // Standard rotation
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 2, 3, 1],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [4, 4, 4, 4], // All ON
];

// ============================================================================
// MOSFET Controller
// ============================================================================

struct MosfetController {
    p1: OutputPin,
    p2: OutputPin,
    n1: OutputPin,
    n2: OutputPin,
}

impl MosfetController {
    fn new(gpio: &Gpio) -> Result<Self, rppal::gpio::Error> {
        // P-channel MOSFETs are active-low, N-channel are active-high
        let mut p1 = gpio.get(PIN_P1)?.into_output_low();
        let mut p2 = gpio.get(PIN_P2)?.into_output_low();
        let mut n1 = gpio.get(PIN_N1)?.into_output_low();
        let mut n2 = gpio.get(PIN_N2)?.into_output_low();

        // Start with all OFF
        p1.set_high(); // P-channel off
        p2.set_high();
        n1.set_low();  // N-channel off
        n2.set_low();

        Ok(Self { p1, p2, n1, n2 })
    }

    /// Apply a MOSFET state - optimized for speed
    #[inline(always)]
    fn apply_state(&mut self, state: MosfetState) {
        // P-channel: active-low (true = low = on)
        self.p1.write(if state.p1 { Level::Low } else { Level::High });
        self.p2.write(if state.p2 { Level::Low } else { Level::High });
        
        // N-channel: active-high (true = high = on)
        self.n1.write(if state.n1 { Level::High } else { Level::Low });
        self.n2.write(if state.n2 { Level::High } else { Level::Low });
    }

    fn all_off(&mut self) {
        self.apply_state(STATE_DEFINITIONS[5]);
    }
}

// ============================================================================
// Application State
// ============================================================================

struct AppState {
    frequency: f64,
    step_rate: u32,          // 1 or 10
    sequence_sel: usize,     // 0-7
    sequence_mode: bool,
    sequence_display_detailed: bool,
    
    dial_offset: i32,
    freq_base: f64,
    sequence_mode_base: i32,
    
    last_freq_toggle: Instant,
    current_step: usize,
    last_step_time: Instant,
    last_rotary_steps: i32,
    
    cached_display: Option<[u8; 4]>,
}

impl AppState {
    fn new() -> Self {
        Self {
            frequency: DEFAULT_FREQ,
            step_rate: 1,
            sequence_sel: 1,
            sequence_mode: false,
            sequence_display_detailed: false,
            
            dial_offset: 0,
            freq_base: DEFAULT_FREQ,
            sequence_mode_base: 0,
            
            last_freq_toggle: Instant::now(),
            current_step: 0,
            last_step_time: Instant::now(),
            last_rotary_steps: 0,
            
            cached_display: None,
        }
    }

    fn step_time(&self) -> Duration {
        let period = 1.0 / self.frequency;
        Duration::from_secs_f64(period / 2.0)
    }
}

// ============================================================================
// Main Application
// ============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    log::info!("Switch Charging Controller starting...");

    let gpio = Gpio::new()?;
    
    // Initialize components
    let mut mosfets = MosfetController::new(&gpio)?;
    let mut display = TM1637::new(&gpio, PIN_DISPLAY_CLK, PIN_DISPLAY_DIO)?;
    let rotary = RotaryEncoder::new(&gpio, PIN_ROTARY_CLK, PIN_ROTARY_DT)?;
    let button = Button::new(&gpio, PIN_ROTARY_BTN)?;
    
    display.set_brightness(3);
    
    let mut state = AppState::new();
    state.dial_offset = rotary.steps();
    state.last_rotary_steps = rotary.steps();

    // Setup graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = Arc::clone(&running);
    
    ctrlc::set_handler(move || {
        log::info!("Shutdown signal received");
        running_clone.store(false, Ordering::SeqCst);
    })?;

    log::info!("Controller ready. Frequency: {} Hz, Sequence: {}", 
               state.frequency, state.sequence_sel);

    // Track button state for hold detection
    let mut button_was_pressed = false;
    let mut button_press_start: Option<Instant> = None;
    let mut mode_toggled_this_press = false;

    // Main loop
    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        let current_steps = rotary.steps();

        // Button handling
        let button_pressed = button.is_pressed();
        
        if button_pressed && !button_was_pressed {
            // Button just pressed
            button_press_start = Some(now);
            mode_toggled_this_press = false;
        }
        
        if button_pressed {
            // Check for hold to toggle mode
            if let Some(start) = button_press_start {
                if !mode_toggled_this_press && now.duration_since(start) >= HOLD_TIME {
                    // Toggle mode
                    state.sequence_mode = !state.sequence_mode;
                    state.dial_offset = current_steps;
                    state.freq_base = state.frequency;
                    mode_toggled_this_press = true;
                    
                    if state.sequence_mode {
                        log::info!("Entered sequence selector mode");
                        state.sequence_mode_base = 0;
                        mosfets.all_off();
                    } else {
                        log::info!("Exited sequence selector mode");
                    }
                }
            }
        }
        
        if !button_pressed && button_was_pressed {
            // Button just released
            if let Some(start) = button_press_start {
                let duration = now.duration_since(start);
                
                // Short press handling (only if we didn't toggle mode)
                if !mode_toggled_this_press 
                   && duration < HOLD_TIME 
                   && now.duration_since(state.last_freq_toggle) >= FREQ_TOGGLE_HYSTERESIS 
                {
                    if state.sequence_mode {
                        state.sequence_display_detailed = !state.sequence_display_detailed;
                        log::info!("Sequence display detail: {}", state.sequence_display_detailed);
                    } else {
                        state.step_rate = if state.step_rate == 1 { 10 } else { 1 };
                        log::info!("Frequency step rate: x{}", state.step_rate);
                    }
                    state.last_freq_toggle = now;
                }
                
                // Reset dial offset on release
                state.dial_offset = current_steps;
                state.freq_base = state.frequency;
            }
            button_press_start = None;
        }
        
        button_was_pressed = button_pressed;

        if !state.sequence_mode {
            // === FREQUENCY MODE ===
            
            // Handle rotary changes
            if current_steps != state.last_rotary_steps {
                let increment = if state.step_rate == 1 { 0.1 } else { 1.0 };
                let delta = (current_steps - state.dial_offset) as f64 * increment;
                state.frequency = (state.freq_base + delta).clamp(MIN_FREQ, MAX_FREQ);
                
                log::info!("Frequency: {:.1} Hz", state.frequency);
                state.last_rotary_steps = current_steps;
            }

            // Execute switching at the correct rate
            if now.duration_since(state.last_step_time) >= state.step_time() {
                let sequence = &SEQUENCES[state.sequence_sel];
                let state_index = sequence[state.current_step];
                
                mosfets.apply_state(STATE_DEFINITIONS[state_index]);
                
                state.last_step_time = now;
                state.current_step = (state.current_step + 1) % 4;
            }
        } else {
            // === SEQUENCE MODE ===
            
            if current_steps != state.last_rotary_steps {
                let delta = current_steps - state.dial_offset - state.sequence_mode_base;
                let new_sel = ((1 + delta).rem_euclid(SEQUENCES.len() as i32)) as usize;
                state.sequence_sel = if new_sel == 0 { SEQUENCES.len() } else { new_sel };
                
                log::info!("Sequence: {} -> {:?}", 
                          state.sequence_sel, SEQUENCES[state.sequence_sel - 1]);
                state.last_rotary_steps = current_steps;
            }
            
            // Small sleep in sequence mode to reduce CPU
            thread::sleep(Duration::from_micros(100));
        }

        // Update display
        update_display(&state, &mut display);
    }

    // Cleanup
    log::info!("Shutting down...");
    mosfets.all_off();
    display.clear();

    Ok(())
}

fn update_display(state: &AppState, display: &mut TM1637) {
    if !state.sequence_mode {
        display.display_frequency(state.frequency);
    } else {
        let seq = SEQUENCES[state.sequence_sel - 1];
        let seq_arr: [usize; 4] = [seq[0], seq[1], seq[2], seq[3]];
        display.display_sequence(
            state.sequence_sel,
            state.sequence_display_detailed,
            &seq_arr,
        );
    }
}
