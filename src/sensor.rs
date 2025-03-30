use esp_idf_hal::{adc::{oneshot::{AdcChannelDriver, AdcDriver}, ADC1, ADC2}, gpio::{Gpio13, Gpio14, Gpio34, Gpio35}};




pub struct SensorArray<'a> {
    pub adc_gpio34: AdcChannelDriver<'a, Gpio34, &'a AdcDriver<'a, ADC1>>, // home
    pub adc_gpio35: AdcChannelDriver<'a, Gpio35, &'a AdcDriver<'a, ADC1>>, // home
    pub adc_gpio13: AdcChannelDriver<'a, Gpio13, &'a AdcDriver<'a, ADC2>>, // away
    pub adc_gpio14: AdcChannelDriver<'a, Gpio14, &'a AdcDriver<'a, ADC2>>, // away
}
