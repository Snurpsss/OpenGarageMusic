# OpenGarageMusic 0.9.21 — guide for an AI that programs MIDI pedals

**Version.** This guide was written by OpenGarageMusic **0.9.21** and describes the script format **1** and the pedals that version knows (`dd500`, `rv500`), plus the G3S. Use the guide that came with the user's version of the program (`ai-guide.md` in its archive, List → "Guide for an AI…" in the MIDI Pilot, `GET /guide` on its API): a newer program may know more parameters, an older one fewer. Scripts say the format with `"opengaragemusic": 1`.

OpenGarageMusic is a music studio for Windows (its source is published, free for personal use). Its **MIDI Pilot** window drives the user's MIDI pedals. You, the AI, do not talk to the pedals: you write a **script**, a JSON file the user applies in the MIDI Pilot (List → "Apply an AI script…"); the program shows what will be written, the user confirms, and the program writes the pedal and reads it back. The user may also give you an **export** in the same format (what a memory holds now) for you to change. When the user's program runs a local API (see the end), an agent on the same computer can call it with the same JSON.

## The script

```json
{ "opengaragemusic": 1,
  "actions": [
    { "pedal": "rv500", "memory": "50C", "name": "COMF NUMB", "from": "init",
      "set": { "Reverb : MODE": "HALL", "Reverb : time": "4.0", "Reverb : PRE-DELAY": 40, "Reverb : Effect Level": 30 } },
    { "pedal": "dd500", "pc_map": { "1": "81A", "2:5": "03A" } },
    { "pedal": "dd500", "system": { "MIDI Rx Channel": "Ch.1" } },
    { "g3s": "bank", "bank": 12, "name": "COMFORT.|NUMB", "song_mode": true },
    { "g3s": "preset", "bank": 12, "switch": 2, "name": "INTRO", "loops": [9, 10, 12],
      "order": [10, 7, 11, 12, 1, 2, 3, 4, 5, 6, 8, 9],
      "midi": [ { "kind": "PC down", "channel": 1, "program": 4 },
                { "kind": "CC down", "channel": 2, "controller": 80, "value": 127 } ] }
  ] }
```

Rules:

- `actions` run in order. A pedal action names the pedal by its key (below) and does one thing: a `memory` with `set` (and optionally `name`, `from`), a `pc_map`, or a `system`.
- `memory` is the memory written, "01A" to "99C" (99 banks of 3 slots A, B, C). `from` is `"memory"` (default: start from what the memory holds) or `"init"` (every parameter at its first value; use it when you set a whole sound).
- `set` maps **exact parameter names** (listed below for each pedal) to values **as the pedal shows them**: a label of the parameter's list (case does not matter), or a **whole number in the range given** ("0 to 120" means exactly that: not a percentage). When a parameter says "shown by the pedal as …", that text gives the unit and the scale of the number: `Delay Time` 1 to 10000 shown as 1ms - 10.0s is in milliseconds (440 for 440 ms); `BPM` 15 to 2400000 shown as 1.5 - 240000.0 is in tenths (1200 for 120.0); a range like -50 to 50 is written as is (the pedal shows it so). Do not invent parameter names. Parameters you leave out keep their value.
- `pc_map` maps a program change to a memory: the key is the program (0 to 127), or `"bank:program"` with the bank select (0 to 2). The pedal's own `MIDI Program Change IN` must be on for program changes to work.
- `name` is the patch's name, at most 16 ASCII characters for the BOSS pedals, 9 for a G3S preset; a G3S bank name is two lines of ten with `|` between.
- Always propose a plan in words first, then the script; keep scripts small (a few memories), and do not write memories the user did not name.

## BOSS DD-500 (key `dd500`)

297 memories, "01A" to "99C". The document the description comes from: https://static.roland.com/assets/media/pdf/DD-500_Ver-200_MIDI_Imple.pdf

### Patch parameters

By group; each with its shown range or its values, and its first value in brackets.

**Delay**

- `Patch Name`: text, 16 characters [INIT]
- `Delay MODE`: `STANDARD`, `TERA ECHO`, `SLOW ATTACK`, `FILTER`, `SHIMMER`, `SFX`, `REVERSE`, `PATTERN`, `DUAL`, `VINT. DIGITAL`, `TAPE`, `ANALOG` [STANDARD]
- `Delay Time`: 1 to 10000, shown by the pedal as 1ms - 10.0s [500]
- `BPM`: 15 to 2400000, shown by the pedal as Quarter Note=1.5 -=240000.0 [1200]
- `Note`: `16th`, `8th triplet`, `dotted 16th`, `8th`, `quarter triplet`, `dotted 8th`, `quarter`, `half triplet`, `dotted quarter`, `half`, `whole triplet`, `dotted half`, `whole` [quarter]
- `Feedback`: 0 to 100 [50]
- `Tone`: `-50` … `50` (101 values) [0]
- `Effect Level`: 0 to 120 [60]
- `Direct Level`: 0 to 100 [100]
- `Modulation Depth`: 0 to 100 [0]
- `Modulation Rate`: `0` … `1/16` (114 values) [50]
- `Carryover OFF/ON`: `OFF`, `ON` [ON]
- `Effect Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Direct Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `LED status`: `OFF`, `ON` [OFF]
- `Input level`: 0 to 100 [100]
- `Direct Feedback`: 0 to 100, shown by the pedal as 0% - 100% [100]
- `Feedback damp`: 0 to 10 [0]
- `Modulation mode`: `SINGLE`, `BI-PHASE` [SINGLE]
- `Input mode`: `A/B`, `MIX` [A/B]
- `Tempo hold`: `OFF`, `ON` [OFF]
- `Output gain`: -6 to 6, shown by the pedal as -6dB - +6dB [0]
- `Feedback loop sw`: `OFF`, `ON` [OFF]

**EQ**

- `EQ ON/OFF`: `OFF`, `PRE`, `FEEDBACK LOOP`, `POST` [OFF]
- `EQ Total Level`: -20 to 20, shown by the pedal as -20dB - +20dB [0]
- `EQ Low Cut Filter Freq`: `FLAT`, `20.0Hz`, `25.0Hz`, `31.5Hz`, `40.0Hz`, `50.0Hz`, `63.0Hz`, `80.0Hz`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz` [FLAT]
- `EQ Low Gain`: -20 to 20, shown by the pedal as -20dB - +20dB [0]
- `EQ Low Mid Gain`: -20 to 20, shown by the pedal as -20dB - +20dB [0]
- `EQ Low Mid Freq`: `20.0Hz` … `10.0kHz` (28 values) [500Hz]
- `EQ Low Mid Q`: `0.5`, `1`, `2`, `4`, `8`, `16` [1]
- `EQ High Mid Gain`: -20 to 20, shown by the pedal as -20dB - +20dB [0]
- `EQ High Mid Freq`: `20.0Hz` … `10.0kHz` (28 values) [4.00kHz]
- `EQ High Mid Q`: `0.5`, `1`, `2`, `4`, `8`, `16` [1]
- `High EQ Gain`: -20 to 20, shown by the pedal as -20dB - +20dB [0]
- `High Cut Filter Freq`: `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.50kHz`, `3.15kHz`, `4.00kHz`, `5.00kHz`, `6.30kHz`, `8.00kHz`, `10.0kHz`, `12.5kHz`, `16.0kHz`, `FLAT` [FLAT]
- `Low DAMP`: -20 to 0, shown by the pedal as -20dB - +0dB [0]
- `Low DAMP FREQ`: `20.0Hz`, `25.0Hz`, `31.5Hz`, `40.0Hz`, `50.0Hz`, `63.0Hz`, `80.0Hz`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz` [20.0Hz]
- `Hi Damp GAIN`: -20 to 0, shown by the pedal as -20dB - +0dB [0]
- `Hi Damp FREQ`: `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.50kHz`, `3.15kHz`, `4.00kHz`, `5.00kHz`, `6.30kHz`, `8.00kHz`, `10.0kHz`, `12.5kHz`, `16.0kHz` [16.0kHz]

**Duck**

- `DUCK Sens`: 0 to 100 [50]
- `DUCK Pre Depth`: 0 to 100 [0]
- `DUCK Post Depth`: 0 to 100 [0]

**Switches and pedal**

- `TAP/CTL Function`: `OFF`, `TAP`, `HOLD`, `WARP`, `TWIST`, `MOMENT`, `ROLL 1/2`, `ROLL 1/4`, `ROLL 1/8`, `FADE` [TAP]
- `CTL1 Function`: `OFF`, `TAP`, `HOLD`, `WARP`, `TWIST`, `MOMENTARY`, `ROLL 1/2`, `ROLL 1/4`, `ROLL 1/8`, `FADE`, `EFFECT SW`, `BANK UP`, `BANK DOWN`, `LOOPonOff`, `LOOP CLR` [OFF]
- `CTL1 Mode`: `MOMENT`, `TOGGLE` [MOMENT]
- `CTL2 Function`: `OFF`, `TAP`, `HOLD`, `WARP`, `TWIST`, `MOMENTARY`, `ROLL 1/2`, `ROLL 1/4`, `ROLL 1/8`, `FADE`, `EFFECT SW`, `BANK UP`, `BANK DOWN`, `LOOPonOff`, `LOOP CLR` [OFF]
- `CTL2 Mode`: `MOMENT`, `TOGGLE` [MOMENT]
- `Exp Pedal Function`: `OFF`, `TIME`, `FEEDBACK`, `TONE`, `E.LEVEL`, `MOD DEPTH`, `MOD RATE` [OFF]
- `Exp target min`: 0 to 10000 [0]
- `Exp target max`: 0 to 10000 [0]
- `TAP/CTL Mode`: `MOMENT`, `TOGGLE` [MOMENT]
- `TAP/CTL Rise time`: 0 to 100 [50]
- `TAP/CTL Fall time`: 0 to 100 [50]
- `CTL1 Rise time`: 0 to 100 [50]
- `CTL1 Fall time`: 0 to 100 [50]
- `CTL2 Rise time`: 0 to 100 [50]
- `CTL2 Fall time`: 0 to 100 [50]
- `TAP/CTL Hold level`: 0 to 120 [100]
- `TAP/CTL Warp level`: 0 to 120 [100]
- `TAP/CTL Twist level`: 0 to 120 [100]
- `CTL1 Hold level`: 0 to 120 [100]
- `CTL1 Warp level`: 0 to 120 [100]
- `CTL1 Twist level`: 0 to 120 [100]
- `CTL2 Hold level`: 0 to 120 [100]
- `CTL2 Warp level`: 0 to 120 [100]
- `CTL2 Twist level`: 0 to 120 [100]

**Assign 1**

- `Assign1 sw`: `OFF`, `ON` [OFF]
- `Assign1 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `Assign1 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `Assign1 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `Assign1 target min`: 0 to 2400000 [0]
- `Assign1 target max`: 0 to 2400000 [3]
- `Assign1 Active Range LO`: 0 to 126 [0]
- `Assign1 Active Range HI`: 1 to 127 [127]
- `Assign1 Wave Rate`: `0` … `1/16` (114 values) [50]
- `Assign1 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `Assign1 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `Assign1 Internal Pedal Time`: 0 to 100 [30]
- `Assign1 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 2**

- `Assign2 sw`: `OFF`, `ON` [OFF]
- `Assign2 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `Assign2 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `Assign2 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `Assign2 target min`: 0 to 2400000 [0]
- `Assign2 target max`: 0 to 2400000 [3]
- `ASSIGN2 Active Range LO`: 0 to 126 [0]
- `ASSIGN2 Active Range HI`: 1 to 127 [127]
- `ASSIGN2 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN2 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN2 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN2 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN2 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 3**

- `Assign3 sw`: `OFF`, `ON` [OFF]
- `Assign3 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `Assign3 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `Assign3 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `Assign3 target min`: 0 to 2400000 [0]
- `Assign3 target max`: 0 to 2400000 [3]
- `ASSIGN3 Active Range LO`: 0 to 126 [0]
- `ASSIGN3 Active Range HI`: 1 to 127 [127]
- `ASSIGN3 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN3 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN3 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN3 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN3 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 4**

- `Assign4 sw`: `OFF`, `ON` [OFF]
- `Assign4 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `Assign4 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `Assign4 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `Assign4 target min`: 0 to 2400000 [0]
- `Assign4 target max`: 0 to 2400000 [3]
- `ASSIGN4 Active Range LO`: 0 to 126 [0]
- `ASSIGN4 Active Range HI`: 1 to 127 [127]
- `ASSIGN4 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN4 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN4 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN4 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN4 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 5**

- `ASSIGN5 sw`: `OFF`, `ON` [OFF]
- `ASSIGN5 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `ASSIGN5 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `ASSIGN5 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `ASSIGN5 target min`: 0 to 2400000 [0]
- `ASSIGN5 target max`: 0 to 2400000 [3]
- `ASSIGN5 Active Range LO`: 0 to 126 [0]
- `ASSIGN5 Active Range HI`: 1 to 127 [127]
- `ASSIGN5 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN5 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN5 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN5 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN5 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 6**

- `ASSIGN6 sw`: `OFF`, `ON` [OFF]
- `ASSIGN6 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `ASSIGN6 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `ASSIGN6 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `ASSIGN6 target min`: 0 to 2400000 [0]
- `ASSIGN6 target max`: 0 to 2400000 [3]
- `ASSIGN6 Active Range LO`: 0 to 126 [0]
- `ASSIGN6 Active Range HI`: 1 to 127 [127]
- `ASSIGN6 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN6 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN6 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN6 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN6 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 7**

- `ASSIGN7 sw`: `OFF`, `ON` [OFF]
- `ASSIGN7 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `ASSIGN7 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `ASSIGN7 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `ASSIGN7 target min`: 0 to 2400000 [0]
- `ASSIGN7 target max`: 0 to 2400000 [3]
- `ASSIGN7 Active Range LO`: 0 to 126 [0]
- `ASSIGN7 Active Range HI`: 1 to 127 [127]
- `ASSIGN7 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN7 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN7 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN7 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN7 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Assign 8**

- `ASSIGN8 sw`: `OFF`, `ON` [OFF]
- `ASSIGN8 source`: `TAP/CTL` … `CC#95` (70 values) [TAP/CTL]
- `ASSIGN8 source mode`: `MOMENT`, `TOGGLE` [TOGGLE]
- `ASSIGN8 target`: `EFFECT SW` … `TAP/CTL LED` (206 values) [EQ SW]
- `ASSIGN8 target min`: 0 to 2400000 [0]
- `ASSIGN8 target max`: 0 to 2400000 [3]
- `ASSIGN8 Active Range LO`: 0 to 126 [0]
- `ASSIGN8 Active Range HI`: 1 to 127 [127]
- `ASSIGN8 Wave Rate`: `0` … `1/16` (114 values) [50]
- `ASSIGN8 Wave Form`: `SAW`, `TRI`, `SIN` [SAW]
- `ASSIGN8 Internal Pedal Trigger`: `PAT CNG` … `CC#95` (69 values) [PAT CNG]
- `ASSIGN8 Internal Pedal Time`: 0 to 100 [30]
- `ASSIGN8 Internal Pedal Curve`: `LINEAR`, `SLOW`, `FAST` [LINEAR]

**Other**

- `Assign Input Sens`: 0 to 100 [50]

**Tera Echo**

- `Tera Echo Resonance`: 0 to 100 [50]
- `Tera Echo EQ sw`: `OFF`, `PRE`, `POST` [OFF]

**Slow Attack**

- `Slow Attack Sens`: 0 to 100 [50]
- `Slow Attack Rise Time`: 0 to 100 [50]
- `Slow attack target`: `EFFECT`, `ALL` [EFFECT]
- `Slow attack curve`: -50 to 50, shown by the pedal as -50 - +50 [0]

**Filter**

- `Filter LFO Type`: `TRI`, `SIN`, `SQR` [TRI]
- `Filter LFO Rate`: `0` … `1/16` (114 values) [50]
- `Filter LFO Depth`: 0 to 100 [50]
- `Filter Type`: `LPF`, `BPF`, `HPF` [LPF]
- `Filter Cutoff Frequency`: 0 to 19 [10]
- `Filter Resonance`: 0 to 100 [50]
- `Filter Position`: `PRE`, `POST` [POST]
- `Filter Mode`: `AUTO`, `TOUCH` [AUTO]
- `Filter touch polarity`: `DOWN`, `UP` [DOWN]
- `Filter touch sens`: `-100` … `100` (201 values) [50]
- `Filter touch peak`: 0 to 100 [50]
- `Filter touch fequency`: 0 to 100 [50]
- `Filter touch decay`: 0 to 100 [50]

**Shimmer**

- `Pitch Pitch`: -24 to 24, shown by the pedal as -24 - +24 [12]
- `Pitch Pitch Fine`: -50 to 50, shown by the pedal as -50 - +50 [0]
- `Pitch/Direct Balance`: 0 to 100 [100]
- `Shimmer rise time`: 0 to 10 [2]
- `Shimmer type`: `PITCH`, `OVERTONE` [PITCH]
- `Shimmer overtone upper`: `-50` … `50` (101 values) [0]
- `Shimmer overtone lower`: `-50` … `50` (101 values) [0]
- `Shimmer overtone direct`: `-50` … `50` (101 values) [0]
- `Shimmer pitchshifter mode`: `MONO`, `STEREO` [MONO]
- `Shimmer pitchshifter feedback`: 0 to 100 [50]
- `Shimmer pitchshifter2 feedback`: 0 to 100 [50]
- `Shimmer Pitch2 Pitch`: -24 to 24, shown by the pedal as -24 - +24 [12]
- `Shimmer Pitch2 Pitch Fine`: -50 to 50, shown by the pedal as -50 - +50 [0]
- `Shimmer Pitchshifter character`: `OFF`, `ON` [ON]

**SFX**

- `SFX LoFi Bit Depth`: `OFF`, `15`, `14`, `13`, `12`, `11`, `10`, `9`, `8`, `7`, `6`, `5`, `4`, `3`, `2`, `1` [14]
- `SFX LoFi Sample Rate`: `OFF` … `1/32` (32 values) [1/10]
- `SFX LoFi Balance`: 0 to 100 [100]
- `SFX Tremolo Wave`: 0 to 100 [80]
- `SFX Tremolo Rate`: `0` … `1/16` (114 values) [95]
- `SFX Tremolo Depth`: 0 to 100 [100]
- `SFX Tremolo Position`: `PRE`, `POST` [POST]
- `SFX type`: `TREMOLO`, `PHASER` [TREMOLO]
- `SFX phaser rate`: 0 to 113 [50]
- `SFX phaser depth`: 0 to 100 [50]
- `SFX phaser resonance`: 0 to 100 [50]
- `SFX phaser manual`: 0 to 100 [50]

**Pattern**

- `Pattern`: `PAT1`, `PAT2`, `PAT3`, `PAT4`, `PAT5`, `PAT6`, `PAT7`, `PAT8`, `PAT9`, `PAT10`, `USER` [PAT1]
- `Delay1 Delay Time`: `0%` … `24/24` (189 values) [1/16]
- `Delay1 Level`: 0 to 100 [100]
- `Delay1 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay2 Delay Time`: `0%` … `24/24` (189 values) [2/16]
- `Delay2 Level`: 0 to 100 [100]
- `Delay2 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay3 Delay Time`: `0%` … `24/24` (189 values) [3/16]
- `Delay3 Level`: 0 to 100 [100]
- `Delay3 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay4 Delay Time`: `0%` … `24/24` (189 values) [4/16]
- `Delay4 Level`: 0 to 100 [100]
- `Delay4 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay5 Delay Time`: `0%` … `24/24` (189 values) [5/16]
- `Delay5 Level`: 0 to 100 [0]
- `Delay5 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay6 Delay Time`: `0%` … `24/24` (189 values) [6/16]
- `Delay6 Level`: 0 to 100 [0]
- `Delay6 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay7 Delay Time`: `0%` … `24/24` (189 values) [7/16]
- `Delay7 Level`: 0 to 100 [0]
- `Delay7 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay8 Delay Time`: `0%` … `24/24` (189 values) [8/16]
- `Delay8 Level`: 0 to 100 [0]
- `Delay8 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay9 Delay Time`: `0%` … `24/24` (189 values) [9/16]
- `Delay9 Level`: 0 to 100 [0]
- `Delay9 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay10 Delay Time`: `0%` … `24/24` (189 values) [10/16]
- `Delay10 Level`: 0 to 100 [0]
- `Delay10 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay11 Delay Time`: `0%` … `24/24` (189 values) [11/16]
- `Delay11 Level`: 0 to 100 [0]
- `Delay11 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay12 Delay Time`: `0%` … `24/24` (189 values) [12/16]
- `Delay12 Level`: 0 to 100 [0]
- `Delay12 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay13 Delay Time`: `0%` … `24/24` (189 values) [13/16]
- `Delay13 Level`: 0 to 100 [0]
- `Delay13 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay14 Delay Time`: `0%` … `24/24` (189 values) [14/16]
- `Delay14 Level`: 0 to 100 [0]
- `Delay14 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay15 Delay Time`: `0%` … `24/24` (189 values) [15/16]
- `Delay15 Level`: 0 to 100 [0]
- `Delay15 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `Delay16 Delay Time`: `0%` … `24/24` (189 values) [16/16]
- `Delay16 Level`: 0 to 100 [0]
- `Delay16 Pan`: 0 to 100, shown by the pedal as L50 - CENTER - R50 [50]
- `PATTERN NOTE`: `16th`, `8th triplet`, `dotted 16th`, `8th`, `quarter triplet`, `dotted 8th`, `quarter`, `half triplet`, `dotted quarter`, `half`, `whole triplet`, `dotted half`, `whole` [whole]
- `Pattern type`: `DELAY`, `GATE` [DELAY]
- `Pattern Step number`: `8`, `12`, `16`, `24` [16]
- `Pattern STEP1 length`: 0 to 100 [50]
- `Pattern STEP1 Level`: 0 to 100 [100]
- `Pattern STEP2 length`: 0 to 100 [50]
- `Pattern STEP2 Level`: 0 to 100 [100]
- `Pattern STEP3 length`: 0 to 100 [50]
- `Pattern STEP3 Level`: 0 to 100 [100]
- `Pattern STEP4 length`: 0 to 100 [50]
- `Pattern STEP4 Level`: 0 to 100 [100]
- `Pattern STEP5 length`: 0 to 100 [50]
- `Pattern STEP5 Level`: 0 to 100 [100]
- `Pattern STEP6 length`: 0 to 100 [50]
- `Pattern STEP6 Level`: 0 to 100 [100]
- `Pattern STEP7 length`: 0 to 100 [50]
- `Pattern STEP7 Level`: 0 to 100 [100]
- `Pattern STEP8 length`: 0 to 100 [50]
- `Pattern STEP8 Level`: 0 to 100 [100]
- `Pattern STEP9 length`: 0 to 100 [50]
- `Pattern STEP9 Level`: 0 to 100 [100]
- `Pattern STEP10 length`: 0 to 100 [50]
- `Pattern STEP10 Level`: 0 to 100 [100]
- `Pattern STEP11 length`: 0 to 100 [50]
- `Pattern STEP11 Level`: 0 to 100 [100]
- `Pattern STEP12 length`: 0 to 100 [50]
- `Pattern STEP12 Level`: 0 to 100 [100]
- `Pattern STEP13 length`: 0 to 100 [50]
- `Pattern STEP13 Level`: 0 to 100 [100]
- `Pattern STEP14 length`: 0 to 100 [50]
- `Pattern STEP14 Level`: 0 to 100 [100]
- `Pattern STEP15 length`: 0 to 100 [50]
- `Pattern STEP15 Level`: 0 to 100 [100]
- `Pattern STEP16 length`: 0 to 100 [50]
- `Pattern STEP16 Level`: 0 to 100 [100]
- `Pattern STEP17 length`: 0 to 100 [50]
- `Pattern STEP17 Level`: 0 to 100 [100]
- `Pattern STEP18 length`: 0 to 100 [50]
- `Pattern STEP18 Level`: 0 to 100 [100]
- `Pattern STEP19 length`: 0 to 100 [50]
- `Pattern STEP19 Level`: 0 to 100 [100]
- `Pattern STEP20 length`: 0 to 100 [50]
- `Pattern STEP20 Level`: 0 to 100 [100]
- `Pattern STEP21 length`: 0 to 100 [50]
- `Pattern STEP21 Level`: 0 to 100 [100]
- `Pattern STEP22 length`: 0 to 100 [50]
- `Pattern STEP22 Level`: 0 to 100 [100]
- `Pattern STEP23 length`: 0 to 100 [50]
- `Pattern STEP23 Level`: 0 to 100 [100]
- `Pattern STEP24 length`: 0 to 100 [50]
- `Pattern STEP24 Level`: 0 to 100 [100]

**Dual**

- `Dual Mode`: `SERIES`, `PARALLEL`, `A/B`, `A/B SERIS`, `A/B PARA` [A/B]
- `Dual D1 Type`: `STANDARD`, `ANALOG`, `TAPE` [STANDARD]
- `Dual D1 Effect Level`: 0 to 100 [100]
- `Dual Link`: `OFF`, `ON` [ON]
- `Dual D2 Time`: 1 to 5000 [400]
- `Dual D2 Time (LINK=ON)`: 1 to 100, shown by the pedal as 1% - 100% [75]
- `Dual D2 Feedback`: 0 to 100 [50]
- `Dual D2 Feedback (LINK=ON)`: 0 to 100, shown by the pedal as 0% - 100% [100]
- `Dual D2 Effect Level`: 0 to 100 [100]
- `Dual D2 Type`: `STANDARD`, `ANALOG`, `TAPE` [STANDARD]

**Vintage Digital**

- `Vintage Type`: `SDE-2000`, `SDE-3000`, `DD-2` [SDE-3000]
- `Vintage Filter Switch`: `OFF`, `ON` [OFF]
- `Vintage TIME x 2 Switch`: `OFF`, `ON` [OFF]
- `Vintage Delay Phase`: `NOR`, `INV` [NOR]
- `Vintage Feedback Phase`: `NOR`, `INV` [NOR]

**Tape**

- `Tape Type`: `RE-201`, `EchoP`, `EchoR` [RE-201]
- `Tape Number of Head`: `1`, `2`, `3`, `1+2`, `1+3`, `2+3`, `1+2+3` [2+3]
- `Tape wow and flutter`: 0 to 100 [50]
- `Tape Modulation rate`: 0 to 100 [50]
- `TAPE EchoR head`: `1`, `2`, `3`, `4`, `1+2`, `2+3`, `3+4`, `1+3`, `2+4`, `1+2+3`, `2+3+4`, `1+2+3+4` [1]
- `TAPE EchoR selector`: `ECHO`, `REPEAT`, `SWELL` [REPEAT]

**Analog**

- `Analog Stage`: `4096`, `4096x2`, `4096x4` [4096]

### System settings (`system`)

- `Global Output Mode`: `STEREO`, `A:DIR B:EFX`, `DIRECT MUTE` [STEREO]
- `Bank wait mode`: `WAIT`, `IMMEDIATE` [WAIT]
- `Bank Extent Begin`: 0 to 98, shown by the pedal as 01 - 99 [0]
- `Bank Extent End`: 0 to 98, shown by the pedal as 01 - 99 [98]
- `Global Knob Lock Switch`: `OFF`, `ON` [OFF]
- `Global Knob Mode`: `IMMEDIATE`, `HOOK` [IMMEDIATE]
- `Phrase Loop Mode`: `96k MONO`, `48k ST`, `48k MONO` [96k MONO]
- `Global Bypass Mode`: `BUFFERED`, `TRUE` [BUFFERED]
- `Global Pedal Action`: `PUSH`, `RELEASE` [PUSH]
- `Foot Switch Hold Time`: `1sec`, `2sec`, `3sec`, `4sec`, `5sec` [2sec]
- `Foot Switch Mode`: `NORMAL`, `A/B/C`, `A/B SIMUL`, `SW DN/UP` [NORMAL]
- `Global Display Type`: `DISP_TIME`, `DISP_BPM`, `DISP_PATCH`, `DISP_PARAM` [DISP_TIME]
- `MIDI Rx Channel`: `Ch.1`, `Ch.2`, `Ch.3`, `Ch.4`, `Ch.5`, `Ch.6`, `Ch.7`, `Ch.8`, `Ch.9`, `Ch.10`, `Ch.11`, `Ch.12`, `Ch.13`, `Ch.14`, `Ch.15`, `Ch.16`, `OFF` [Ch.1]
- `MIDI Tx Channel`: `Ch.1`, `Ch.2`, `Ch.3`, `Ch.4`, `Ch.5`, `Ch.6`, `Ch.7`, `Ch.8`, `Ch.9`, `Ch.10`, `Ch.11`, `Ch.12`, `Ch.13`, `Ch.14`, `Ch.15`, `Ch.16`, `Rx`, `OFF` [Rx]
- `MIDI Program Change IN`: `OFF`, `ON` [ON]
- `MIDI Program Change OUT`: `OFF`, `ON` [ON]
- `MIDI Bank Select OUT`: `MSB`, `M+L` [MSB]
- `MIDI Control Change IN`: `OFF`, `ON` [ON]
- `MIDI Control Change OUT`: `OFF`, `ON` [ON]
- `MIDI Effect A Control Change`: `OFF` … `CC#95 ......(*A)` (64 values) [CC#28]
- `MIDI Effect B Control Change`: `OFF` … `CC#95` (64 values) [CC#29]
- `MIDI TAP/CTL Switch Control Change`: `OFF` … `CC#95` (64 values) [CC#82]
- `MIDI Feedback Control Change`: `OFF` … `CC#95` (64 values) [CC#18]
- `MIDI Effect Level Control Change`: `OFF` … `CC#95` (64 values) [CC#19]
- `MIDI Tone Control Change`: `OFF` … `CC#95` (64 values) [CC#20]
- `MIDI Mod Depth Control Change`: `OFF` … `CC#95` (64 values) [CC#21]
- `MIDI CTL1 Control Change`: `OFF` … `CC#95` (64 values) [CC#80]
- `MIDI CTL2 Control Change`: `OFF` … `CC#95` (64 values) [CC#81]
- `MIDI EXP Control Change`: `OFF` … `CC#95` (64 values) [CC#16]
- `MIDI EFFECT ON/OFF Control Change`: `OFF` … `CC#95` (64 values) [CC#27]
- `MIDI LOOPER ON/OFF Control Change`: `OFF` … `CC#95` (64 values) [CC#22]
- `MIDI LOOPER [REC/DUB] Control Change`: `OFF` … `CC#95` (64 values) [CC#23]
- `MIDI LOOPER [PLAY] Control Change`: `OFF` … `CC#95` (64 values) [CC#24]
- `MIDI LOOPER [STOP] Control Change`: `OFF` … `CC#95` (64 values) [CC#25]
- `MIDI LOOPER [CLEAR] Control Change`: `OFF` … `CC#95` (64 values) [CC#26]
- `MIDI Sync`: `INTERNAL`, `EXT (USB)`, `EXT (MIDI)`, `AUTO` [INTERNAL]
- `MIDI source of realtime message to output`: `INT`, `USB`, `MIDI` [INT]
- `MIDI In THRU into Out`: `OFF`, `USB`, `MIDI`, `U+M` [MIDI]
- `USB In THRU into Out`: `OFF`, `USB`, `MIDI`, `U+M` [OFF]
- `effect sw`: `OFF`, `ON` [ON]
- `effect sw2`: `OFF`, `ON` [OFF]
- `LOOP Level`: 0 to 100 [100]
- `MIDI Time Control Change`: `OFF` … `CC#95` (64 values) [CC#17]
- `Feedback loop sw`: `OFF`, `ON` [OFF]

## BOSS RV-500 (key `rv500`)

297 memories, "01A" to "99C". The document the description comes from: https://static.roland.com/assets/media/pdf/RV-500_MIDI_Imple_jpn01_W.pdf

### Patch parameters

By group; each with its shown range or its values, and its first value in brackets.

**Reverb**

- `Patch Name`: text, 16 characters [INIT]
- `Reverb : MODE`: `ROOM`, `HALL`, `PLATE`, `SPRING`, `SHIMMER`, `FAST DECAY`, `EARLY REF`, `NON-LINEAR`, `SFX`, `DUAL`, `SRV`, `SPACE ECHO` [ROOM]
- `Reverb : time`: `0.1` … `10.0` (100 values) [1.5]
- `Reverb : PRE-DELAY`: 0 to 200 [50]
- `Reverb : Effect Level`: 0 to 100 [50]
- `Reverb : Tone Low`: -50 to 50 [0]
- `Reverb : Tone High`: -50 to 50 [0]
- `Reverb : Low Cut`: `FLAT`, `20.0Hz`, `25.0Hz`, `31.5Hz`, `40.0Hz`, `50.0Hz`, `63.0Hz`, `80.0Hz`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz` [FLAT]
- `Reverb : High Cut`: `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.50kHz`, `3.15kHz`, `4.00kHz`, `5.00kHz`, `6.30kHz`, `8.00kHz`, `10.0kHz`, `12.5kHz`, `16.0kHz`, `FLAT` [FLAT]
- `Reverb : Low DAMP`: -50 to 50 [0]
- `Reverb : High Damp`: -50 to 50 [0]
- `Reverb : Density`: 1 to 10 [5]
- `Reverb : Mod Depth`: 0 to 100 [0]
- `Reverb : Mod Rate`: 0 to 100 [50]
- `Reverb : Delay Mode`: `OFF`, `SERIES`, `PARALLEL` [SERIES]
- `Carryover OFF/ON`: `OFF`, `ON` [ON]
- `Direct Level`: 0 to 100 [100]
- `DUCK Sens`: 0 to 100 [50]
- `DUCK Pre Depth`: 0 to 100 [0]
- `DUCK Post Depth`: 0 to 100 [0]
- `Output gain (db)`: `-6`, `-5`, `-4`, `-3`, `-2`, `-1`, `0`, `+1`, `+2`, `+3`, `+4`, `+5`, `+6` [0]
- `Input Level`: 0 to 100 [100]
- `FADE IN/OUT`: `OFF`, `ON` [OFF]
- `Assign Input Sens`: 0 to 100 [50]
- `LED status`: `OFF`, `ON` [OFF]

**Delay**

- `Delay : Time`: 1 to 2000 [500]
- `Delay : BPM`: 1 to 2400000 [1200]
- `Delay : Note`: `1/16`, `1/8T`, `1/16D`, `1/8`, `1/4T`, `1/8D`, `1/4`, `1/2T`, `1/4D`, `1/2`, `1/1T`, `1/2D`, `1/1` [1/4]
- `Delay : Feedback`: 0 to 100 [50]
- `Delay : Level`: 0 to 120 [0]
- `Delay : Tone Low`: -50 to 50 [0]
- `Delay : Tone High`: -50 to 50 [0]
- `Delay : Mod Depth`: 0 to 100 [0]
- `Delay : Mod Rate`: 0 to 100 [50]
- `Delay : Input Level`: 0 to 100 [100]
- `Delay : Tempo Hold`: `OFF`, `ON` [OFF]

**ROOM**

- `ROOM : TYPE`: `AMBIENCE`, `SMALL`, `MEDIUM`, `LARGE` [MEDIUM]

**HALL**

- `HALL : TYPE`: `SMALL`, `MEDIUM`, `LARGE` [MEDIUM]

**SPRING**

- `SPRING : Spring number`: 1 to 3 [2]

**SHIMMER**

- `SHIMMER : Shift1 Pitch`: -24 to 24 [12]
- `SHIMMER : Shift1 Pitch fine`: -50 to 50 [0]
- `SHIMMER : Shift1 level`: 0 to 100 [50]
- `SHIMMER : Shift1 Release`: 0 to 100 [50]
- `SHIMMER : Shift2 Pitch`: -24 to 24 [12]
- `SHIMMER : Shift2 Pitch fine`: -50 to 50 [0]
- `SHIMMER : Shift2 level`: 0 to 100 [50]
- `SHIMMER : Shift2 Release`: 0 to 100 [50]

**FAST DECAY**

- `FAST DECAY : Reverb Decay`: 1 to 10 [5]

**EARLY REFLECTION**

- `EARLY REFLECTION : Time`: 1 to 10 [3]
- `EARLY REFLECTION : TYPE`: 1 to 4 [1]
- `EARLY REFLECTION : Envelope`: 1 to 10 [1]

**NON-LINEAR**

- `NON-LINEAR : Type`: `GATE`, `REVERSE`, `NON-LINEAR` [GATE]

**NON-LINEAR REVERSE**

- `NON-LINEAR REVERSE : Time`: 1 to 10 [10]
- `NON-LINEAR REVERSE : gate Time`: 1 to 10 [10]

**NON-LINEAR GATE**

- `NON-LINEAR GATE : Threhold`: 0 to 100 [50]
- `NON-LINEAR GATE : Hold Time`: 1 to 10 [2]

**SFX**

- `SFX : TYPE`: `LO-FI`, `SLOWVERB`, `STORM` [LO-FI]

**SFX Lo-fi**

- `SFX Lo-fi : LO-FI`: 1 to 10 [5]
- `SFX Lo-fi : Distortion`: 0 to 10 [4]
- `SFX Lo-FI : LO-FI Level`: 0 to 100 [100]
- `SFX Lo-FI : Direct Level`: 0 to 100 [0]

**SFX Slowverb**

- `SFX Slowverb : Rise time`: 0 to 100 [20]
- `SFX Slowverb : Sens`: 0 to 100 [50]
- `SFX Slowverb : Lower harm`: 0 to 100 [50]
- `SFX Slowverb : Upper harm`: 0 to 100 [50]
- `SFX Slowverb : Unison mix`: 0 to 100 [50]
- `SFX Slowverb : Detune`: 0 to 100 [50]

**SFX Storm**

- `SFX Storm : Color`: 0 to 100 [50]
- `SFX Storm : Depth`: 0 to 100 [50]
- `SFX Storm : Speed`: 0 to 100 [50]

**DUAL**

- `DUAL : 1 Type`: `ROOM`, `HALL`, `PLATE`, `SPRING` [ROOM]
- `DUAL : 1 Delay Time`: 1 to 100 [15]
- `DUAL : 1 PRE-DELAY`: 0 to 200 [50]
- `DUAL : 1 Density`: 1 to 10 [5]
- `DUAL : 1 Tone Low`: -50 to 50 [0]
- `DUAL : 1 Tone High`: -50 to 50 [0]
- `DUAL : 1 Low Cut`: `FLAT`, `20.0Hz`, `25.0Hz`, `31.5Hz`, `40.0Hz`, `50.0Hz`, `63.0Hz`, `80.0Hz`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz` [FLAT]
- `DUAL : 1 High Cut`: `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.50kHz`, `3.15kHz`, `4.00kHz`, `5.00kHz`, `6.30kHz`, `8.00kHz`, `10.0kHz`, `12.5kHz`, `16.0kHz`, `FLAT` [FLAT]
- `DUAL : 1 Effect Level`: 0 to 100 [50]
- `DUAL : 2 Type`: `ROOM`, `HALL`, `PLATE`, `SPRING` [ROOM]
- `DUAL : 2 Delay Time`: 1 to 100 [15]
- `DUAL : 2 PRE-DELAY`: 0 to 200 [50]
- `DUAL : 2 Density`: 1 to 10 [5]
- `DUAL : 2 Tone Low`: -50 to 50 [0]
- `DUAL : 2 Tone High`: -50 to 50 [0]
- `DUAL : 2 Low Cut`: `FLAT`, `20.0Hz`, `25.0Hz`, `31.5Hz`, `40.0Hz`, `50.0Hz`, `63.0Hz`, `80.0Hz`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz` [FLAT]
- `DUAL : 2 High Cut`: `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.50kHz`, `3.15kHz`, `4.00kHz`, `5.00kHz`, `6.30kHz`, `8.00kHz`, `10.0kHz`, `12.5kHz`, `16.0kHz`, `FLAT` [FLAT]
- `DUAL : 2 Effect Level`: 0 to 100 [50]
- `DUAL : Crossover`: `PARALLEL`, `100Hz`, `125Hz`, `160Hz`, `200Hz`, `250Hz`, `315Hz`, `400Hz`, `500Hz`, `630Hz`, `800Hz`, `1.00kHz`, `1.25kHz`, `1.60kHz`, `2.00kHz`, `2.5kHz`, `3.15kHz`, `4.00kHz` [PARALLEL]

**SRV**

- `SRV : Selection`: `R0.3`, `R1.0`, `R7.0`, `R15`, `R22`, `R26`, `R32`, `R37`, `H15`, `H22`, `H26`, `H32`, `H37`, `P-B`, `P-A` [P-A]
- `SRV : HF Damp`: 5 to 100 [50]
- `SRV : Density`: 0 to 9 [0]
- `SRV : Attack Gain`: 0 to 9 [0]
- `SRV : Attack time`: 0 to 9 [0]
- `SRV : Early reflections density`: 0 to 9 [0]
- `SRV : Early reflections level`: 0 to 99 [99]
- `SRV : Low Gain`: -24 to 12 [0]
- `SRV : Low Freq`: 4 to 100 [50]
- `SRV : Mid Gain`: -24 to 12 [0]
- `SRV : Mid Freq`: 25 to 999 [500]
- `SRV : Mid Q`: 2 to 90 [50]
- `SRV : High Gain`: -24 to 12 [0]
- `SRV : High Freq`: 80 to 999 [700]
- `SRV : High Q`: 2 to 90 [50]
- `SRV : OUTPUT MODE`: `MONO`, `STEREO` [STEREO]

**SPACE ECHO**

- `SPACE ECHO : Echo Repeat rate`: 1 to 10000 [500]
- `SPACE ECHO : Echo BPM`: 1 to 2400000 [1200]
- `SPACE ECHO : Echo Head Select`: `1`, `2`, `3`, `1+2`, `1+3`, `2+3`, `1+2+3` [1+2]
- `SPACE ECHO : Echo Volume`: 0 to 120 [50]
- `SPACE ECHO : Reverb : Effect Level`: 0 to 100 [0]

**Switches and pedal**

- `TAP/CTL Function`: 0 to 8 [1]
- `TAP/CTL Mode`: `OFF`, `ON` [OFF]
- `TAP/CTL Hold Time`: 0 to 100 [50]
- `TAP/CTL Twist Rise Time`: 0 to 100 [50]
- `TAP/CTL Twist Fall Time`: 0 to 100 [50]
- `CTL1 Function`: 0 to 8 [0]
- `CTL1 Mode`: `OFF`, `ON` [OFF]
- `CTL1 Hold Time`: 0 to 100 [50]
- `CTL1 Twist Rise time`: 0 to 100 [50]
- `CTL1 Twist Fall time`: 0 to 100 [50]
- `CTL2 Function`: 0 to 8 [0]
- `CTL2 Mode`: `OFF`, `ON` [OFF]
- `CTL2 Hold Time`: 0 to 100 [50]
- `CTL2 Rise Time`: 0 to 100 [50]
- `CTL2 Fall Time`: 0 to 100 [50]
- `Exp Pedal Function`: 0 to 14 [0]
- `Exp target min`: -100 to 2400000 [0]
- `Exp target max`: -100 to 2400000 [0]

**Assign 1**

- `Assign1 sw`: `OFF`, `ON` [OFF]
- `Assign1 source`: 0 to 69 [0]
- `Assign1 souce mode`: `OFF`, `ON` [ON]
- `Assign1 target`: 0 to 109 [0]
- `Assign1 target min`: -100 to 2400000 [0]
- `Assign1 target max`: -100 to 2400000 [1]
- `Assign1 Active Range LO`: 0 to 126 [0]
- `Assign1 Active Range HI`: 1 to 127 [127]
- `Assign1 Wave Rate`: 0 to 113 [50]
- `Assign1 Wave Form`: 0 to 2 [0]
- `Assign1 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign1 Internal Pedal Time`: 0 to 100 [30]
- `Assign1 Internal Pedal Curver`: 0 to 2 [0]

**Assign 2**

- `Assign2 sw`: `OFF`, `ON` [OFF]
- `Assign2 source`: 0 to 69 [0]
- `Assign2 souce mode`: `OFF`, `ON` [ON]
- `Assign2 target`: 0 to 109 [0]
- `Assign2 target min`: -100 to 2400000 [0]
- `Assign2 target max`: -100 to 2400000 [1]
- `ASSIGN2 Active Range LO`: 0 to 126 [0]
- `ASSIGN2 Active Range HI`: 1 to 127 [127]
- `ASSIGN2 Wave Rate`: 0 to 113 [50]
- `ASSIGN2 Wave Form`: 0 to 2 [0]
- `ASSIGN2 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign2 Internal Pedal Time`: 0 to 100 [30]
- `Assign2 Internal Pedal Curver`: 0 to 2 [0]

**Assign 3**

- `Assign3 sw`: `OFF`, `ON` [OFF]
- `Assign3 source`: 0 to 69 [0]
- `Assign3 souce mode`: `OFF`, `ON` [ON]
- `Assign3 target`: 0 to 109 [0]
- `Assign3 target min`: -100 to 2400000 [0]
- `Assign3 target max`: -100 to 2400000 [1]
- `ASSIGN3 Active Range LO`: 0 to 126 [0]
- `ASSIGN3 Active Range HI`: 1 to 127 [127]
- `ASSIGN3 Wave Rate`: 0 to 113 [50]
- `ASSIGN3 Wave Form`: 0 to 2 [0]
- `ASSIGN3 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign3 Internal Pedal Time`: 0 to 100 [30]
- `Assign3 Internal Pedal Curver`: 0 to 2 [0]

**Assign 4**

- `Assign4 sw`: `OFF`, `ON` [OFF]
- `Assign4 source`: 0 to 69 [0]
- `Assign4 souce mode`: `OFF`, `ON` [ON]
- `Assign4 target`: 0 to 109 [0]
- `Assign4 target min`: -100 to 2400000 [0]
- `Assign4 target max`: -100 to 2400000 [1]
- `ASSIGN4 Active Range LO`: 0 to 126 [0]
- `ASSIGN4 Active Range HI`: 1 to 127 [127]
- `ASSIGN4 Wave Rate`: 0 to 113 [50]
- `ASSIGN4 Wave Form`: 0 to 2 [0]
- `ASSIGN4 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign4 Internal Pedal Time`: 0 to 100 [30]
- `Assign4 Internal Pedal Curver`: 0 to 2 [0]

**Assign 5**

- `ASSIGN5 sw`: `OFF`, `ON` [OFF]
- `ASSIGN5 source`: 0 to 69 [0]
- `ASSIGN5 souce mode`: `OFF`, `ON` [ON]
- `ASSIGN5 target`: 0 to 109 [0]
- `ASSIGN5 target min`: -100 to 2400000 [0]
- `ASSIGN5 target max`: -100 to 2400000 [1]
- `ASSIGN5 Active Range LO`: 0 to 126 [0]
- `ASSIGN5 Active Range HI`: 1 to 127 [127]
- `ASSIGN5 Wave Rate`: 0 to 113 [50]
- `ASSIGN5 Wave Form`: 0 to 2 [0]
- `ASSIGN5 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign5 Internal Pedal Time`: 0 to 100 [30]
- `Assign5 Internal Pedal Curver`: 0 to 2 [0]

**Assign 6**

- `ASSIGN6 sw`: `OFF`, `ON` [OFF]
- `ASSIGN6 source`: 0 to 69 [0]
- `ASSIGN6 souce mode`: `OFF`, `ON` [ON]
- `ASSIGN6 target`: 0 to 109 [0]
- `ASSIGN6 target min`: -100 to 2400000 [0]
- `ASSIGN6 target max`: -100 to 2400000 [1]
- `ASSIGN6 Active Range LO`: 0 to 126 [0]
- `ASSIGN6 Active Range HI`: 1 to 127 [127]
- `ASSIGN6 Wave Rate`: 0 to 113 [50]
- `ASSIGN6 Wave Form`: 0 to 2 [0]
- `ASSIGN6 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign6 Internal Pedal Time`: 0 to 100 [30]
- `Assign6 Internal Pedal Curver`: 0 to 2 [0]

**Assign 7**

- `ASSIGN7 sw`: `OFF`, `ON` [OFF]
- `ASSIGN7 source`: 0 to 69 [0]
- `ASSIGN7 souce mode`: `OFF`, `ON` [ON]
- `ASSIGN7 target`: 0 to 109 [0]
- `ASSIGN7 target min`: -100 to 2400000 [0]
- `ASSIGN7 target max`: -100 to 2400000 [1]
- `ASSIGN7 Active Range LO`: 0 to 126 [0]
- `ASSIGN7 Active Range HI`: 1 to 127 [127]
- `ASSIGN7 Wave Rate`: 0 to 113 [50]
- `ASSIGN7 Wave Form`: 0 to 2 [0]
- `ASSIGN7 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign7 Internal Pedal Time`: 0 to 100 [30]
- `Assign7 Internal Pedal Curver`: 0 to 2 [0]

**Assign 8**

- `ASSIGN8 sw`: `OFF`, `ON` [OFF]
- `ASSIGN8 source`: 0 to 69 [0]
- `ASSIGN8 souce mode`: `OFF`, `ON` [ON]
- `ASSIGN8 target`: 0 to 109 [0]
- `ASSIGN8 target min`: -100 to 2400000 [0]
- `ASSIGN8 target max`: -100 to 2400000 [1]
- `ASSIGN8 Active Range LO`: 0 to 126 [0]
- `ASSIGN8 Active Range HI`: 1 to 127 [127]
- `ASSIGN8 Wave Rate`: 0 to 113 [50]
- `ASSIGN8 Wave Form`: 0 to 2 [0]
- `ASSIGN8 Internal Pedal Trigger`: 0 to 68 [0]
- `Assign8 Internal Pedal Time`: 0 to 100 [30]
- `Assign8 Internal Pedal Curver`: 0 to 2 [0]

### System settings (`system`)

- `Global Output Mode`: 0 to 2 [0]
- `Bank wait mode`: `OFF`, `ON` [OFF]
- `Bank Extent Begin`: 0 to 98 [0]
- `Bank Extent End`: 0 to 98 [98]
- `Global Knob Lock Switch`: `OFF`, `ON` [OFF]
- `Global Knob Mode`: `OFF`, `ON` [OFF]
- `Global Bypass Mode`: `OFF`, `ON` [OFF]
- `Global Pedal Action`: `OFF`, `ON` [OFF]
- `Foot Switch Mode`: 0 to 3 [0]
- `Global Display Type`: 0 to 3 [0]
- `MIDI Rx Channel`: 0 to 16 [0]
- `MIDI Tx Channel`: 0 to 17 [16]
- `MIDI Program Change IN`: `OFF`, `ON` [ON]
- `MIDI Program Change OUT`: `OFF`, `ON` [ON]
- `MIDI Bank Select OUT`: `OFF`, `ON` [OFF]
- `MIDI Control Change IN`: `OFF`, `ON` [ON]
- `MIDI Control Change OUT`: `OFF`, `ON` [ON]
- `MIDI Reverb Time Control Change`: 0 to 63 [17]
- `MIDI Reverb Pre-Delay Control Change`: 0 to 63 [18]
- `MIDI Reverb E.Level Control Change`: 0 to 63 [19]
- `MIDI Reverb Low Control Change`: 0 to 63 [20]
- `MIDI Reverb High Control Change`: 0 to 63 [21]
- `MIDI Delay Time Control Change`: 0 to 63 [22]
- `MIDI Delay Pre-Delay Control Change`: 0 to 63 [23]
- `MIDI Delay E.Level Control Change`: 0 to 63 [24]
- `MIDI Delay Low Control Change`: 0 to 63 [25]
- `MIDI Delay High Control Change`: 0 to 63 [26]
- `MIDI EFFECT ON/OFF Control Change`: 0 to 63 [27]
- `MIDI EFFECT A ON/OFF Control Change`: 0 to 63 [28]
- `MIDI EFFECT B ON/OFF Control Change`: 0 to 63 [29]
- `MIDI CTL1 Control Change`: 0 to 63 [48]
- `MIDI CTL2 Control Change`: 0 to 63 [49]
- `MIDI EXP Control Change`: 0 to 63 [16]
- `MIDI Sync`: 0 to 3 [0]
- `MIDI source of realtime message to output`: 0 to 2 [0]
- `MIDI In THRU into Out`: 0 to 3 [2]
- `USB In THRU into Out`: 0 to 3 [0]
- `effect sw`: `OFF`, `ON` [ON]
- `effect sw2`: `OFF`, `ON` [OFF]
- `dummy1`: 0 to 100 [0]
- `dummy1`: 0 to 100 [0]
- `dummy1`: 0 to 100 [0]
- `dummy1`: 0 to 100 [0]

## The GigRig G3S pedal switcher (`g3s`)

The G3S is edited through a **backup file** the user loads in the MIDI Pilot and restores in the switcher with its editor. It has up to 99 banks of 28 presets (14 on the footswitches, the rest hidden), 12 loops, and each preset sends up to 20 MIDI lines. Actions:

- `{ "g3s": "bank", "bank": n, "name": "LINE ONE|LINE TWO", "song_mode": true }`: names bank n (from 1; banks are added up to it), two lines of ten characters; `song_mode` makes the switcher show names instead of numbers.
- `{ "g3s": "preset", "bank": n, "switch": s, "name": "...", "loops": [..], "order": [..], "midi": [..] }`: switch s (1 to 28), `name` at most 9 characters, `loops` the loops on (1 to 12), `order` the twelve loops in the order of the signal, `midi` replaces the preset's MIDI lines: each `kind` one of "PC down", "CC down", "PC up", "CC up", "PC stomp", "CC stomp", "PC sequence", "CC sequence", "PC remote", "CC remote", "PC stomp on", "CC stomp on", "off", `channel` 1 to 16, then `program` for a PC kind or `controller` and `value` for a CC kind. Lines you leave out are cleared.
- `{ "g3s": "remove_bank", "bank": n }`.
- `{ "g3s": "loops", "pedals": { "1": [ { "name": "Fuzz Face", "kind": "fuzz" }, { "name": "Vox Wah", "kind": "wah" } ], "11": [ { "name": "DD-500", "kind": "delay", "device": "DD-500" } ] }, "signal_order": [1, 8, 10, 6, 2, 4, 3, 5, 7, 9, 11, 12], "bank_switches": [1, 8] }`: says which pedals sit in which loops of the switcher (kinds: fuzz, wah, booster, overdrive, distortion, compressor, equalizer, chorus, phaser, flanger, tremolo, octaver, delay, reverb, volume, other), which loop holds which MIDI device of the chain, the order the signal goes through the loops (what a new preset should use), and the footswitches that change banks. Kept in the program's settings, not in the switcher.

Which footswitches change banks is a setting of the switcher itself, not of this file: ask the user. When 1 and 8 do (a common choice), presets go on switches 2 to 7 and 9 to 14; otherwise all of 1 to 14 are presets. Many users keep bank 1 as their stompbox bank and give one bank to each song. Each line of `midi` has its own channel: a preset may drive several devices.

## The pedalboard file

The user may give you their **pedalboard file** (MIDI Pilot → List → "Export the pedalboard for an AI…", or `GET /pedalboard`): a script with a `pedalboard` section and `actions`. `pedalboard.devices` are the devices of the chain with their MIDI channels; `pedalboard.switcher` says which pedals sit in each loop of the switcher (`loops`, by loop number, each pedal with its `name`, `kind` and the `device` of the chain when it is MIDI), the order the signal goes through the loops (`signal_order`) and the footswitches that change banks (`bank_switches`); `mode` and `numbering` say how songs are sent. The `actions` are everything known, in the format above: the songs, the switcher's loops and backup (banks and presets), and each memory of the pedals with all its values and their program change maps. Read it before writing: a preset turns loops on by their number, so the loops tell you which pedal each number is; a `device` in a loop is where to send program changes; use the memories that exist (and their numbers in the program change map) before writing new ones; keep the user's conventions (bank 1, the bank switches, the signal order).

## The songs of the MIDI Pilot (`song`)

The MIDI Pilot shows the songs of a concert as large buttons; choosing one sends MIDI to the user's devices. In the switcher's mode a song sends its `number` (a bank select or a program change to the switcher); in the chain's mode it sends its `messages`, each to a device of the user's chain by the device's name (ask the user for the names, or `GET /devices` on the API: for example "DD-500" on channel 1 and "RV-500" on channel 2). Actions:

- `{ "song": "add", "artist": "Pink Floyd", "title": "Comfortably Numb", "display": "COMF NUMB", "number": 12, "messages": [ { "device": "DD-500", "program": 4, "delay_ms": 20 }, { "device": "RV-500", "bank": 0, "program": 2 }, { "device": "G3S", "controller": 0, "value": 12 } ] }`: adds a song (`display` and `number` optional; the number follows the last song's when left out). A message is a `program` (with `bank` for a bank select before it), or a `controller` with a `value`, or a `sysex` in hexadecimal; `delay_ms` is a pause after it (up to 500).
- `{ "song": "set", "artist": "…", "title": "…", "number": 13, "display": "…", "messages": [ … ] }`: changes the song of that artist and title (compared up to case and signs); only the fields given change, `messages` replaces them all.
- `{ "song": "remove", "artist": "…", "title": "…" }`.

**Bank numbering.** What a switcher needs to reach a bank is its own business and the user's settings: never assume an offset. In the switcher's mode the program sends the song's `number` as a bank select (controller 0) or a program change, minus one when the user ticked "Numbers from 1" (`GET /songs` says so under `switcher`); in the chain's mode the song's `messages` go out as written. Copy the convention of the user's existing songs (their export, or `GET /songs`): if their bank 12 is reached by `number` 12 or by controller 0 value 12, do the same for new songs; when there are no songs yet, ask the user, and have them try one song with "Send now" before writing many. A CSV file imported in the MIDI Pilot follows the same rule: its number column is the song's `number`.

The user's list exported for you (List → "Export the songs for an AI…", or `GET /songs`) is made of `add` actions you can change and send back as `set` actions.

## Local API (for an agent on the user's computer)

When the user turns it on (MIDI Pilot → Settings → "API for an AI…"), the program listens on `http://127.0.0.1:<port>/` with a token the user gives you: send `Authorization: Bearer <token>`. Bodies and answers use the format above.

- `GET /` what runs; `GET /guide` this text; `GET /devices` the devices of the chain; `GET /pedals/{key}` the parameters.
- `GET /pedals/{key}/memories` the names; `GET /pedals/{key}/memories/{50C}` a memory as an action; `PUT /pedals/{key}/memories/{50C}` with `{ "name", "from", "set" }` writes it (the user confirms in the window unless they allowed unattended writes); `POST /pedals/{key}/try` with `{ "set" }` plays the values without writing.
- `GET|PUT /pedals/{key}/pc_map`, `GET|PUT /pedals/{key}/system`.
- `GET /g3s` the backup loaded as actions; `POST /g3s/actions` with `{ "actions": [..] }`; `POST /g3s/save` writes the backup file.
- `GET /songs` the songs of the list as actions; `GET /pedalboard` the whole pedalboard file.
- `POST /actions` with a whole script (songs included).

Answers: 200 done, 202 waiting for the user's confirmation, 400 a wrong script (the reason in `error`), 401 token, 404, 409 the module is busy, 503 the MIDI ports are not open.
