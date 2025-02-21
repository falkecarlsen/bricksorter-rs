#![allow(unreachable_code)]

extern crate ev3dev_lang_rust;

use core::time::Duration;
use ev3dev_lang_rust::motors::{MediumMotor, MotorPort, TachoMotor};
use ev3dev_lang_rust::sensors::{ColorSensor, LightSensor, Sensor, SensorPort};
use ev3dev_lang_rust::Ev3Result;
use std::sync::{Arc, RwLock};
use std::thread;
use std::thread::sleep;

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
    brick_window: Vec<BrickColor>,
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
            brick_window: Vec::new(),
        }
    }

    fn update(&mut self, new_reading: BrickColor) {
        self.readings.push(new_reading);
        // remove oldest reading beyond window size
        if self.readings.len() > self.window_size {
            self.readings.remove(0);
        }
        // most likely brick is the one that has been detected most frequently, use filter on readings to get most common
        let curr_brick = *self
            .readings
            .iter()
            .rev()
            .take(self.window_size)
            .max_by_key(|&x| self.readings.iter().filter(|&y| *y == *x).count())
            .unwrap_or(&BrickColor::None);
        // update most likely brick
        self.update_brick_window(curr_brick);
        // most likely brick has been detected thrice and is not BrickColor::None
        if self.readings.iter().filter(|&x| *x == curr_brick).count() >= 3 && curr_brick != BrickColor::None {
            self.most_likely_brick = curr_brick;
        } else { // if no brick has been detected thrice, set most likely brick to None
            self.most_likely_brick = BrickColor::None;
        }


    }

    fn update_brick_window(&mut self, brick: BrickColor) {
        self.brick_window.push(brick);
        if self.brick_window.len() > 5 {
            self.brick_window.remove(0);
        }
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

    #[cfg(feature = "debug")]
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
        BrickColor::Blue => 1,
        BrickColor::Green => 1,
        BrickColor::Yellow => -1,
        BrickColor::Red => -1,
        BrickColor::White => 0,
        BrickColor::Brown => 1,
    };

    if direction == 0 {
        return Ok(()); // no kick
    }

    let time_now = std::time::Instant::now();

    let prime_angle = 170;
    let mut angular_movement = 0;

    // do prime operation
    angular_movement += prime_angle * direction;
    let max_speed = 120;
    let steps = 5;
    for i in 0..steps {
        let speed = max_speed - i * (max_speed / steps);
        run_to_abs_pos(&kicker, angular_movement / steps, speed)?;
    }

    // delay, ensure no negative time
    if time_now.elapsed() < duration {
        sleep(duration - time_now.elapsed());
    } else {
        eprintln!("WARN: Prime operation took longer than required duration! {:?}", time_now.elapsed());
    }

    // kick
    let kick_angle = -angular_movement - 45 * direction;
    run_to_abs_pos(&kicker, kick_angle, 500)?;
    angular_movement += kick_angle;

    #[cfg(feature = "debug")]
    println!("Time to kick brick: {:?}", time_now.elapsed());

    sleep(Duration::from_millis(500));

    // return to initial position
    run_to_abs_pos(&kicker, -angular_movement, 500)?;

    #[cfg(feature = "debug")]
    println!("angular movement: {}", angular_movement);

    Ok(())
}

fn main() -> Ev3Result<()> {
    println!("Bricksorter-rs starting...");

    let conveyor_speed = 45;
    let conveyor = MediumMotor::get(MotorPort::OutA)?;
    let kicker = TachoMotor::get(MotorPort::OutB)?;

    let s1 = ColorSensor::get(SensorPort::In1)?;
    let s2 = ColorSensor::get(SensorPort::In2)?;
    s1.set_mode_col_color()?;
    s2.set_mode_col_color()?;
    let s3 = LightSensor::get(SensorPort::In3)?;

    // get base ambient light level over 5 samples, avg'ed out
    let mut base_light_level = 0;
    for _ in 0..10 {
        base_light_level += s3.get_value0().unwrap();
        sleep(Duration::from_millis(50));
    }

    let base_light_level = base_light_level / 10;
    println!("Base light level: {}", base_light_level);
    let s3_v_bias = 50;

    conveyor.run_direct()?;
    conveyor.set_duty_cycle_sp(conveyor_speed)?;

    // reset kicker rotary encoder position (ensure piston is aimed at center of conveyor on init)
    // note: positive degrees are counter-clockwise, negative are clockwise from persp. back motor
    let _first_pos = kicker.set_position(0)?;
    kicker.set_speed_sp(300)?;
    kicker.set_stop_action(TachoMotor::STOP_ACTION_HOLD)?; // ensure angle is held on kicker


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
        sleep(Duration::from_millis(16));
    });


    // thread for main loop only reading from debouncer
    let s1_debounce_reader = Arc::clone(&debouncer_s1);
    let s2_debounce_reader = Arc::clone(&debouncer_s2);
    let main_handle = thread::spawn(move || loop {
        #[cfg(feature = "debug")]
        println!(
            "Most likely bricks: s1: {: <8} s2: {: <8}, s1_bricks: {:?}, s2_bricks: {:?}",
            s1_debounce_reader.read().unwrap().get_most_likely_brick(),
            s2_debounce_reader.read().unwrap().get_most_likely_brick(),
            s1_debounce_reader.read().unwrap().brick_window,
            s2_debounce_reader.read().unwrap().brick_window,
        );

        // pause to drill object on conveyor if light beam is disturbed
        if cfg!(feature = "drill") && i32::abs(s3.get_value0().unwrap() - base_light_level) > s3_v_bias {
            println!("Drilling brick on conveyor!");
            conveyor.set_duty_cycle_sp(0).unwrap();
            sleep(Duration::from_millis(1000));
            conveyor.set_duty_cycle_sp(conveyor_speed).unwrap();
            // wait for object to be removed before ending routine
            while i32::abs(s3.get_value0().unwrap() - base_light_level) > s3_v_bias {
                sleep(Duration::from_millis(100));
            }
        }

        // schedule kick if brick is detected
        if s2_debounce_reader.read().unwrap().get_most_likely_brick() != BrickColor::None {
            let brick_color = s2_debounce_reader.read().unwrap().get_most_likely_brick();
            println!("Kicking a brick: {:?}", brick_color);
            schedule_timed_piston(&kicker, Duration::from_millis(4900), brick_color).unwrap();
        };

        sleep(Duration::from_millis(200));
    });

    debouncer_sensor_handle.join().unwrap();
    main_handle.join().unwrap();
    Ok(())
}
