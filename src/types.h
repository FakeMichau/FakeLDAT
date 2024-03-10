#include <Arduino.h>
#include <Keyboard.h>
#include <Mouse.h>

enum ReportMode {
  RAW,
  SUMMARY,  // WIP: sometimes drops an event or reports very low latency
  COMBINED
};

enum TriggerMode {
  MOUSE,
  KEYBOARD,
};

enum TriggerOverride {
  NOOVERRIDE = -1,
  RELEASE,
  PRESS
};


class Sensor {
  pin_size_t pin;
  pin_size_t offset_pin;
  uint16_t   brightness;

public:
  Sensor(pin_size_t new_pin, pin_size_t new_offset_pin) {
    analogReadResolution(ADC_RESOLUTION);
    pin = new_pin;
    offset_pin = new_offset_pin;
  }

  void measure() {
    // uint16_t offset = analogRead(offset_pin);
    brightness = analogRead(pin) ^ (1 << ADC_RESOLUTION) - 1;
    // brightness = analogRead(pin);
  }
  uint16_t get_brightness() {
    return brightness;
  }
};

class Button {
  pin_size_t pin;
  bool       last_state;
  bool       current_state;

public:
  Button(pin_size_t new_pin) {
    pin = new_pin;
    pinMode(new_pin, INPUT_PULLUP);
    last_state = false;
  }

  void measure() {
    last_state = current_state;
    current_state = digitalRead(pin) == LOW;
  }
  bool get_state() {
    return current_state;
  }
  bool state_changed() {
    return last_state != current_state;
  }
};

// class Action  {
// };

#define HISTORY_SIZE 150

class FakeLDAT {
  Button*  trigger;
  Sensor*  sensor;
  uint64_t interval_us;
  uint64_t timestamp;
  uint64_t trigger_high_timestamp;

  const bool     trigger_on_press = true; // as opposed to on release
  const uint8_t  mouse_to_press   = MOUSE_LEFT;
  const uint16_t threshold        = 150;

  uint16_t calc_threshold(uint16_t current_value) {
    static uint16_t history[HISTORY_SIZE]{};
    static uint64_t count;
    uint32_t sum = 0;
    for (auto& num : history) sum += num;
    history[count % HISTORY_SIZE] = current_value;
    count++;
    return sum / HISTORY_SIZE + threshold;
  }

public:
  TriggerOverride trigger_override = NOOVERRIDE;

  FakeLDAT(pin_size_t button_pin, pin_size_t sensor_pin, pin_size_t offset_pin, uint64_t rate = 2000) {
    trigger = new Button(button_pin);
    sensor = new Sensor(sensor_pin, offset_pin);
    timestamp = time_us_64();
    trigger_high_timestamp = 0;
    interval_us = 1000000 / rate;
  }
  ~FakeLDAT() {
    delete(trigger);
    delete(sensor);
  }

  const uint64_t get_interval() {
    return interval_us;
  }
  void update() {
    sensor->measure();
    timestamp = time_us_64();
    switch (trigger_override) {
    case RELEASE:
      Mouse.release(mouse_to_press);
      break;
    case PRESS:
      Mouse.press(mouse_to_press);
      break;
    case NOOVERRIDE:
    default:
      trigger->measure();
      if (trigger->state_changed()) {
        if (trigger->get_state() == trigger_on_press) {
          Mouse.press(mouse_to_press);
        }
        else if (trigger->get_state() != trigger_on_press) {
          Mouse.release(mouse_to_press);
        }
      }
      break;
    }
  }
  void report_raw() {
    Serial.printf("%llu,%hu,%hhu\n", timestamp, sensor->get_brightness(), trigger->get_state());
  }
  void report_summary() {
    uint16_t threshold = calc_threshold(sensor->get_brightness());
    if (trigger->state_changed() && trigger->get_state() == trigger_on_press) {
      trigger_high_timestamp = timestamp;
    }
    else if (trigger_high_timestamp && sensor->get_brightness() > threshold) {
      Serial.printf("%llu \t%hu\n", timestamp - trigger_high_timestamp, threshold);
      trigger_high_timestamp = 0;
    }
  }
};
