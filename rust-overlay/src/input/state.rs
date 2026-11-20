use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct PressStamp {
    pub release_time: Instant,
    pub duration_secs: f32,
}

#[derive(Debug)]
struct KeyRuntime {
    is_down: bool,
    total: u64,
    press_times: VecDeque<Instant>,
    press_start: Option<Instant>,
    press_durations: VecDeque<PressStamp>,
}

impl Default for KeyRuntime {
    fn default() -> Self {
        Self {
            is_down: false,
            total: 0,
            press_times: VecDeque::new(),
            press_start: None,
            press_durations: VecDeque::new(),
        }
    }
}

pub struct KeyStateManager {
    inner: RwLock<HashMap<u32, KeyRuntime>>,
}

impl KeyStateManager {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn set_total(&self, vk: u32, total: u64) {
        let mut map = self.inner.write();
        map.entry(vk).or_default().total = total;
    }

    /// [M-1] 입력 모드 전환 시 고착(stuck) 방지를 위해 눌림 상태 초기화
    pub fn clear_pressed_states(&self) {
        let mut map = self.inner.write();
        for kr in map.values_mut() {
            kr.is_down = false;
            kr.press_start = None;
        }
    }

    pub fn reset_totals(&self) {
        let mut map = self.inner.write();
        for kr in map.values_mut() {
            kr.total = 0;
            kr.press_times.clear();
            kr.press_durations.clear();
        }
    }

    pub fn on_key_event(&self, vk: u32, pressed: bool) -> bool {
        let now = Instant::now();
        let mut map = self.inner.write();
        let kr = map.entry(vk).or_default();

        // 자동 반복 입력 및 중복 이벤트 필터링
        if pressed && kr.is_down {
            return false;
        }
        if !pressed && !kr.is_down {
            return false;
        }

        kr.is_down = pressed;
        if pressed {
            kr.total += 1;
            kr.press_times.push_back(now);
            kr.press_start = Some(now);
            prune_old_presses(&mut kr.press_times, now);
        } else if let Some(start) = kr.press_start.take() {
            let dur = now.duration_since(start).as_secs_f32();
            if dur > 0.001 {
                kr.press_durations.push_back(PressStamp {
                    release_time: now,
                    duration_secs: dur,
                });
                // [L-3] 최대 트랙 높이(600px) / 최저 속도(60px/s) = 10초 이상 여유 보관 (15초)
                while let Some(front) = kr.press_durations.front() {
                    if now.duration_since(front.release_time) > Duration::from_secs(15) {
                        kr.press_durations.pop_front();
                    } else {
                        break;
                    }
                }
            }
        }
        true
    }

    pub fn is_down(&self, vk: u32) -> bool {
        self.inner.read().get(&vk).map(|k| k.is_down).unwrap_or(false)
    }

    pub fn get_total(&self, vk: u32) -> u64 {
        self.inner.read().get(&vk).map(|k| k.total).unwrap_or(0)
    }

    pub fn get_kps(&self, vk: u32) -> usize {
        let now = Instant::now();
        let map = self.inner.read();
        match map.get(&vk) {
            Some(kr) => kr
                .press_times
                .iter()
                .rev()
                .take_while(|t| now.duration_since(**t) <= Duration::from_secs(1))
                .count(),
            None => 0,
        }
    }

    pub fn get_press_start(&self, vk: u32) -> Option<Instant> {
        self.inner.read().get(&vk).and_then(|k| k.press_start)
    }

    pub fn get_press_durations(&self, vk: u32) -> Vec<PressStamp> {
        self.inner
            .read()
            .get(&vk)
            .map(|k| k.press_durations.iter().copied().collect())
            .unwrap_or_default()
    }

    pub fn collect_recent_timestamps(&self, unique_vks: &[u32], now: Instant) -> Vec<Instant> {
        let map = self.inner.read();
        let mut all = Vec::new();
        for &vk in unique_vks {
            if let Some(kr) = map.get(&vk) {
                for &t in kr.press_times.iter().rev() {
                    if now.duration_since(t) <= Duration::from_secs(1) {
                        all.push(t);
                    } else {
                        break;
                    }
                }
            }
        }
        all.sort_unstable();
        all
    }

    pub fn has_active_activity(&self, unique_vks: &[u32], max_track_age_secs: f32) -> bool {
        let now = Instant::now();
        let map = self.inner.read();
        for &vk in unique_vks {
            if let Some(kr) = map.get(&vk) {
                if kr.is_down {
                    return true;
                }
                if kr
                    .press_times
                    .back()
                    .is_some_and(|t| now.duration_since(*t) <= Duration::from_millis(1100))
                {
                    return true;
                }
                if kr.press_durations.back().is_some_and(|s| {
                    now.duration_since(s.release_time).as_secs_f32() <= max_track_age_secs
                }) {
                    return true;
                }
            }
        }
        false
    }
}

fn prune_old_presses(dq: &mut VecDeque<Instant>, now: Instant) {
    while let Some(front) = dq.front() {
        if now.duration_since(*front) > Duration::from_secs(1) {
            dq.pop_front();
        } else {
            break;
        }
    }
}
