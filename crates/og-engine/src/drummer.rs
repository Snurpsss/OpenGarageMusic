//! The virtual drummer: turns a handful of settings into a drum part.
//! The same settings always give the same part.

use crate::model::{DrumStyle, DrummerParams, HatMode, Note, Perc};
use crate::synth::{gm, Rng};

/// One bar on a sixteenth-note grid. In `kick` and `snare`, 2 is an accent,
/// 1 a normal hit. The `_extra` rows are chances (in percent) of an added hit
/// when complexity is at its maximum.
struct Groove {
    kick: [u8; 16],
    kick_extra: [u8; 16],
    snare: [u8; 16],
    snare_extra: [u8; 16],
    /// Hi-hat steps played at low complexity, and those added at high complexity.
    hat: [u8; 16],
    hat_extra: [u8; 16],
    /// Steps where the hi-hat opens.
    open: [u8; 16],
    /// Fraction of a sixteenth by which off-beats are delayed even without swing.
    swing: f32,
}

fn groove(style: DrumStyle) -> Groove {
    match style {
        DrumStyle::Rock => Groove {
            kick: [2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 1, 0, 0, 0, 0, 0],
            kick_extra: [0, 0, 0, 30, 0, 0, 40, 20, 0, 0, 0, 30, 0, 0, 50, 20],
            snare: [0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
            snare_extra: [0, 0, 20, 0, 0, 0, 0, 40, 0, 40, 0, 0, 0, 0, 0, 30],
            hat: [2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0],
            hat_extra: [0, 60, 0, 60, 0, 60, 0, 60, 0, 60, 0, 60, 0, 60, 0, 60],
            open: [0; 16],
            swing: 0.0,
        },
        DrumStyle::Pop => Groove {
            kick: [2, 0, 0, 0, 0, 0, 1, 0, 2, 0, 0, 0, 0, 0, 0, 0],
            kick_extra: [0, 0, 0, 20, 0, 0, 0, 30, 0, 0, 50, 0, 0, 30, 0, 0],
            snare: [0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
            snare_extra: [0, 0, 0, 0, 0, 0, 0, 20, 0, 30, 0, 0, 0, 0, 0, 40],
            hat: [2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0],
            hat_extra: [0, 40, 0, 40, 0, 40, 0, 40, 0, 40, 0, 40, 0, 40, 0, 40],
            open: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0],
            swing: 0.0,
        },
        DrumStyle::HipHop => Groove {
            kick: [2, 0, 0, 0, 0, 0, 0, 1, 0, 0, 2, 0, 0, 0, 0, 0],
            kick_extra: [0, 0, 0, 40, 0, 0, 30, 0, 0, 30, 0, 0, 0, 40, 0, 30],
            snare: [0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
            snare_extra: [0, 0, 0, 0, 0, 0, 0, 0, 0, 20, 0, 0, 0, 0, 0, 30],
            hat: [2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0],
            hat_extra: [0, 30, 0, 50, 0, 30, 0, 70, 0, 30, 0, 50, 0, 30, 0, 80],
            open: [0; 16],
            swing: 0.3,
        },
        DrumStyle::Electro => Groove {
            kick: [2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0],
            kick_extra: [0, 0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 40, 0],
            snare: [0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
            snare_extra: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 40],
            hat: [0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 2, 0, 0, 0, 2, 0],
            hat_extra: [50, 40, 0, 40, 50, 40, 0, 40, 50, 40, 0, 40, 50, 40, 0, 40],
            open: [0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 1, 0],
            swing: 0.0,
        },
        DrumStyle::Funk => Groove {
            kick: [2, 0, 0, 1, 0, 0, 1, 0, 0, 0, 2, 0, 0, 1, 0, 0],
            kick_extra: [0, 30, 0, 0, 0, 0, 0, 30, 0, 40, 0, 0, 0, 0, 0, 40],
            snare: [0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0],
            snare_extra: [0, 40, 50, 0, 0, 30, 0, 60, 0, 60, 0, 40, 0, 0, 30, 50],
            hat: [2, 1, 1, 1, 2, 1, 1, 1, 2, 1, 1, 1, 2, 1, 1, 1],
            hat_extra: [0; 16],
            open: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
            swing: 0.1,
        },
        DrumStyle::Jazz => Groove {
            kick: [1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
            kick_extra: [0, 0, 0, 0, 0, 0, 30, 0, 0, 0, 0, 0, 0, 0, 30, 0],
            snare: [0; 16],
            snare_extra: [0, 0, 30, 0, 0, 0, 40, 0, 0, 0, 30, 0, 0, 0, 50, 0],
            hat: [2, 0, 0, 0, 2, 0, 1, 0, 2, 0, 0, 0, 2, 0, 1, 0],
            hat_extra: [0, 0, 40, 0, 0, 0, 0, 0, 0, 0, 40, 0, 0, 0, 0, 0],
            open: [0; 16],
            swing: 0.0,
        },
        DrumStyle::Latin => Groove {
            kick: [2, 0, 0, 1, 0, 0, 2, 0, 2, 0, 0, 1, 0, 0, 2, 0],
            kick_extra: [0; 16],
            snare: [1, 0, 0, 1, 0, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0],
            snare_extra: [0, 0, 30, 0, 0, 30, 0, 0, 30, 0, 0, 0, 0, 0, 30, 0],
            hat: [2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0, 2, 0, 1, 0],
            hat_extra: [0, 50, 0, 50, 0, 50, 0, 50, 0, 50, 0, 50, 0, 50, 0, 50],
            open: [0; 16],
            swing: 0.0,
        },
    }
}

/// Builds the notes of a drummer region of `length` beats.
pub fn generate(p: &DrummerParams, length: f64, bar_len: f64) -> Vec<Note> {
    generate_following(p, length, bar_len, None)
}

/// As `generate`, but with the kick drum played where `kicks` say (beats from
/// the start of the region, moved to the nearest sixteenth) instead of where
/// the style puts it: the drummer "follows" another instrument.
pub fn generate_following(p: &DrummerParams, length: f64, bar_len: f64, kicks: Option<&[f64]>) -> Vec<Note> {
    let followed: Option<Vec<i64>> = kicks.map(|beats| beats.iter().map(|beat| (beat / 0.25).round() as i64).collect());
    let g = groove(p.style);
    let bar_len = bar_len.max(0.25);
    let steps = ((bar_len * 4.0).round() as usize).max(1);
    let bars = ((length / bar_len).ceil() as usize).min(2000);
    let complexity = p.complexity.clamp(0.0, 1.0);
    let level = 0.5 + 0.5 * p.loudness.clamp(0.0, 1.0);
    let jazz = p.style == DrumStyle::Jazz;
    // Latin and jazz play the "snare" row as rim clicks and soft comping.
    let snare_pitch = if p.style == DrumStyle::Latin { gm::RIM } else { gm::SNARE };
    let fill_every = match p.fills {
        f if f < 0.05 => 0,
        f if f < 0.4 => 8,
        f if f < 0.75 => 4,
        _ => 2,
    };
    let fill_steps = if p.fills > 0.6 && complexity > 0.5 { 8.min(steps) } else { 4.min(steps) };

    let mut notes = Vec::new();
    let hit = |notes: &mut Vec<Note>, start: f64, pitch: u8, vel: f32| {
        if start < length {
            let velocity = (vel * level).round().clamp(1.0, 127.0) as u8;
            notes.push(Note { start, length: 0.2, pitch, velocity, string: None, ..Default::default() });
        }
    };

    for bar in 0..bars {
        let mut rng = Rng(p.seed.wrapping_mul(7919).wrapping_add(bar as u32 * 104_729).wrapping_add(17));
        let filling = fill_every > 0 && (bar + 1) % fill_every == 0;
        for step in 0..steps {
            let s = step % 16;
            let mut t = (bar as f64) * bar_len + step as f64 * 0.25;
            // Swing delays the off-beats: every other sixteenth, or every other
            // eighth for jazz where the feel is in triplets.
            let swing = (p.swing + g.swing).clamp(0.0, 1.0);
            if jazz {
                if s % 4 == 2 {
                    t += f64::from(0.6 + 0.4 * swing) * 0.1667;
                }
            } else if s % 2 == 1 {
                t += f64::from(swing) * 0.0833;
            }
            let chance = |rng: &mut Rng, percent: u8| rng.unit() * 100.0 < f32::from(percent) * complexity;

            if filling && step >= steps - fill_steps {
                // A run down the toms, getting louder, ending on the snare.
                let i = step - (steps - fill_steps);
                let pitch = match (i * 4 / fill_steps, complexity > 0.3) {
                    (_, false) | (0, _) => gm::SNARE,
                    (1, _) => gm::TOM_HIGH,
                    (2, _) => gm::TOM_MID,
                    _ => gm::TOM_LOW,
                };
                if complexity > 0.25 || i.is_multiple_of(2) {
                    hit(&mut notes, t, pitch, 80.0 + 40.0 * i as f32 / fill_steps as f32);
                }
                if i.is_multiple_of(4) && p.kick_snare {
                    hit(&mut notes, t, gm::KICK, 100.0);
                }
                continue;
            }

            if p.kick_snare {
                match &followed {
                    Some(steps_played) => {
                        // The kick lands with the other instrument; the first beat of
                        // each bar is a little stronger.
                        let at = (bar * steps + step) as i64;
                        if steps_played.contains(&at) {
                            hit(&mut notes, t, gm::KICK, if step == 0 { 118.0 } else { 104.0 });
                        }
                    }
                    None => {
                        if g.kick[s] > 0 || chance(&mut rng, g.kick_extra[s]) {
                            hit(&mut notes, t, gm::KICK, if g.kick[s] == 2 { 118.0 } else { 98.0 });
                        }
                    }
                }
                if g.snare[s] > 0 {
                    hit(&mut notes, t, snare_pitch, if g.snare[s] == 2 { 116.0 } else { 96.0 });
                } else if chance(&mut rng, g.snare_extra[s]) {
                    // Ghost note.
                    hit(&mut notes, t, snare_pitch, 42.0 + 14.0 * rng.unit());
                }
            }

            let hat_on = g.hat[s] > 0 || (complexity > 0.55 && chance(&mut rng, g.hat_extra[s]));
            if hat_on {
                let vel = if g.hat[s] == 2 { 96.0 } else { 70.0 } + 8.0 * rng.bipolar();
                match p.hats {
                    HatMode::Off => {}
                    HatMode::Hats if jazz => {
                        // Jazz keeps time on the ride; the hi-hat closes on two and four.
                        hit(&mut notes, t, gm::RIDE, vel);
                        if s % 8 == 4 {
                            hit(&mut notes, t, gm::HAT_CLOSED, 70.0);
                        }
                    }
                    HatMode::Hats => {
                        let open = g.open[s] > 0 && complexity > 0.2;
                        hit(&mut notes, t, if open { gm::HAT_OPEN } else { gm::HAT_CLOSED }, vel);
                    }
                    HatMode::Ride => hit(&mut notes, t, gm::RIDE, vel),
                    HatMode::Toms => {
                        let pitch = if s % 8 < 4 { gm::TOM_MID } else { gm::TOM_LOW };
                        if s % 2 == 0 {
                            hit(&mut notes, t, pitch, vel);
                        }
                    }
                }
            }

            match p.perc {
                Perc::Off => {}
                Perc::Tambourine if s % 4 == 2 => hit(&mut notes, t, gm::TAMBOURINE, 84.0),
                Perc::Shaker if s % 2 == 0 || complexity > 0.4 => {
                    hit(&mut notes, t, gm::SHAKER, if s % 4 == 0 { 78.0 } else { 54.0 });
                }
                Perc::Clap if s % 8 == 4 => hit(&mut notes, t, gm::CLAP, 100.0),
                _ => {}
            }
        }
        // A crash marks the bar after a fill.
        if filling && bar + 1 < bars {
            hit(&mut notes, (bar as f64 + 1.0) * bar_len, gm::CRASH, 108.0);
        }
    }
    notes.sort_by(|a, b| a.start.total_cmp(&b.start));
    notes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_inside_the_region() {
        for style in DrumStyle::ALL {
            let p = DrummerParams { style, complexity: 0.9, fills: 0.9, ..Default::default() };
            let a = generate(&p, 32.0, 4.0);
            let b = generate(&p, 32.0, 4.0);
            assert_eq!(a, b);
            assert!(a.len() > 30, "{style:?} {}", a.len());
            assert!(a.iter().all(|n| n.start >= 0.0 && n.start < 32.0 && n.velocity > 0));
        }
    }

    #[test]
    fn settings_shape_the_part() {
        let base = DrummerParams::default();
        let simple = generate(&DrummerParams { complexity: 0.0, fills: 0.0, ..base }, 16.0, 4.0);
        let busy = generate(&DrummerParams { complexity: 1.0, fills: 0.0, ..base }, 16.0, 4.0);
        assert!(busy.len() > simple.len());

        let no_kit = generate(&DrummerParams { kick_snare: false, fills: 0.0, ..base }, 16.0, 4.0);
        assert!(no_kit.iter().all(|n| n.pitch != gm::KICK && n.pitch != gm::SNARE));

        let quiet = generate(&DrummerParams { loudness: 0.0, ..base }, 16.0, 4.0);
        let loud = generate(&DrummerParams { loudness: 1.0, ..base }, 16.0, 4.0);
        assert!(quiet.iter().map(|n| u32::from(n.velocity)).sum::<u32>() < loud.iter().map(|n| u32::from(n.velocity)).sum::<u32>());

        let fills = generate(&DrummerParams { fills: 1.0, complexity: 0.8, ..base }, 16.0, 4.0);
        assert!(fills.iter().any(|n| n.pitch == gm::CRASH));
        assert!(fills.iter().any(|n| n.pitch == gm::TOM_LOW));

        let three_four = generate(&base, 12.0, 3.0);
        assert!(three_four.iter().filter(|n| n.pitch == gm::KICK && n.start % 3.0 == 0.0).count() >= 4);
    }
}
