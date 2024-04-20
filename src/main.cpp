#include "types.h"

FakeLDAT* m_device;
// TODO: low gain, high gain = resistors

int usb_hid_poll_interval = 1; // default is 10ms
uint64_t time_to_sleep = 0;
void setup() {
  Serial.begin();
  pin_size_t _macro_pin = 20;
  pin_size_t button_pin = 16;
  pin_size_t sensor_pin = 26;
  pin_size_t offset_pin = 27;
  m_device = new FakeLDAT(button_pin, sensor_pin, offset_pin, 2000, RAW, MOUSE);
}

void loop() {
  static uint64_t timestamp = 0;
  const uint64_t interval_us = m_device->get_interval();

  m_device->tick();

  uint64_t time_delta = time_us_64() - timestamp;
  uint64_t time_to_sleep = interval_us < time_delta ? 0 : interval_us - time_delta; // zero meaning it's running behind
  sleep_us(time_to_sleep);
  timestamp = time_us_64();
}
