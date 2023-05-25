use fugit::{Duration, ExtU64, Instant, MicrosDuration, MicrosDurationU64};
use heapless::spsc::Queue;

trait TimeBase {
    fn start() {}
    fn stop() {}
    fn pause() {}
    fn reset() {}
}

pub trait SyncClock<const TIMER_HZ: u32> {
    fn add_sample(
        &mut self,
        instant: Instant<u64, 1, TIMER_HZ>,
    ) -> Option<Duration<u64, 1, TIMER_HZ>>;
}

pub struct MidiSync<const TIMER_HZ: u32, const BUF_SIZE: usize> {
    pulse_duration: Option<Duration<u64, 1, TIMER_HZ>>,
    beat_duration: Option<Duration<u64, 1, TIMER_HZ>>,
    previous_sample: Option<Instant<u64, 1, TIMER_HZ>>,
    pulses_per_quarter_note: u32,
    //buf: heapless::spsc::Queue<Duration<T, 1, TIMER_HZ>, BUF_SIZE>,
}

impl<const TIMER_HZ: u32, const BUF_SIZE: usize> MidiSync<TIMER_HZ, BUF_SIZE> {
    pub fn new() -> Self {
        MidiSync {
            pulse_duration: None,
            beat_duration: None,
            previous_sample: None,
            pulses_per_quarter_note: 24,
        }
    }
}
impl<const TIMER_HZ: u32, const BUF_SIZE: usize> SyncClock<TIMER_HZ>
    for MidiSync<TIMER_HZ, BUF_SIZE>
{
    fn add_sample(
        &mut self,
        sample: Instant<u64, 1, TIMER_HZ>,
    ) -> Option<Duration<u64, 1, TIMER_HZ>> {
        match self.previous_sample {
            Some(prev_sample) => {
                // !TODO Make safe and add bounds check
                let dif = sample.checked_duration_since(prev_sample)?;
                match self.pulse_duration {
                    Some(dur) => {
                        self.pulse_duration = Some(dur.checked_add(dif)? / 2);
                        self.pulse_duration
                    }
                    None => None,
                }
            }
            None => {
                self.previous_sample = Some(sample);
                None
            }
        }
    }
}
