extern crate ev3dev_lang_rust;

use ev3dev_lang_rust::motors::{MediumMotor, MotorPort};
use ev3dev_lang_rust::sensors::{ColorSensor, SensorPort};
use ev3dev_lang_rust::Ev3Result;
use std::sync::{Arc, RwLock};
use std::thread;

#[derive(PartialEq, Debug, Copy, Clone)]
#[derive(strum_macros::Display)]
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
            _ => BrickColor::None
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
        self.most_likely_brick = *self.readings.iter()
            .rev().take(self.window_size)
            .max_by_key(|&x| self.readings.iter().filter(|&y| *y == *x).count())
            .unwrap_or(&BrickColor::None);
    }

    fn get_most_likely_brick(&self) -> BrickColor {
        self.most_likely_brick
    }

    fn get_readings(&self) -> Vec<BrickColor> {
        self.readings.clone()
    }
}

fn main() -> Ev3Result<()> {
    println!("Hello, EV3!");
    // Get large motor on port outA.
    let motor = MediumMotor::get(MotorPort::OutA)?;

    // Set command "run-direct".
    motor.run_direct()?;

    motor.set_duty_cycle_sp(0)?;

    let sensor = ColorSensor::get(SensorPort::In1)?;

    sensor.set_mode_col_color()?;

    println!("Sensor color: {:?}", sensor.get_color_enum());

    let debouncer = Arc::new(RwLock::new(SensorDebouncer::new(10)));


    // thread for reading sensor data and running debouncer
    let debouncer_sensor = Arc::clone(&debouncer);
    let debouncer_sensor_handle = thread::spawn(move|| {
        loop {
            let reading = sensor.get_color_enum();
            debouncer_sensor.write().unwrap().update(reading);
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    });

    // thread for main loop only reading from debouncer
    let debouncer_reader = Arc::clone(&debouncer);
    let main_handle = thread::spawn(move|| {
        loop {
            //println!("Current color: {:?}", debouncer.get_most_likely_brick());
            println!("Readings: {:?}", debouncer_reader.read().unwrap().get_readings());
            std::thread::sleep(std::time::Duration::from_millis(1000));
        }
    });

    debouncer_sensor_handle.join().unwrap();
    main_handle.join().unwrap();
    Ok(())
}