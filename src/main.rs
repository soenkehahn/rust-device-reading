#[macro_use]
extern crate galvanic_test;
extern crate jack;

mod areas;
mod evdev;
mod generator;
mod input;
mod run_jack;

use areas::Areas;
use evdev::*;
use generator::Generator;
use input::MouseInput;
use run_jack::run_jack_generator;
use std::clone::Clone;
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::*;

#[derive(Debug)]
pub enum AppError {
    JackError(jack::Error),
    AppError { description: String },
}

impl AppError {
    fn new(description: String) -> AppError {
        AppError::AppError { description }
    }
}

impl<E: std::error::Error> From<E> for AppError {
    fn from(e: E) -> Self {
        AppError::AppError {
            description: String::from(e.description()),
        }
    }
}

fn main() -> Result<(), AppError> {
    fork_evdev_logging();
    let mutex = Arc::new(Mutex::new(Generator::new(300.0)));
    let _active_client = run_jack_generator(mutex.clone()).map_err(AppError::JackError)?;
    let mouse_input = MouseInput::new(File::open("/dev/input/mice")?);
    mouse_input.for_each(|position| {
        let frequency = 300.0 + position.x as f32;
        match mutex.lock() {
            Err(e) => {
                println!("main_: error: {:?}", e);
            }
            Ok(mut generator) => {
                generator.frequency = frequency;
            }
        }
    });
    Ok(())
}

fn fork_evdev_logging() {
    thread::spawn(|| {
        let areas = Areas::new(100);
        for position in Positions::new("/dev/input/event15").unwrap() {
            println!("{:?}", position);
            position.if_touch(&|position| {
                println!("{:?}", areas.frequency(position));
            });
        }
    });
}
