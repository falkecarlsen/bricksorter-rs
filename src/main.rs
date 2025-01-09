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

fn run_to_abs_pos(motor: TachoMotor, position: i32, speed: i32) -> Ev3Result<()> {
    // check direction to go
    let current_pos = motor.get_position()?;
    if current_pos < position {
        motor.set_duty_cycle_sp(speed)?;
    } else {
        motor.set_duty_cycle_sp(-speed)?;
    }

    motor.wait(
        || motor.get_position().unwrap() >= position,
        Some(Duration::from_secs(5)),
    );
    motor.stop()?;
    Ok(())
}

fn schedule_timed_piston(kicker: TachoMotor, duration: Duration, brick_color: BrickColor) -> Ev3Result<()> {
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
    //
    let time_now = std::time::Instant::now();
    // do prime operation
    run_to_abs_pos(kicker.clone(), 90 * direction, 20)?;

    // delay for duration
    while time_now.elapsed() < duration {
        thread::sleep(Duration::from_millis(10));
    }

    // kick
    run_to_abs_pos(kicker.clone(), -90 * direction, 20)?;

    Ok(())
}


fn main() -> Ev3Result<()> {
    println!("Hello, EV3!");
    // Get large motor on port outA.
    let conveyor = MediumMotor::get(MotorPort::OutA)?;
    let kicker = TachoMotor::get(MotorPort::OutB)?;

    conveyor.run_direct()?;
    conveyor.set_duty_cycle_sp(45)?;

    // reset kicker rotary encoder position (ensure piston is aimed at center of conveyer on init)
    let _first_pos = kicker.set_position(0)?;

    kicker.run_direct()?;

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
    let main_handle = thread::spawn(move || {
        loop {
            println!(
                "Most likely bricks: s1: {: <8} s2: {: <8}",
                s1_debounce_reader.read().unwrap().get_most_likely_brick(),
                s2_debounce_reader.read().unwrap().get_most_likely_brick()
            );
            if s2_debounce_reader.read().unwrap().get_most_likely_brick() != BrickColor::None {
                let brick_color = s2_debounce_reader.read().unwrap().get_most_likely_brick();
                println!("Kicking a brick: {:?}", brick_color);
                schedule_timed_piston(kicker.clone(), Duration::from_secs(3), brick_color).unwrap();
            };

            std::thread::sleep(Duration::from_millis(200));
        }
    });

    debouncer_sensor_handle.join().unwrap();
    main_handle.join().unwrap();
    Ok(())
}
