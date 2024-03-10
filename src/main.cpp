#include "types.h"

FakeLDAT *m_device;
#define KEY_TO_PRESS     0x78  // ASCII

void setup() {
  Serial.begin();
  pin_size_t _auto_pin  = 15;
  pin_size_t button_pin = 13;
  pin_size_t sensor_pin = 26;
  pin_size_t offset_pin = 27;
  m_device = new FakeLDAT(button_pin, sensor_pin, offset_pin, 2000);
}

void loop() {
  const uint64_t interval_us = m_device->get_interval();

  uint64_t timestamp  = time_us_64();

  m_device->update();
  // m_device->report_raw();
  m_device->report_summary();

  uint64_t time_delta = time_us_64() - timestamp;
  uint64_t time_to_sleep = interval_us < time_delta ? 0 : interval_us - time_delta; // zero meaning it's running behind
  sleep_us(time_to_sleep);
}

void setup1() {
}


void loop1() {
}
