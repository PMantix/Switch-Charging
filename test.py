from gpiozero import RotaryEncoder, Button, OutputDevice
from tm1637 import TM1637
from time import sleep, time

# Frequency limits (Hz)
max_freq = 300
min_freq = 0.1

# --- Define MOSFETs ---
P1 = OutputDevice(17, active_high=False)
P2 = OutputDevice(27, active_high=False)
N1 = OutputDevice(22, active_high=True)
N2 = OutputDevice(23, active_high=True)

# --- Fixed states ---
state_definitions = [
    (True,  False, True,  False),
    (True,  False, False, True),
    (False, True,  True,  False),
    (False, True,  False, True),
    (True,  True,  True,  True),
    (False, False, False, False)
]

# --- Allowed sequences ---
sequences = [
    [5, 5, 5, 5],
    [0, 1, 2, 3],
    [0, 1, 3, 2],
    [0, 2, 1, 3],
    [0, 2, 3, 1],
    [0, 3, 1, 2],
    [0, 3, 2, 1],
    [4, 4, 4, 4]
]

# --- Setup TM1637 Display ---
CLK_PIN = 18
DIO_PIN = 24
display = TM1637(clk=CLK_PIN, dio=DIO_PIN)
display.set_brightness(3)
if len(display._segments) < 11:
    display._segments.append(0x40)							   					  
BLANK = 10

decimal_start = len(display._segments)
for i in range(10):
    display._segments.append(display._segments[i] | 0x80)

# --- Setup Rotary Encoder and Button ---
ROTARY_CLK = 16
ROTARY_DT = 20
rotary = RotaryEncoder(ROTARY_CLK, ROTARY_DT, max_steps=1000, wrap=False)
ROTARY_BUTTON = 21
button = Button(ROTARY_BUTTON)

# --- Initialize Variables ---
frequency = 1.0
step_rate = 1
last_step_time = time()
current_step = 0

switch_sequence_mode = False
sequence_sel = 1

# Track starting position of the rotary dial in each mode
dial_base_freq = frequency
dial_base_seq = sequence_sel
rotary_start_freq = rotary.steps
rotary_start_seq = rotary.steps

cached_display = None

def compute_display_digits():
    if not switch_sequence_mode:
        freq_disp = max(min_freq, int(frequency * 10))
        return [int(d) for d in f"{freq_disp:04}"]
    else:
        return [BLANK, BLANK, BLANK, decimal_start + sequence_sel]

def update_display_if_changed():
    global cached_display
    new_digits = compute_display_digits()
    if new_digits != cached_display:
        display.display(new_digits)
        cached_display = new_digits

def button_pressed():
    global press_start
    press_start = time()

def button_released():
    global step_rate, dial_base_freq, rotary_start_freq
    duration = time() - press_start if press_start is not None else 0
    if not switch_sequence_mode:
        if duration < 0.5:
            dial_base_freq = frequency
            rotary_start_freq = rotary.steps
            step_rate = 10 if step_rate == 1 else 1
            print(f"Frequency adjustment factor set to x{step_rate}")
    press_start = None

def toggle_mode():
    global switch_sequence_mode, dial_base_seq, rotary_start_seq
    switch_sequence_mode = not switch_sequence_mode
    if switch_sequence_mode:
        print("Entered sequence selector mode")
        dial_base_seq = sequence_sel
        rotary_start_seq = rotary.steps
        P1.off(); P2.off(); N1.off(); N2.off()
    else:
        print("Exited sequence selector mode")

button.when_pressed = button_pressed
button.when_released = button_released
button.when_held = toggle_mode

# --- Main Loop ---
while True:
    now = time()
    if not switch_sequence_mode:
        increment = 0.1 if step_rate == 1 else 1.0
        frequency = max(min(dial_base_freq + (rotary.steps - rotary_start_freq) * increment, max_freq), min_freq)
        period = 1.0 / frequency
        step_time = period / 2.0
        if now - last_step_time >= step_time:
            seq = sequences[sequence_sel - 1]
            state_index = seq[current_step]
            state = state_definitions[state_index]
            P1.value, P2.value, N1.value, N2.value = state
            last_step_time = now
            current_step = (current_step + 1) % 4
    else:
        new_sequence_sel = (dial_base_seq + (rotary.steps - rotary_start_seq)) % len(sequences)
        sequence_sel = new_sequence_sel if new_sequence_sel > 0 else len(sequences)
        print(f"Sequence set to: {sequence_sel} -> {sequences[sequence_sel - 1]}")
    update_display_if_changed()
    sleep(0.01)
