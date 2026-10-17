use egui::{Event, MouseWheelUnit, Pos2, TouchId, TouchPhase, Vec2};


const WINDOW: f64 = 0.1;

const STILL: f64 = 0.05;

const TAU: f32 = 0.35;

const START_SPEED: f32 = 150.0;

const STOP_SPEED: f32 = 10.0;

const MAX_SPEED: f32 = 8000.0;

const STALE: f64 = 0.1;


pub(crate) struct Fling {

    pub(crate) enabled: bool,

    samples: Vec<(f64, Vec2)>,

    touch_positions: Vec<(TouchId, Pos2)>,

    velocity: Vec2,

    at: f64,

    stalled: u8,

}


impl Default for Fling {

    fn default() -> Self {

        Self::new(!cfg!(any(target_os = "macos", target_arch = "wasm32")))

    }

}


impl Fling {

    pub(crate) fn new(enabled: bool) -> Self {

        Self { enabled, samples: Vec::new(), touch_positions: Vec::new(), velocity: Vec2::ZERO, at: 0.0, stalled: 0 }

    }


    pub(crate) fn gliding(&self) -> bool {

        self.velocity != Vec2::ZERO

    }


    pub(crate) fn stop(&mut self) {

        self.velocity = Vec2::ZERO;

        self.stalled = 0;

    }


    pub(crate) fn read(&mut self, events: &[Event], now: f64, glide: bool) {

        for event in events {

            match event {

                Event::MouseWheel { unit, delta, phase, modifiers } => {

                    self.stop();

                    if *unit != MouseWheelUnit::Point || modifiers.command || modifiers.ctrl || modifiers.shift {

                        self.samples.clear();

                        continue;

                    }

                    match phase {

                        TouchPhase::Start => {

                            self.samples.clear();

                            self.push(now, *delta);

                        }

                        TouchPhase::Move => self.push(now, *delta),

                        TouchPhase::End => {

                            if glide && self.enabled {

                                self.velocity = self.lift_velocity(now);

                                self.at = now;

                            }

                            self.samples.clear();

                        }

                        TouchPhase::Cancel => self.samples.clear(),

                    }

                }

                Event::Touch { id, phase, pos, .. } => {

                    match phase {

                        TouchPhase::Start => {

                            self.stop();

                            self.samples.clear();

                            self.touch_positions.retain(|(t_id, _)| t_id != id);

                            self.touch_positions.push((*id, *pos));

                        }

                        TouchPhase::Move => {

                            let count = self.touch_positions.len().max(1) as f32;

                            if let Some(prev) = self.touch_positions.iter_mut().find(|(t_id, _)| t_id == id) {

                                let delta = (*pos - prev.1) / count;

                                prev.1 = *pos;

                                self.push(now, delta);

                            } else {

                                self.touch_positions.push((*id, *pos));

                            }

                        }

                        TouchPhase::End => {

                            let count = self.touch_positions.len().max(1) as f32;

                            if let Some(pos_idx) = self.touch_positions.iter().position(|(t_id, _)| t_id == id) {

                                let prev_pos = self.touch_positions.remove(pos_idx).1;

                                let delta = (*pos - prev_pos) / count;

                                if delta.length_sq() > 0.0 {

                                    self.push(now, delta);

                                }

                            }

                            if self.touch_positions.is_empty() {

                                if glide && self.enabled {

                                    self.velocity = self.lift_velocity(now);

                                    self.at = now;

                                }

                                self.samples.clear();

                            }

                        }

                        TouchPhase::Cancel => {

                            self.touch_positions.retain(|(t_id, _)| t_id != id);

                            if self.touch_positions.is_empty() {

                                self.samples.clear();

                            }

                        }

                    }

                }

                _ => {}

            }

        }

    }


    fn push(&mut self, now: f64, delta: Vec2) {

        if !(delta.x.is_finite() && delta.y.is_finite()) {

            return;

        }

        match self.samples.last_mut() {

            Some((at, d)) if *at == now => *d += delta,

            _ => self.samples.push((now, delta)),

        }

        self.samples.retain(|(at, _)| now - at <= WINDOW);

    }


    fn lift_velocity(&self, now: f64) -> Vec2 {

        let recent: Vec<&(f64, Vec2)> = self.samples.iter().filter(|(at, _)| now - at <= WINDOW).collect();

        let (Some(first), Some(last)) = (recent.first(), recent.last()) else { return Vec2::ZERO };

        let span = (last.0 - first.0) as f32;

        if recent.len() < 2 || span <= 0.0 || now - last.0 > STILL {

            return Vec2::ZERO;

        }

        let moved = recent.iter().skip(1).fold(Vec2::ZERO, |sum, (_, d)| sum + *d);

        let v = moved / span;

        if !(v.x.is_finite() && v.y.is_finite()) || v.length() < START_SPEED {

            return Vec2::ZERO;

        }

        if v.length() > MAX_SPEED { v * (MAX_SPEED / v.length()) } else { v }

    }


    pub(crate) fn step(&mut self, now: f64) -> Vec2 {

        if !self.gliding() {

            return Vec2::ZERO;

        }

        let dt = now - self.at;

        if !(0.0..=STALE).contains(&dt) {

            self.stop();

            return Vec2::ZERO;

        }

        self.at = now;

        let decay = (-(dt as f32) / TAU).exp();

        let moved = self.velocity * TAU * (1.0 - decay);

        self.velocity *= decay;

        if self.velocity.length() < STOP_SPEED {

            self.stop();

        }

        moved

    }


    pub(crate) fn moved(&mut self, moved: bool) {

        if moved {

            self.stalled = 0;

        } else {

            self.stalled = self.stalled.saturating_add(1);

            if self.stalled >= 2 {

                self.stop();

            }

        }

    }

}


#[cfg(test)]

mod tests {

    use super::*;

    use egui::{Modifiers, vec2};


    fn wheel(dy: f32, phase: TouchPhase) -> Event {

        Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, dy), phase, modifiers: Modifiers::NONE }

    }


    fn swipe(fling: &mut Fling, dy: f32, moves: usize) -> f64 {

        let dt = 1.0 / 60.0;

        let mut now = 0.0;

        fling.read(&[wheel(dy, TouchPhase::Start)], now, true);

        for _ in 0..moves {

            now += dt;

            fling.read(&[wheel(dy, TouchPhase::Move)], now, true);

        }

        now += dt;

        fling.read(&[wheel(0.0, TouchPhase::End)], now, true);

        now

    }


    #[test]

    fn a_flick_glides_on_at_its_lift_speed_and_stops() {

        let mut fling = Fling::new(true);

        let mut now = swipe(&mut fling, -25.0, 6);

        assert!(fling.gliding());

        assert!((fling.velocity.y + 1500.0).abs() < 1.0, "{:?}", fling.velocity);

        let mut travelled = 0.0;

        let mut passes = 0;

        while fling.gliding() {

            now += 1.0 / 60.0;

            let d = fling.step(now);

            assert!(d.y <= 0.0 && d.x == 0.0, "keeps the swipe's direction: {d:?}");

            travelled += d.y;

            passes += 1;

            assert!(passes < 600, "the glide ends");

        }

        assert!((-travelled - 1500.0 * TAU).abs() < 10.0, "travelled {travelled}");

        assert_eq!(fling.step(now + 1.0 / 60.0), Vec2::ZERO);

    }


    #[test]

    fn a_slow_drag_or_a_pause_before_lifting_does_not_glide() {

        let mut fling = Fling::new(true);

        swipe(&mut fling, -1.0, 6);

        assert!(!fling.gliding(), "60 points a second is below the start speed");


        let mut fling = Fling::new(true);

        let dt = 1.0 / 60.0;

        fling.read(&[wheel(-25.0, TouchPhase::Start)], 0.0, true);

        fling.read(&[wheel(-25.0, TouchPhase::Move)], dt, true);

        fling.read(&[wheel(0.0, TouchPhase::End)], dt + 0.2, true);

        assert!(!fling.gliding(), "the finger rested before it lifted");

    }


    #[test]

    fn new_input_stops_a_glide() {

        let mut fling = Fling::new(true);

        let now = swipe(&mut fling, -25.0, 6);

        fling.read(&[wheel(-2.0, TouchPhase::Start)], now + 0.1, true);

        assert!(!fling.gliding(), "fingers back on the touchpad");


        let mut fling = Fling::new(true);

        let now = swipe(&mut fling, -25.0, 6);

        let notch = Event::MouseWheel { unit: MouseWheelUnit::Line, delta: vec2(0.0, -1.0), phase: TouchPhase::Move, modifiers: Modifiers::NONE };

        fling.read(&[notch], now + 0.1, true);

        assert!(!fling.gliding(), "a wheel notch");

    }


    #[test]

    fn no_glide_where_it_is_off_or_not_wanted() {

        let mut off = Fling::new(false);

        swipe(&mut off, -25.0, 6);

        assert!(!off.gliding(), "macOS and the web glide by themselves or not at all");


        let mut fling = Fling::new(true);

        let dt = 1.0 / 60.0;

        fling.read(&[wheel(-25.0, TouchPhase::Start)], 0.0, true);

        fling.read(&[wheel(-25.0, TouchPhase::Move)], dt, true);

        fling.read(&[wheel(-25.0, TouchPhase::Move)], 2.0 * dt, true);

        fling.read(&[wheel(0.0, TouchPhase::End)], 3.0 * dt, false);

        assert!(!fling.gliding(), "animations off, paging or a dialog");


        let mut zoom = Fling::new(true);

        let ctrl = |phase| Event::MouseWheel { unit: MouseWheelUnit::Point, delta: vec2(0.0, -25.0), phase, modifiers: Modifiers::COMMAND };

        zoom.read(&[ctrl(TouchPhase::Start)], 0.0, true);

        zoom.read(&[ctrl(TouchPhase::Move)], dt, true);

        zoom.read(&[ctrl(TouchPhase::Move)], 2.0 * dt, true);

        zoom.read(&[ctrl(TouchPhase::End)], 3.0 * dt, true);

        assert!(!zoom.gliding(), "a pinch or ⌘/Ctrl swipe zooms");

    }


    #[test]

    fn an_edge_a_stall_or_a_wild_speed_ends_or_caps_the_glide() {

        let mut fling = Fling::new(true);

        let now = swipe(&mut fling, -25.0, 6);

        fling.step(now + 1.0 / 60.0);

        fling.moved(false);

        assert!(fling.gliding(), "one pass may not show the move yet");

        fling.moved(false);

        assert!(!fling.gliding(), "against the edge");


        let mut fling = Fling::new(true);

        let now = swipe(&mut fling, -25.0, 6);

        assert_eq!(fling.step(now + 1.0), Vec2::ZERO, "a second-long gap (the tab was hidden)");

        assert!(!fling.gliding());


        let mut fling = Fling::new(true);

        swipe(&mut fling, -5000.0, 6);

        assert!((fling.velocity.length() - MAX_SPEED).abs() < 1.0, "{:?}", fling.velocity);


        let mut fling = Fling::new(true);

        swipe(&mut fling, f32::NAN, 6);

        assert!(!fling.gliding(), "a speed that isn't a number");

    }


    #[test]

    fn a_touch_flick_glides_on_and_stops() {

        let mut fling = Fling::new(true);

        let id = TouchId(0);

        let dt = 1.0 / 60.0;

        let mut now = 0.0;

        let start = egui::pos2(100.0, 500.0);

        fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::Start, pos: start, force: None }], now, true);

        for i in 1..=6 {

            now += dt;

            let pos = egui::pos2(100.0, 500.0 - 25.0 * i as f32);

            fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::Move, pos, force: None }], now, true);

        }

        now += dt;

        let end_pos = egui::pos2(100.0, 500.0 - 25.0 * 7.0);

        fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::End, pos: end_pos, force: None }], now, true);

        assert!(fling.gliding());

        assert!((fling.velocity.y + 1500.0).abs() < 1.0, "{:?}", fling.velocity);

        let mut travelled = 0.0;

        while fling.gliding() {

            now += 1.0 / 60.0;

            let d = fling.step(now);

            assert!(d.y <= 0.0 && d.x == 0.0);

            travelled += d.y;

        }

        assert!((-travelled - 1500.0 * TAU).abs() < 10.0);

    }


    #[test]

    fn touch_down_stops_ongoing_glide() {

        let mut fling = Fling::new(true);

        let id = TouchId(0);

        let dt = 1.0 / 60.0;

        let mut now = 0.0;

        fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::Start, pos: egui::pos2(100.0, 500.0), force: None }], now, true);

        for i in 1..=6 {

            now += dt;

            fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::Move, pos: egui::pos2(100.0, 500.0 - 25.0 * i as f32), force: None }], now, true);

        }

        now += dt;

        fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::End, pos: egui::pos2(100.0, 325.0), force: None }], now, true);

        assert!(fling.gliding());

        fling.read(&[Event::Touch { device_id: egui::TouchDeviceId(0), id, phase: TouchPhase::Start, pos: egui::pos2(100.0, 325.0), force: None }], now + 0.05, true);

        assert!(!fling.gliding());

    }

}

