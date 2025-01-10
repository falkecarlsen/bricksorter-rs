#![allow(unreachable_code)]

extern crate ev3dev_lang_rust;

use core::time::Duration;
use ev3dev_lang_rust::motors::{MediumMotor, MotorPort, TachoMotor};
use ev3dev_lang_rust::sensors::{ColorSensor, SensorPort};
use ev3dev_lang_rust::Ev3Result;
use std::sync::{Arc, RwLock};
use std::thread;

#[derive(PartialEq, Debug, Copy, Clone, strum_macros::Display)]
enum BrickColor {
    None = 0,
    Black,
    Blue,
    Green,
    Yellow,
    Red,
    White,
    Brown,
}

trait GetColorEnumTrait {
    fn get_color_enum(&self) -> BrickColor;
}

impl GetColorEnumTrait for ColorSensor {
    fn get_color_enum(&self) -> BrickColor {
        match self.get_color() {
            Ok(0) => BrickColor::None,
            Ok(1) => BrickColor::Black,
            Ok(2) => BrickColor::Blue,
            Ok(3) => BrickColor::Green,
            Ok(4) => BrickColor::Yellow,
            Ok(5) => BrickColor::Red,
            Ok(6) => BrickColor::White,
            Ok(7) => BrickColor::Brown,
            _ => BrickColor::None,
        }
    }
}

#[derive(Clone)]
struct SensorDebouncer {
    readings: Vec<BrickColor>,
    window_size: usize,
    most_likely_brick: BrickColor,
}

impl SensorDebouncer {
    fn new(window_size: usize) -> Self {
        let mut readings: Vec<BrickColor> = Vec::new();
        // prime with 15 readings of None
        for _ in 0..window_size {
            readings.push(BrickColor::None);
        }
        Self {
            readings,
            window_size,
            most_likely_brick: BrickColor::None,
        }
    }

    fn update(&mut self, new_reading: BrickColor) {
        self.readings.push(new_reading);
        // remove oldest reading beyond window size
        if self.readings.len() > self.window_size {
            self.readings.remove(0);
        }
        // most likely brick is the one that has been detected most frequently, use filter on readings to get most common
        self.most_likely_brick = *self
            .readings
            .iter()
            .rev()
            .take(self.window_size)
            .max_by_key(|&x| self.readings.iter().filter(|&y| *y == *x).count())
            .unwrap_or(&BrickColor::None);
    }

    fn get_most_likely_brick(&self) -> BrickColor {
        self.most_likely_brick
    }

    #[allow(dead_code)]
    fn get_readings(&self) -> Vec<BrickColor> {
        self.readings.clone()
    }
}

fn run_to_abs_pos(motor: &TachoMotor, angle: i32, speed: i32) -> Ev3Result<()> {
    const TIMEOUT: i32 = 1000;

    println!(
        "Running to rel angle: {} (curr: {}) with speed {}",
        angle,
        motor.get_position()?,
        speed
    );
    motor.set_speed_sp(speed)?;
    motor.run_to_rel_pos(Some(angle))?;
    motor.wait_until_not_moving(Some(Duration::from_millis(TIMEOUT as u64)));
    Ok(())
}

fn schedule_timed_piston(
    kicker: &TachoMotor,
    duration: Duration,
    brick_color: BrickColor,
) -> Ev3Result<()> {
    // encode direction (left, right, or no kick) by scalar
    let direction = match brick_color {
        BrickColor::None => 0,
        BrickColor::Black => 0,
        BrickColor::Blue => -1,
        BrickColor::Green => 0,
        BrickColor::Yellow => 1,
        BrickColor::Red => -1,
        BrickColor::White => 0,
        BrickColor::Brown => 0,
    };

    let time_now = std::time::Instant::now();

    let mut angular_movement = 0;

    // do prime operation
    run_to_abs_pos(&kicker, 90 * direction, 50)?;
    angular_movement += 90 * direction;

    // delay
    std::thread::sleep(duration - time_now.elapsed());

    // kick
    run_to_abs_pos(&kicker, -45 * direction, 500)?;
    angular_movement -= 45 * direction;

    println!("Time to kick brick: {:?}", time_now.elapsed());

    // return to initial position
    run_to_abs_pos(&kicker, -angular_movement, 500)?;


    Ok(())
}

fn main() -> Ev3Result<()> {
    println!("Hello, EV3!");
    // Get large motor on port outA.
    let conveyor = MediumMotor::get(MotorPort::OutA)?;
    let kicker = TachoMotor::get(MotorPort::OutB)?;

    conveyor.run_direct()?;
    //conveyor.set_duty_cycle_sp(45)?;

    // reset kicker rotary encoder position (ensure piston is aimed at center of conveyor on init)
    let _first_pos = kicker.set_position(0)?;
    kicker.set_speed_sp(300)?;
    kicker.set_stop_action(TachoMotor::STOP_ACTION_HOLD)?; // ensure angle is held on kicker

    let s1 = ColorSensor::get(SensorPort::In1)?;
    let s2 = ColorSensor::get(SensorPort::In2)?;

    s1.set_mode_col_color()?;
    s2.set_mode_col_color()?;

    let debouncer_s1 = Arc::new(RwLock::new(SensorDebouncer::new(10)));
    let debouncer_s2 = Arc::new(RwLock::new(SensorDebouncer::new(10)));

    // thread for reading sensor data and running debouncer
    let debouncer_sensor_s1 = Arc::clone(&debouncer_s1);
    let debouncer_sensor_s2 = Arc::clone(&debouncer_s2);
    let debouncer_sensor_handle = thread::spawn(move || loop {
        debouncer_sensor_s1
            .write()
            .unwrap()
            .update(s1.get_color_enum());
        debouncer_sensor_s2
            .write()
            .unwrap()
            .update(s2.get_color_enum());
        std::thread::sleep(Duration::from_millis(16));
    });

    // thread for main loop only reading from debouncer
    let s1_debounce_reader = Arc::clone(&debouncer_s1);
    let s2_debounce_reader = Arc::clone(&debouncer_s2);
    let main_handle = thread::spawn(move || loop {
        println!(
            "Most likely bricks: s1: {: <8} s2: {: <8}",
            s1_debounce_reader.read().unwrap().get_most_likely_brick(),
            s2_debounce_reader.read().unwrap().get_most_likely_brick()
        );
        if s2_debounce_reader.read().unwrap().get_most_likely_brick() != BrickColor::None {
            let brick_color = s2_debounce_reader.read().unwrap().get_most_likely_brick();
            println!("Kicking a brick: {:?}", brick_color);
            schedule_timed_piston(&kicker, Duration::from_secs(3), brick_color).unwrap();
        };

        std::thread::sleep(Duration::from_millis(200));
    });

    debouncer_sensor_handle.join().unwrap();
    main_handle.join().unwrap();
    Ok(())
}
