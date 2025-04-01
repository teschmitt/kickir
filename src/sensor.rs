use std::str::FromStr;

use anyhow::Result;
use esp_idf_hal::{
    adc::{
        oneshot::{AdcChannelDriver, AdcDriver},
        ADC1, ADC2,
    },
    gpio::{Gpio13, Gpio14, Gpio34, Gpio35},
};

pub type ThreshValue = u16;

#[derive(Debug, PartialEq, Eq)]
pub struct ParseThreshChangeError;

#[derive(Debug)]
pub enum ThreshSide {
    Home,
    Away,
}

#[derive(Debug)]
pub struct ThreshChange {
    pub side: ThreshSide,
    pub new_value: ThreshValue,
}

impl FromStr for ThreshChange {
    type Err = ParseThreshChangeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').map(|part| part.trim()).collect();
        if parts.len() != 2 {
            return Err(ParseThreshChangeError);
        }
        let side = match parts[0].to_uppercase().as_str() {
            "HOME" => ThreshSide::Home,
            "AWAY" => ThreshSide::Away,
            _ => return Err(ParseThreshChangeError),
        };
        let new_value = parts[1]
            .parse::<ThreshValue>()
            .map_err(|_| ParseThreshChangeError)?;

        Ok(ThreshChange { side, new_value })
    }
}

pub struct SensorArray<'a> {
    pub adc_gpio34: AdcChannelDriver<'a, Gpio34, &'a AdcDriver<'a, ADC1>>, // home
    pub adc_gpio35: AdcChannelDriver<'a, Gpio35, &'a AdcDriver<'a, ADC1>>, // home
    pub adc_gpio13: AdcChannelDriver<'a, Gpio13, &'a AdcDriver<'a, ADC2>>, // away
    pub adc_gpio14: AdcChannelDriver<'a, Gpio14, &'a AdcDriver<'a, ADC2>>, // away
}
