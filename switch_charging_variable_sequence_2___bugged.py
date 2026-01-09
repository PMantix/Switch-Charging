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
dial_base = frequency
dial_offset = rotary.steps
last_step_time = time()
current_step = 0

switch_sequence_mode = False
sequence_sel = 1
sequence_mode_base = 0
sequence_display_detailed = False
press_start = None
last_freq_toggle_time = 0
freq_toggle_hysteresis = 1
last_rotary_steps = rotary.steps  # Stores last dial position

                                                                                  
cached_display = None

def compute_display_digits():
    if not switch_sequence_mode:
        freq_disp = max(min_freq, int(frequency * 10))
        return [int(d) for d in f"{freq_disp:04}"]
    else:
        if not sequence_display_detailed:
            return [BLANK, BLANK, BLANK, decimal_start + sequence_sel]
        else:
            if sequence_sel == 1:
                return [0, 0, 0, 0]
            elif sequence_sel == len(sequences):
                return [1, 1, 1, 1]
            else:
                seq = sequences[sequence_sel - 1]
                return [s + 1 for s in seq]

def update_display_if_changed():
    global cached_display
    new_digits = compute_display_digits()
    if new_digits != cached_display:
        display.display(new_digits)
        cached_display = new_digits

# --- Button Callbacks ---
def button_pressed():
    global press_start
    press_start = time()

def button_released():
    global press_start, sequence_display_detailed, step_rate, last_freq_toggle_time, dial_base, dial_offset
    duration = time() - press_start if press_start is not None else 0
    if switch_sequence_mode:
        if duration < 0.5 and time() - last_freq_toggle_time > freq_toggle_hysteresis:
            sequence_display_detailed = not sequence_display_detailed
            print("Sequence display detail toggled:", sequence_display_detailed)
            last_freq_toggle_time = time()
    else:
        if duration < 0.5 and time() - last_freq_toggle_time >= freq_toggle_hysteresis:
            dial_base = frequency
            dial_offset = rotary.steps
            step_rate = 10 if step_rate == 1 else 1
            print(f"Frequency adjustment factor set to x{step_rate}")
            last_freq_toggle_time = time()
    press_start = None

def toggle_mode():
    global switch_sequence_mode, sequence_mode_base, dial_offset, sequence_sel
                                                                                         
                    

    switch_sequence_mode = not switch_sequence_mode

    if switch_sequence_mode:
        print("Entered sequence selector mode")
                                                                        
                                                                            

                                             
        sequence_mode_base = rotary.steps - (sequence_sel - 1)
                                            

        P1.off(); P2.off(); N1.off(); N2.off()
                                        

    else:
        print("Exited sequence selector mode")
                                                                   
                                                                       

                                                             
                                        
                                        

                                                                
                                             
        dial_offset = rotary.steps

                                                                        
                                                                             


button.hold_time = 0.5
button.when_pressed = button_pressed
button.when_released = button_released
button.when_held = toggle_mode

# --- Main Loop ---
while True:
    now = time()
                                                               

    if not switch_sequence_mode:
        # **Handle Frequency Update Independently**
        increment = 0.1 if step_rate == 1 else 1.0
        raw_frequency = dial_base + (rotary.steps - dial_offset) * increment
        frequency = max(min(raw_frequency, max_freq), min_freq)

                                                             
                                      
                                     
                                                                                               

        period = 1.0 / frequency
        step_time = period / 2.0

        # **Run Switching Logic at the Correct Rate**
        if now - last_step_time >= step_time:
            seq = sequences[sequence_sel - 1]
            state_index = seq[current_step]
            state = state_definitions[state_index]

            match state:
                case (True, False, True, False):
                    P1.on(); P2.off(); N1.on(); N2.off()
                case (True, False, False, True):
                    P1.on(); P2.off(); N1.off(); N2.on()
                case (False, True, True, False):
                    P1.off(); P2.on(); N1.on(); N2.off()
                case (False, True, False, True):
                    P1.off(); P2.on(); N1.off(); N2.on()
                case (True, True, True, True):
                    P1.on(); P2.on(); N1.on(); N2.on()
                case (False, False, False, False):
                    P1.off(); P2.off(); N1.off(); N2.off()
                case _:
                    P1.off(); P2.off(); N1.off(); N2.off()

            last_step_time = now
            current_step = (current_step + 1) % 4
    else:
        # **Detect Rotary Changes Immediately for Sequence Mode**
        if rotary.steps != last_rotary_steps:
            sequence_sel = (1 + (rotary.steps - sequence_mode_base)) % len(sequences)
            if sequence_sel == 0:
                sequence_sel = len(sequences)
            print(f"Sequence set to: {sequence_sel} -> {sequences[sequence_sel - 1]}")
                                                                                            
            last_rotary_steps = rotary.steps  # Update last position
        sleep(0.01)

    # **Update Display Independently from Frequency Updates**
    update_display_if_changed()