extern crate ev3dev_lang_rust;

use ev3dev_lang_rust::{sensor_mode, Ev3Result};
use ev3dev_lang_rust::motors::{LargeMotor, MediumMotor, TachoMotor, MotorPort};
use ev3dev_lang_rust::sensors::{LightSensor, ColorSensor, HiTechnicColorSensor, SensorPort};

fn main() -> Ev3Result<()> {
    println!("Hello, EV3!");
    // Get large motor on port outA.
    let motor = MediumMotor::get(MotorPort::OutA)?;

    // Set command "run-direct".
    motor.run_direct()?;

    motor.set_duty_cycle_sp(25)?;

    let sensor = LightSensor::get(SensorPort::In1)?;

    #[derive(PartialEq)]
    #[derive(Debug)]
    enum BrickColor {
        None,
        Black,
        Yellow,
        Unknown,
    }

    let mut readings: Vec<BrickColor> = Vec::new();
    // prime with 15 readings of None
    for _ in 0..15 {
        readings.push(BrickColor::None);
    }

    loop {
        let intensity = sensor.get_ambient_light_intensity()? as u8;
        print!("Current ambient light intensity: {:?} recognised as: ", intensity);
        std::thread::sleep(std::time::Duration::from_millis(250));
        // if bightness briefly spikes to ~25 then is black brick, if spikes to ~50 then is yellow
        match intensity {
            0..=21 => readings.push(BrickColor::None),
            22..=44 => readings.push(BrickColor::Black),
            45..=70 => readings.push(BrickColor::Yellow),
            _ => readings.push(BrickColor::Unknown)
        }
        const WINDOW_SIZE: usize = 10;
        // most likely brick is the one that has been detected most frequently, use filter on readings to get most common
        let most_likely_brick = readings.iter().rev().take(WINDOW_SIZE).max_by_key(|&x| readings.iter().filter(|&y| *y == *x).count()).unwrap();
        println!("Last: {:?} Most likely within {:?} {:?}", readings.last(), WINDOW_SIZE, most_likely_brick);
    }


    Ok(())
}