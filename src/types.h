#include <Arduino.h>
#include <Keyboard.h>
#include <Mouse.h>

enum ReportMode {
  RAW,
  SUMMARY,  // WIP: sometimes drops an event or reports very low latency
  COMBINED
};

enum ActionMode {
  MOUSE,
  KEYBOARD,
};

enum TriggerOverride {
  RELEASE,
  PRESS,
  OVERRIDE_IN_PROGRESS,
  NOOVERRIDE,
};

enum Command {
  SET_POLL_RATE = 0x01,
  GET_POLL_RATE = 0x21,
  SET_REPORT_MODE = 0x02,
  GET_REPORT_MODE = 0x22,
  SET_THRESHOLD = 0x03,
  GET_THRESHOLD = 0x23,
  SET_ACTION = 0x04,
  GET_ACTION = 0x24,
  MANUAL_TRIGGER = 0x1F,
  REPORT_RAW = 0x41,
  REPORT_SUMMARY = 0x42,
};

// commands that can be received
constexpr uint8_t allowed_commands[]{
  SET_POLL_RATE,
  GET_POLL_RATE,
  SET_REPORT_MODE,
  GET_REPORT_MODE,
  SET_THRESHOLD,
  GET_THRESHOLD,
  SET_ACTION,
  GET_ACTION,
  MANUAL_TRIGGER,
};
constexpr uint8_t commands_count = sizeof(allowed_commands);


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

struct Action {
  ActionMode mode;
  uint8_t button;

  Action(ActionMode mode) : mode(mode) { 
    if (mode == MOUSE) button = MOUSE_LEFT;
    else if (mode == KEYBOARD) button = 'x';
  }

  void press() {
    if (mode == MOUSE) Mouse.press(button);
    else if (mode == KEYBOARD) Keyboard.press(button);
  }
  void release() {
    if (mode == MOUSE) Mouse.release(button);
    else if (mode == KEYBOARD) Keyboard.release(button);
  }
};

#define HISTORY_SIZE 150

class FakeLDAT {
  Button* trigger;
  Sensor* sensor;
  uint64_t timestamp;
  uint64_t interval_us = 0;
  uint64_t trigger_high_timestamp = 0;
  uint16_t trigger_override_count = 0;
  int16_t threshold = 150;
  TriggerOverride trigger_override = NOOVERRIDE;

  const bool     trigger_on_press = true; // as opposed to on release

  uint16_t calc_threshold(uint16_t current_value) {
    static uint16_t history[HISTORY_SIZE]{};
    static uint64_t count;
    uint32_t sum = 0;
    for (auto& num : history) sum += num;
    history[count % HISTORY_SIZE] = current_value;
    count++;
    return sum / HISTORY_SIZE + threshold;
  }
  void update_trigger_override() {
    if (trigger_override == PRESS) trigger_override = OVERRIDE_IN_PROGRESS;
    else if (trigger_override == RELEASE) trigger_override = NOOVERRIDE;
    if (trigger_override == NOOVERRIDE) return;
    if (trigger_override == OVERRIDE_IN_PROGRESS && trigger_override_count == 0) {
      trigger_override = RELEASE;
    }
    else {
      trigger_override_count--;
    }
  }
  uint8_t calc_checksum(uint8_t buf[], uint8_t length) {
    uint8_t checksum = 0;
    for (int i = 0; i < length; i++) {
      checksum += buf[i];
    }
    return checksum;
  }
  bool valid_checksum(uint8_t buf[], uint8_t length) {
    uint8_t last_element_index = length - 1;
    uint8_t checksum = calc_checksum(buf, last_element_index);
    return buf[last_element_index] == checksum;
  }
  void write_report(uint8_t command, uint64_t timestamp, uint16_t brightness, uint8_t trigger) {
    uint8_t checksum = command;
    uint8_t bytes[16]{};
    bytes[0] = command;
    for (int i = 0; i < sizeof(timestamp); i++) {
      bytes[1 + i] = (timestamp >> (8 * i)) & 0xFF;
      checksum += bytes[1 + i];
    }
    for (int i = 0; i < sizeof(brightness); i++) {
      bytes[9 + i] = (brightness >> (8 * i)) & 0xFF;
      checksum += bytes[9 + i];
    }
    bytes[11] = trigger;
    checksum += trigger;
    // 12, 13, 14 are empty
    bytes[15] = checksum;
    Serial.write(bytes, sizeof(bytes));
  }
  void update() {
    sensor->measure();
    timestamp = time_us_64();
    switch (trigger_override) {
    case RELEASE:
      action->release();
      if (!trigger_on_press) trigger_high_timestamp = timestamp;
      break;
    case PRESS:
      action->press();
      if (trigger_on_press) trigger_high_timestamp = timestamp;
      break;
    case NOOVERRIDE:
      trigger->measure();
      if (trigger->state_changed()) {
        if (trigger->get_state() == trigger_on_press) {
          action->press();
        }
        else if (trigger->get_state() != trigger_on_press) {
          action->release();
        }
      }
      break;
    default:
      break;
    }
    update_trigger_override();
  }
  void check_for_commands() {
    uint8_t command[16]{};
    while (Serial.available() >= sizeof(command) && Serial.readBytes(command, sizeof(command)) == sizeof(command)) {
      bool valid_command = false;
      for (uint8_t i = 0; i < commands_count; i++) {
        if (allowed_commands[i] == command[0]) {
          valid_command = true;
          break;
        }
      }
      if (!valid_command || !valid_checksum(command, sizeof(command))) continue;
      switch ((Command)command[0]) {

      case SET_POLL_RATE:
        set_rate(static_cast<unsigned>(command[2]) << 8 | static_cast<unsigned>(command[1]));
      case GET_POLL_RATE:
        command[1] = (1000000 / get_interval()) & 0xFF;
        command[2] = (1000000 / get_interval()) >> 8 & 0xFF;
        break;

      case SET_REPORT_MODE:
        if (command[1] > 3) break; // :D
        mode = (ReportMode)command[1];
      case GET_REPORT_MODE:
        command[1] = mode;
        break;

      case SET_THRESHOLD:
        threshold = static_cast<unsigned>(command[2]) << 8 | static_cast<unsigned>(command[1]);
      case GET_THRESHOLD:
        command[1] = threshold & 0xFF;
        command[2] = threshold >> 8 & 0xFF;
        break;

      case SET_ACTION:
        if (command[1] > 2) break; // :D
        action->mode = (ActionMode)command[1];
        action->button = command[2];  // check if key is valid for a given trigger
      case GET_ACTION:
        command[1] = action->mode;
        command[2] = action->button;
        break;

      case MANUAL_TRIGGER:
        manual_trigger();
        break;

      default:
        break;
      }

      command[sizeof(command) - 1] = calc_checksum(command, sizeof(command) - 1);
      // 3 - 14 are empty
      Serial.write(command, sizeof(command));
    }
  }
  void manual_trigger() {
    trigger_override = PRESS;
    trigger_override_count = 50 * 1000 / interval_us; // always 50ms, make configurable?
  }
  void set_rate(uint64_t rate) {
    interval_us = 1000000 / rate;
  }
  void report_raw() {
    auto trigger_state = trigger->get_state() || trigger_override == OVERRIDE_IN_PROGRESS || trigger_override == PRESS;
    write_report(Command::REPORT_RAW, timestamp, sensor->get_brightness(), (uint8_t)trigger_state);
  }
  void report_summary() {
    uint16_t threshold = calc_threshold(sensor->get_brightness());
    if (trigger_override == NOOVERRIDE && trigger->state_changed() && trigger->get_state() == trigger_on_press) {
      trigger_high_timestamp = timestamp;
    }
    else if (trigger_high_timestamp && sensor->get_brightness() > threshold) {
      write_report(Command::REPORT_SUMMARY, timestamp - trigger_high_timestamp, threshold, 1);
      trigger_high_timestamp = 0;
    }
  }


public:
  ReportMode mode;
  Action* action;

  FakeLDAT(pin_size_t button_pin, pin_size_t sensor_pin, pin_size_t offset_pin, uint64_t rate, ReportMode report_mode, ActionMode action_mode) {
    trigger = new Button(button_pin);
    sensor = new Sensor(sensor_pin, offset_pin);
    action = new Action(action_mode);
    timestamp = time_us_64();
    set_rate(rate);
    mode = report_mode;
  }
  ~FakeLDAT() {
    delete(trigger);
    delete(sensor);
    delete(action);
  }

  void tick() {
    check_for_commands();
    update();
    if (mode == RAW || mode == COMBINED) {
      report_raw();
    }
    if (mode == SUMMARY || mode == COMBINED) {
      report_summary();
    }
  }
  const uint64_t get_interval() {
    return interval_us;
  }

};
