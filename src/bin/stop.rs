use ev3dev_lang_rust::Ev3Result;
use ev3dev_lang_rust::motors::{MediumMotor, MotorPort};

fn main() -> Ev3Result<()> {

    let motor = MediumMotor::get(MotorPort::OutA)?;
    motor.run_direct()?;
    motor.set_duty_cycle_sp(0)?;
    let kicker = MediumMotor::get(MotorPort::OutB)?;
    kicker.run_direct()?;
    kicker.set_duty_cycle_sp(0)?;

    Ok(())
}