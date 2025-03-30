use crate::sensor::SensorArray;
use anyhow::Result;
use log::error;
use std::{
    fmt::Display,
    time::{Duration, Instant},
};

const WAIT_AFTER_DETECTION: Duration = Duration::from_secs(2);
const THRESHOLD_DETECT_OBJECT: u16 = 50;

#[derive(Default)]
pub enum DetectedGoal {
    #[default]
    None,
    Home,
    Away,
}

impl Display for DetectedGoal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectedGoal::Home => write!(f, "Home"),
            DetectedGoal::Away => write!(f, "Away"),
            DetectedGoal::None => write!(f, "None"),
        }
    }
}

pub struct GoalDetector<'a> {
    pub last_goal: std::time::Instant,
    sensors: SensorArray<'a>,
    threshold_home: u16,
    threshold_away: u16,
}

impl<'a> GoalDetector<'a> {
    pub fn new(sensors: SensorArray<'a>) -> Self {
        Self {
            last_goal: Instant::now(),
            sensors,
            threshold_home: THRESHOLD_DETECT_OBJECT,
            threshold_away: THRESHOLD_DETECT_OBJECT,
        }
    }

    fn home_triggered(&mut self) -> Result<bool> {
        Ok(self.sensors.adc_gpio34.read()? < self.threshold_home
            || self.sensors.adc_gpio35.read()? < self.threshold_home)
    }

    fn away_triggered(&mut self) -> Result<bool> {
        Ok(self.sensors.adc_gpio13.read()? < self.threshold_away
            || self.sensors.adc_gpio14.read()? < self.threshold_away)
    }

    pub fn scan(&mut self) -> DetectedGoal {
        match (
            self.last_goal.elapsed() >= WAIT_AFTER_DETECTION,
            self.home_triggered(),
            self.away_triggered(),
        ) {
            (true, Ok(true), _) => DetectedGoal::Home,
            (true, _, Ok(true)) => DetectedGoal::Away,
            (_, Err(e), _) => {
                error!("Error reading home sensor: {:?}", e);
                DetectedGoal::None
            }
            (_, _, Err(e)) => {
                error!("Error reading away sensor: {:?}", e);
                DetectedGoal::None
            }
            _ => DetectedGoal::None,
        }
    }

    pub fn last_goal_now(&mut self) {
        self.last_goal = Instant::now();
    }
}
