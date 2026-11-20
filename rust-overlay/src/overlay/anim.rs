use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Copy)]
struct KeyAnimState {
    prev_pressed: bool,
    start_val: f32,
    current_val: f32,
    change_time: Instant,
}

pub struct AnimTracker {
    states: HashMap<u32, KeyAnimState>,
}

impl AnimTracker {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }

    #[inline]
    fn smoothstep(x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    }

    /// 현재 이징 계수 (0.0..=1.0) 와 애니메이션 진행 여부 반환
    pub fn sample(
        &mut self,
        vk: u32,
        now_pressed: bool,
        now: Instant,
        press_ms: f32,
        release_ms: f32,
    ) -> (f32, bool) {
        let entry = self.states.entry(vk).or_insert_with(|| KeyAnimState {
            prev_pressed: now_pressed,
            start_val: if now_pressed { 1.0 } else { 0.0 },
            current_val: if now_pressed { 1.0 } else { 0.0 },
            change_time: now,
        });

        if now_pressed != entry.prev_pressed {
            entry.prev_pressed = now_pressed;
            entry.start_val = entry.current_val;
            entry.change_time = now;
        }

        let target = if now_pressed { 1.0 } else { 0.0 };
        let duration_ms = if now_pressed {
            press_ms.max(1.0)
        } else {
            release_ms.max(1.0)
        };

        let elapsed_ms = now.duration_since(entry.change_time).as_secs_f32() * 1000.0;
        let raw = (elapsed_ms / duration_ms).clamp(0.0, 1.0);
        let s = Self::smoothstep(raw);
        let val = entry.start_val + (target - entry.start_val) * s;
        entry.current_val = val.clamp(0.0, 1.0);

        let animating = (entry.current_val - target).abs() > 0.002;
        if !animating {
            entry.current_val = target;
        }
        (entry.current_val, animating)
    }
}
