#include <Arduino.h>
#include <Keyboard.h>
#include <Mouse.h>
#include "config.h"

bool button_state() {
  static PinStatus last_state = HIGH;
  pinMode(BUTTON_PIN, INPUT_PULLUP);
  PinStatus current_state = digitalRead(BUTTON_PIN);
  if (current_state != last_state) {
    if (trigger_mode == KEYBOARD) {
      if      (current_state == LOW)  Keyboard.press(KEY_TO_PRESS);
      else if (current_state == HIGH) Keyboard.release(KEY_TO_PRESS);
    } 
    else if (trigger_mode == MOUSE) {
      if      (current_state == LOW)  Mouse.press(MOUSE_TO_PRESS);
      else if (current_state == HIGH) Mouse.release(MOUSE_TO_PRESS);
    }
    last_state = current_state;
  }
  return current_state == LOW;
}

uint16_t calc_threshold(uint16_t current_value) {
  static uint16_t history[HISTORY_SIZE]{};
  static uint64_t count;
  uint32_t sum = 0;
  for (auto& num : history) sum += num;
  history[count % HISTORY_SIZE] = current_value;
  count++;
  return sum / (HISTORY_SIZE / THRESHOLD);
}

void setup() {
}

void loop() {
}

void setup1() {
  analogReadResolution(ADC_RESOLUTION);
}

void loop1() {
  uint64_t timestamp  = time_us_64();
  uint16_t brightness = analogRead(SENSOR_PIN); // 0 - 4095, higher = darker
  uint8_t  button     = button_state();

  if (report_mode == RAW || report_mode == COMBINED) {
    Serial.printf("%llu,%hu,%hhu\n", timestamp, brightness, button);
  }
  if (report_mode == SUMMARY || report_mode == COMBINED) {
    static uint8_t  last_button_state = 0;
    static uint64_t button_high_time = 0;

    uint16_t threshold = calc_threshold(brightness);

    if (last_button_state != button && button == TRIGGER_ON_PRESS) {
      button_high_time = timestamp;
    } 
    else if (button_high_time && brightness < threshold) {
      Serial.printf("%llu \t%hu\n", timestamp - button_high_time, threshold);
      button_high_time = 0;
    }
    last_button_state = button;
  }

  uint64_t time_delta = time_us_64() - timestamp;
  uint64_t time_to_sleep = INTERVAL_US < time_delta ? 0 : INTERVAL_US - time_delta; // zero meaning it's running behind
  sleep_us(time_to_sleep);
}